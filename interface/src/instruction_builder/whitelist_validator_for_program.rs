use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use crate::{
    args::WhitelistValidatorForProgramArgs,
    compat::{borsh::to_vec, Compatize, Modernize},
    consts::DELEGATION_PROGRAM_DATA_ID,
    discriminator::DlpDiscriminator,
    pda::program_config_from_program_id,
};

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
    let program_data = Pubkey::find_program_address(
        &[program.as_ref()],
        &bpf_loader_upgradeable::id(),
    )
    .0;
    let delegation_program_data = DELEGATION_PROGRAM_DATA_ID.to_bytes().into();
    let program_compat = program.compatize();
    let program_config_pda =
        program_config_from_program_id(&program_compat).modernize();
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(validator_identity, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(program_data, false),
            AccountMeta::new_readonly(delegation_program_data, false),
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
