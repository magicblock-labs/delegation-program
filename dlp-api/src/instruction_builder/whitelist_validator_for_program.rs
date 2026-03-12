use borsh::to_vec;
use dlp::{
    args::WhitelistValidatorForProgramArgs, consts::DELEGATION_PROGRAM_DATA_ID,
    discriminator::DlpDiscriminator, pda::program_config_from_program_id,
};
use solana_program::{
    bpf_loader_upgradeable,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Whitelist validator for program
///
/// See [dlp::processor::process_whitelist_validator_for_program] for docs.
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
    let program_config_pda = program_config_from_program_id(&program);
    Instruction {
        program_id: dlp::id(),
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
