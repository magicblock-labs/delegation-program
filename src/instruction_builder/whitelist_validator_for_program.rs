use borsh::BorshSerialize;
use solana_program::bpf_loader_upgradeable;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::WhitelistValidatorForProgramArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::program_config_pda_from_pubkey;

/// Whitelist validator for program
pub fn whitelist_validator_for_program(
    authority: Pubkey,
    validator_identity: Pubkey,
    program: Pubkey,
    insert: bool,
) -> Instruction {
    let args = WhitelistValidatorForProgramArgs { insert };
    let program_data =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::id()).0;
    let program_config = program_config_pda_from_pubkey(&program);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(validator_identity, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(program_data, false),
            AccountMeta::new(program_config, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::WhitelistValidatorForProgram.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}
