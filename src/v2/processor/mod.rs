mod bootstrap;
mod fraud_proofs;

use dlp_api::v2::DlpV2Instruction;

use pinocchio::{AccountView, ProgramResult};

pub use bootstrap::*;
pub use fraud_proofs::*;

#[inline(never)]
pub fn process_instruction(
    accounts: &[AccountView],
    data: &[u8],
    ix: DlpV2Instruction,
) -> ProgramResult {
    match ix {
        DlpV2Instruction::InitProtocolConfig => {
            process_init_protocol_config(accounts, data)
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
        DlpV2Instruction::UpdateProtocolConfig => {
            process_update_protocol_config(accounts, data)
        }
        DlpV2Instruction::PostCommitment => {
            process_post_commitment(accounts, data)
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
    }
}
