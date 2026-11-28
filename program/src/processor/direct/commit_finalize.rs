use delegation_program_interface as dpi;
use dpi::{CommitFinalizeArgs, CommitPayloadKind};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use wheels::{layout::Decodable, require_n_accounts};

use crate::processor::internal::{
    apply_commit_finalize, CommitFinalizeInput, CommitStateUpdate,
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

    // return: 89

    let args = CommitFinalizeArgs::decode(data)?;

    // return: 126

    let state_update = match args.payload_kind_x() {
        value if value == CommitPayloadKind::FullData.value() => {
            CommitStateUpdate::FullData(args.payload_x())
        }
        value if value == CommitPayloadKind::Diff.value() => {
            CommitStateUpdate::Diff(dpi::diff::DiffSet::try_new(
                args.payload_x(),
            )?)
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    // return: 162 CU

    const BRANCH: u32 = 2;

    let input = if BRANCH == 1 {
        // cu: 201
        CommitFinalizeInput {
            commit_seq: args.commit_seq(),
            lamports: args.lamports(),
            owner: args.owner(),
            allow_undelegation: args.allow_undelegation(),
            state_update,
        }
    } else if BRANCH == 2 {
        // cu: 163
        CommitFinalizeInput {
            commit_seq: args.commit_seq_x(),
            lamports: args.lamports_x(),
            owner: args.owner_x(),
            allow_undelegation: args.allow_undelegation_x(),
            state_update,
        }
    } else {
        // 162
        CommitFinalizeInput {
            commit_seq: 0,
            lamports: 0,
            owner: &crate::ID,
            allow_undelegation: false,
            state_update,
        }
    };

    // return: if (201 CU) vs else (162 CU)

    //if delegation_state.data_len() < 100000 {
    //    return Ok(());
    //}

    apply_commit_finalize(
        current_validator,
        delegated_account,
        delegation_state,
        delegation_cluster,
        input,
        //CommitFinalizeInput {
        //    commit_seq: args.commit_seq(),
        //    lamports: args.lamports(),
        //    owner: args.owner(),
        //    allow_undelegation: args.allow_undelegation(),
        //    state_update,
        //},
    )
}
