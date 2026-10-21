use delegation_program_interface as dpi;
use dpi::{
    error::DlpError, pda::STATE_BUFFER_SEED, CommitFinalizeFromBufferArgs,
    CommitPayloadKind, StateBuffer,
};
use pinocchio::{
    address::address_eq, error::ProgramError, AccountView, ProgramResult,
};
use wheels::{
    layout::Decodable, require_eq, require_eq_keys, require_n_accounts,
};

use crate::processor::{
    internal::{
        apply_commit_finalize, CommitFinalizeInput, CommitStateUpdate,
    },
    state_buffer_payload_hash,
};

/// Commit and apply one finalized StateBuffer for a Direct cluster.
///
/// Accounts:
/// 0: `[signer, writable]` current validator and lamport settlement account
/// 1: `[writable]`         delegated account
/// 2: `[writable]`         DelegationState PDA
/// 3: `[]`                 finalized StateBuffer PDA
/// 4: `[]`                 DelegationCluster PDA
/// 5: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_commit_finalize_from_buffer(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        current_validator, // force multi-line
        delegated_account,
        delegation_state,
        state_buffer,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 6);

    let args = CommitFinalizeFromBufferArgs::decode(data)?;
    let state_buffer_data = state_buffer.try_borrow()?;
    let buffer = StateBuffer::decode(state_buffer_data.as_ref())?;
    validate_state_buffer(
        current_validator,
        delegated_account,
        state_buffer,
        &buffer,
        args.commit_id(),
    )?;

    let state_update = match args.payload_kind() {
        value if value == CommitPayloadKind::FullData.value() => {
            CommitStateUpdate::FullData(buffer.payload().as_slice())
        }
        value if value == CommitPayloadKind::Diff.value() => {
            CommitStateUpdate::Diff(dpi::diff::DiffSet::try_new(
                buffer.payload().as_slice(),
            )?)
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    apply_commit_finalize(
        current_validator,
        delegated_account,
        delegation_state,
        delegation_cluster,
        CommitFinalizeInput {
            commit_id: args.commit_id(),
            lamports: args.lamports(),
            owner: args.owner(),
            allow_undelegation: args.allow_undelegation(),
            state_update,
        },
    )
}

fn validate_state_buffer(
    current_validator: &AccountView,
    delegated_account: &AccountView,
    state_buffer: &AccountView,
    buffer: &dpi::StateBufferView<'_>,
    commit_id: u64,
) -> ProgramResult {
    let commit_id_bytes = commit_id.to_le_bytes();
    let bump = [buffer.bump()];
    dpi::require_initialized_pda!(
        state_buffer,
        &[
            STATE_BUFFER_SEED,
            delegated_account.address().as_ref(),
            &commit_id_bytes,
            current_validator.address().as_ref(),
            &bump,
        ],
        &crate::fast::ID,
        false
    );
    require_eq!(
        &buffer.discriminator(),
        &StateBuffer::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        buffer.authority(),
        current_validator.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        buffer.account_pubkey(),
        delegated_account.address(),
        DlpError::InvalidDelegatedAccount
    );
    require_eq!(
        buffer.commit_id(),
        commit_id,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        buffer.finalized(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        &buffer.payload_alignment_padding(),
        &[0; 2],
        ProgramError::InvalidAccountData
    );
    require_eq!(
        buffer.payload().len(),
        buffer.total_len() as usize,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        buffer.payload().capacity(),
        buffer.total_len() as usize,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state_buffer.data_len(),
        StateBuffer::data_len_from_payload_capacity(
            buffer.payload().capacity()
        )?,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        buffer.data_hash(),
        &state_buffer_payload_hash(buffer.payload().as_slice()),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
