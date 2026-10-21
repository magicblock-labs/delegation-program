use delegation_program_interface as dpi;
use dpi::{CommitFinalizeArgs, CommitPayloadKind};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use wheels::{layout::Decodable, require_eq, require_n_accounts};

use super::{
    process_commit_finalize_internal, CommitFinalizeInput, CommitStateUpdate,
};

/// Commit and immediately apply one state update for a Direct cluster.
///
/// Accounts:
/// 0: `[signer, writable]` current validator and lamport settlement account
/// 1: `[writable]`         delegated account
/// 2: `[writable]`         DelegationState PDA
/// 3: `[]`                 DelegationCluster PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_commit_finalize(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        current_validator, // force multi-line
        delegated_account,
        delegation_state,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    let args = CommitFinalizeArgs::decode(data)?;
    require_eq!(
        args.payload_alignment_padding(),
        0,
        ProgramError::InvalidInstructionData
    );
    let state_update = match args.payload_kind() {
        value if value == CommitPayloadKind::FullData.value() => {
            CommitStateUpdate::FullData(args.payload())
        }
        value if value == CommitPayloadKind::Diff.value() => {
            CommitStateUpdate::Diff(dpi::diff::DiffSet::try_new(
                args.payload(),
            )?)
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    process_commit_finalize_internal(
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
