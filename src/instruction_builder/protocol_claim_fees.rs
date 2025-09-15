use solana_program::instruction::Instruction;
use solana_program::{bpf_loader_upgradeable, instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminator::DlpDiscriminator;
use crate::pda::{fees_vault_pda, program_config_from_program_id};

/// Claim the accrued fees from the protocol fees vault.
/// See [crate::processor::process_protocol_claim_fees] for docs.
pub fn protocol_claim_fees(admin: Pubkey, fees_receiver: Pubkey, program: Pubkey) -> Instruction {
    let fees_vault_pda = fees_vault_pda();
    let program_config_pda = program_config_from_program_id(&program);
    let delegation_program_data =
        Pubkey::find_program_address(&[crate::ID.as_ref()], &bpf_loader_upgradeable::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(program_config_pda, false),
            AccountMeta::new(fees_receiver, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(delegation_program_data, false),
        ],
        data: DlpDiscriminator::ProtocolClaimFees.to_vec(),
    }
}
