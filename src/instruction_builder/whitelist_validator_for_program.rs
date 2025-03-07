use borsh::to_vec;
use solana_program::bpf_loader_upgradeable;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::WhitelistValidatorForProgramArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::program_config_from_program_id;

/// Whitelist validator for program
///
/// See [crate::processor::process_whitelist_validator_for_program] for docs.
pub fn whitelist_validator_for_program(
    authority: Pubkey,
    validator_identity: Pubkey,
    program: Pubkey,
    insert: bool,
) -> Instruction {
    let args = WhitelistValidatorForProgramArgs { insert };
    let program_data =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::id()).0;
    let program_config_pda = program_config_from_program_id(&program);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(validator_identity, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(program_data, false),
            AccountMeta::new(program_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::WhitelistValidatorForProgram.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}
