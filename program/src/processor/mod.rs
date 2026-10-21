use delegation_program_interface as dpi;
mod bootstrap;
mod delegate;
mod direct;
mod fraud_proofs;
pub(crate) mod internal;

pub use bootstrap::*;
pub use delegate::*;
pub use direct::*;
use dpi::DlpV2Instruction;
pub use fraud_proofs::*;
use pinocchio::{AccountView, ProgramResult};

#[inline(never)]
pub fn process_instruction(
    accounts: &[AccountView],
    data: &[u8],
    ix: DlpV2Instruction,
) -> ProgramResult {
    match ix {
        DlpV2Instruction::InitDelegationCluster => {
            process_init_delegation_cluster(accounts, data)
        }
        DlpV2Instruction::RegisterOperator => {
            process_register_operator(accounts, data)
        }
        DlpV2Instruction::RegisterVerifier => {
            process_register_verifier(accounts, data)
        }
        DlpV2Instruction::UpdateVerifierRegistry => {
            process_update_verifier_registry(accounts, data)
        }
        DlpV2Instruction::UpdateDelegationCluster => {
            process_update_delegation_cluster(accounts, data)
        }
        DlpV2Instruction::ProposeCommitment => {
            process_propose_commitment(accounts, data)
        }
        DlpV2Instruction::ApproveCommitment => {
            process_approve_commitment(accounts, data)
        }
        DlpV2Instruction::WriteStateBuffer => {
            process_write_state_buffer(accounts, data)
        }
        DlpV2Instruction::FinalizeCommitment => {
            process_finalize_commitment(accounts, data)
        }
        DlpV2Instruction::RaiseChallenge => {
            process_raise_challenge(accounts, data)
        }
        DlpV2Instruction::ChallengerReveal => {
            process_challenger_reveal(accounts, data)
        }
        DlpV2Instruction::ResolveDispute => {
            process_resolve_dispute(accounts, data)
        }
        DlpV2Instruction::Delegate => process_delegate(accounts, data),
        DlpV2Instruction::CommitFinalize => {
            process_commit_finalize(accounts, data)
        }
        DlpV2Instruction::CommitFinalizeFromBuffer => {
            process_commit_finalize_from_buffer(accounts, data)
        }
        DlpV2Instruction::PrepareCommitFinalize => {
            process_prepare_commit_finalize(accounts, data)
        }
    }
}

pub(crate) fn state_buffer_payload_hash(payload: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b"magicblock.account_data.v1", payload])
        .to_bytes()
}
