mod fraud_proofs;
mod init_protocol_config;

use dlp_api::v2::DlpV2Instruction;

use crate::solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey,
};

pub use init_protocol_config::*;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
    ix: DlpV2Instruction,
) -> ProgramResult {
    match ix {
        DlpV2Instruction::InitProtocolConfig => {
            process_init_protocol_config(program_id, accounts, data)
        }
    }
}
