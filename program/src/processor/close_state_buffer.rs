use delegation_program_interface as dpi;
use dpi::{
    error::DlpError,
    pda::{PENDING_COMMITMENT_SEED, STATE_BUFFER_SEED},
    CloseStateBufferArgs, PendingCommitment, StateBuffer,
    PENDING_COMMITMENT_STATUS_CANCELLED, PENDING_COMMITMENT_STATUS_EXPIRED,
    PENDING_COMMITMENT_STATUS_FINALIZED,
};
use pinocchio::{
    address::address_eq, error::ProgramError, AccountView, ProgramResult,
};
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys,
    require_n_accounts, require_owned_by, require_signer,
};

use crate::requires::is_uninitialized_account;

/// Close a StateBuffer and return its rent to the payer that created it.
///
/// Accounts:
/// 0: `[signer]`   buffer authority
/// 1: `[writable]` StateBuffer PDA
/// 2: `[]`         delegated account identified by the buffer
/// 3: `[writable]` stored StateBuffer rent payer
/// 4: `[]`         PendingCommitment PDA, initialized or not
#[inline(never)]
pub fn process_close_state_buffer(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [authority, state_buffer, delegated_account, rent_payer, pending_commitment] =
        require_n_accounts!(accounts, 5);

    require_signer!(authority);
    require!(rent_payer.is_writable(), ProgramError::Immutable);

    let args = CloseStateBufferArgs::decode(data)?;
    let commit_seq = args.commit_seq().to_le_bytes();
    let state_buffer_bump = [args.state_buffer_bump()];
    dpi::require_pda!(
        state_buffer,
        &[
            STATE_BUFFER_SEED,
            delegated_account.address().as_ref(),
            &commit_seq,
            authority.address().as_ref(),
            &state_buffer_bump,
        ],
        &crate::fast::ID,
        true
    );

    // The close operation is idempotent so cleanup can be retried safely.
    if is_uninitialized_account(state_buffer) {
        return Ok(());
    }

    require_owned_by!(state_buffer, &crate::fast::ID);
    let state_buffer_data = state_buffer.try_borrow()?;
    let state = StateBuffer::decode(state_buffer_data.as_ref())?;
    require_eq!(
        &state.discriminator(),
        &StateBuffer::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        state.bump(),
        args.state_buffer_bump(),
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        state.authority(),
        authority.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        state.account_pubkey(),
        delegated_account.address(),
        ProgramError::InvalidAccountData
    );
    require_eq!(
        state.commit_seq(),
        args.commit_seq(),
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        state.rent_payer(),
        rent_payer.address(),
        DlpError::InvalidStateBufferRentPayer
    );

    let pending_commitment_bump = [args.pending_commitment_bump()];
    dpi::require_pda!(
        pending_commitment,
        &[
            PENDING_COMMITMENT_SEED,
            delegated_account.address().as_ref(),
            &commit_seq,
            &pending_commitment_bump,
        ],
        &crate::fast::ID,
        false
    );
    if !is_uninitialized_account(pending_commitment) {
        require_owned_by!(pending_commitment, &crate::fast::ID);
        let pending_data = pending_commitment.try_borrow()?;
        let pending = PendingCommitment::decode(pending_data.as_ref())?;
        require_eq!(
            &pending.discriminator(),
            &PendingCommitment::DISCRIMINATOR,
            ProgramError::InvalidAccountData
        );
        require_eq_keys!(
            pending.account_pubkey(),
            delegated_account.address(),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            pending.commit_seq(),
            args.commit_seq(),
            ProgramError::InvalidAccountData
        );
        require!(
            matches!(
                pending.status(),
                PENDING_COMMITMENT_STATUS_FINALIZED
                    | PENDING_COMMITMENT_STATUS_EXPIRED
                    | PENDING_COMMITMENT_STATUS_CANCELLED
            ),
            DlpError::StateBufferInUse
        );
    }

    drop(state_buffer_data);
    rent_payer.set_lamports(
        rent_payer
            .lamports()
            .checked_add(state_buffer.lamports())
            .ok_or(DlpError::Overflow)?,
    );
    state_buffer.set_lamports(0);
    unsafe {
        state_buffer.assign(&pinocchio_system::ID);
    }
    state_buffer.resize(0)
}
