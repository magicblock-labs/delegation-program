use dlp_api::compat::borsh::BorshDeserialize;
use pinocchio::{
    account::MAX_PERMITTED_DATA_INCREASE,
    address::{address_eq, Address},
    cpi::Signer,
    error::ProgramError,
    instruction::seeds,
    AccountView, ProgramResult,
};
use pinocchio_log::log;

use super::to_pinocchio_program_error;
use crate::{
    args::{PreallocateBufferArgs, PreallocateBufferKind},
    error::DlpError,
    pda,
    processor::fast::utils::pda::{create_pda, resize_pda},
    require_gt,
    requires::{
        is_uninitialized_account, require_initialized_delegation_record,
        require_initialized_validator_fees_vault, require_owned_pda,
        require_pda, require_signer,
    },
    state::DelegationRecord,
};

/// Grows a DLP-owned buffer (or the delegated account itself) towards a
/// target size, in steps of at most `MAX_PERMITTED_DATA_INCREASE` bytes.
///
/// Since that growth cap resets for every top-level instruction, a validator
/// that needs to commit/undelegate an account larger than the cap can send
/// several `PreallocateBuffer` instructions (same args, repeated) ahead of
/// the commit/finalize (or undelegate) instruction, all packed into the same
/// transaction, to reach the required size before the actual write happens.
///
/// Accounts:
///
/// 0: `[signer, writable]` the payer funding rent for the growth
/// 1: `[]`                 the delegated account
/// 2: `[writable]`         the delegation record account (only actually
///                         written for `kind = DelegatedAccount`, see below)
/// 3: `[writable]`         the buffer PDA to grow (unused for `DelegatedAccount`)
/// 4: `[]`                 the commit record account
/// 5: `[]`                 the validator fees vault
/// 6: `[]`                 the system program
///
/// Requirements:
///
/// - delegated account is owned by delegation program
/// - delegation record is initialized
/// - validator is a registered validator (has an initialized fees vault) and
///   is the delegation's authority
/// - for `kind = CommitState`, no commit is currently in flight for the
///   delegated account (commit record must be uninitialized), mirroring the
///   precondition `commit_state`/`commit_diff` themselves enforce before
///   they'll create this buffer.
/// - `kind = DelegatedAccount` / `UndelegateBuffer` have no such
///   restriction: both are meant to run precisely *while* a commit record is
///   initialized -- `DelegatedAccount` between `commit_state` and `finalize`,
///   to pre-grow the account before finalize's single-shot resize; and
///   `UndelegateBuffer` alongside finalize in a two-stage commit-and-undelegate,
///   ahead of `undelegate`'s own check (which requires the commit record to be
///   uninitialized by the time *it* runs, not by the time it's preallocated).
/// - for `kind = CommitState` / `UndelegateBuffer`, `target_size` must exceed
///   `MAX_PERMITTED_DATA_INCREASE` -- those buffer PDAs are always closed and
///   recreated fresh each cycle, so a smaller target can always be created
///   directly, in one shot, by the consuming instruction itself.
/// - for `kind = DelegatedAccount`, rent transferred to fund the growth is
///   also added to `delegation_record.lamports`, keeping the ledger in sync
///   with the account's actual balance -- otherwise finalize/commit_finalize's
///   settlement (which diffs against `delegation_record.lamports`) would fund
///   the same growth a second time.
pub fn process_preallocate_buffer(
    _program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, delegation_record_account, buffer_account, commit_record_account, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    require_initialized_delegation_record(
        delegated_account,
        delegation_record_account,
        true,
    )?;
    let mut delegation_record_data =
        delegation_record_account.try_borrow_mut()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator_mut(
            &mut delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

    // Validate common requirements
    validate_common(
        validator,
        delegated_account,
        validator_fees_vault,
        delegation_record,
    )?;

    let args = PreallocateBufferArgs::try_from_slice(data)
        .map_err(|_| ProgramError::BorshIoError)?;
    let target_size = args.target_size as usize;
    match args.kind {
        PreallocateBufferKind::CommitState => {
            // Buffer is closed and recreated fresh every cycle, so a target
            // this small can always be created directly, in one shot, by
            // the consuming instruction itself -- there's never a
            // legitimate reason to preallocate one. Rejecting it keeps
            // "buffer PDA is DLP-owned" synonymous with "it had to be
            // preallocated", which consumers rely on.
            require_gt!(
                target_size,
                MAX_PERMITTED_DATA_INCREASE,
                DlpError::PreallocateBufferTargetTooSmall
            );
            require_no_commit_in_flight(
                delegated_account,
                commit_record_account,
            )?;
            let seeds_arr =
                [pda::COMMIT_STATE_TAG, delegated_account.address().as_ref()];
            let bump = require_pda(
                buffer_account,
                &seeds_arr,
                &crate::fast::ID,
                true,
                "commit state",
            )?;
            grow_target(
                buffer_account,
                &seeds_arr,
                bump,
                target_size,
                validator,
                system_program,
            )
        }
        PreallocateBufferKind::UndelegateBuffer => {
            // Same reasoning as the `CommitState` arm above.
            require_gt!(
                target_size,
                MAX_PERMITTED_DATA_INCREASE,
                DlpError::PreallocateBufferTargetTooSmall
            );
            let seeds_arr = [
                pda::UNDELEGATE_BUFFER_TAG,
                delegated_account.address().as_ref(),
            ];
            let bump = require_pda(
                buffer_account,
                &seeds_arr,
                &crate::fast::ID,
                true,
                "undelegate buffer",
            )?;
            grow_target(
                buffer_account,
                &seeds_arr,
                bump,
                target_size,
                validator,
                system_program,
            )
        }
        PreallocateBufferKind::DelegatedAccount => {
            let transferred = resize_towards(
                delegated_account,
                target_size,
                validator,
                system_program,
            )?;
            // The delegated account's balance persists across cycles and is
            // tracked by `delegation_record.lamports`; finalize/commit_finalize
            // settlement later diffs the ER's reported balance against that
            // value, so it must reflect rent this instruction already funded --
            // otherwise settlement re-funds the same growth again.
            delegation_record.lamports =
                delegation_record.lamports.saturating_add(transferred);
            Ok(())
        }
    }
}

/// Common account validation shared by every `kind`: the validator must be a
/// signer, a registered validator (has an initialized fees vault), and the
/// authority of this specific delegation.
fn validate_common(
    validator: &AccountView,
    delegated_account: &AccountView,
    validator_fees_vault: &AccountView,
    delegation_record: &DelegationRecord,
) -> ProgramResult {
    // Ensure validator is signer and is whitelisted
    require_signer(validator, "validator")?;
    require_initialized_validator_fees_vault(
        validator,
        validator_fees_vault,
        false,
    )?;

    // Validate account is delegated
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;

    // Ensure validator doesn't intervene in other validator's accounts
    if !address_eq(
        &delegation_record.authority.to_bytes().into(),
        validator.address(),
    ) {
        log!("validator is not the delegation authority. validator: ");
        validator.address().log();
        log!("delegation authority: ");
        Address::from(delegation_record.authority.to_bytes()).log();
        Err(DlpError::InvalidAuthority.into())
    } else {
        Ok(())
    }
}

/// Makes sure that there's no in flight commit
fn require_no_commit_in_flight(
    delegated_account: &AccountView,
    commit_record_account: &AccountView,
) -> ProgramResult {
    require_pda(
        commit_record_account,
        &[pda::COMMIT_RECORD_TAG, delegated_account.address().as_ref()],
        &crate::fast::ID,
        false,
        "commit record",
    )?;
    if !is_uninitialized_account(commit_record_account) {
        Err(DlpError::PreallocateBufferCommitInFlight.into())
    } else {
        Ok(())
    }
}

/// Grows PDA towards `target_size`, creating it fresh if it doesn't exist yet,
/// or resizing it by at most`MAX_PERMITTED_DATA_INCREASE` bytes
fn grow_target(
    target_account: &AccountView,
    seeds_arr: &[&[u8]; 2],
    bump: u8,
    target_size: usize,
    payer: &AccountView,
    system_program: &AccountView,
) -> ProgramResult {
    if is_uninitialized_account(target_account) {
        let size = target_size.min(MAX_PERMITTED_DATA_INCREASE);
        create_pda(
            target_account,
            &crate::fast::ID,
            size,
            &[Signer::from(&seeds!(seeds_arr[0], seeds_arr[1], &[bump]))],
            payer,
        )
    } else {
        resize_towards(target_account, target_size, payer, system_program)?;
        Ok(())
    }
}

/// Resizes `account` towards `target_size` by at most
/// `MAX_PERMITTED_DATA_INCREASE` bytes; a no-op if it's already at or past
/// `target_size` (shrinking is never growth-capped, so it's left to whichever
/// instruction actually performs the exact-size write). Returns the amount
/// of lamports transferred to fund the growth (0 if none was needed).
fn resize_towards(
    account: &AccountView,
    target_size: usize,
    payer: &AccountView,
    system_program: &AccountView,
) -> Result<u64, ProgramError> {
    let current = account.data_len();
    if current < target_size {
        let realloc_size =
            core::cmp::min(target_size - current, MAX_PERMITTED_DATA_INCREASE);
        let new_size = current + realloc_size;
        resize_pda(payer, account, system_program, new_size)
    } else {
        Ok(0)
    }
}
