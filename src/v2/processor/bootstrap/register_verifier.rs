use dlp_api::{
    error::DlpError,
    v2::{
        pda::{PROTOCOL_CONFIG_SEED, VERIFIER_BOND_SEED},
        ProtocolConfig, RegisterVerifierArgs, VerifierBond,
        VERIFIER_STATUS_ACTIVE,
    },
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction as system_instruction;

use crate::{
    processor::utils::{
        loaders::{
            load_initialized_pda, load_program, load_signer,
            load_uninitialized_pda,
        },
        pda::create_pda,
    },
    solana_program::{
        account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult,
        program::invoke, program_error::ProgramError, pubkey::Pubkey,
        sysvar::Sysvar,
    },
};

/// Register one verifier for v2 approvals.
///
/// Accounts:
/// 0: `[signer, writable]` verifier identity and stake payer
/// 1: `[signer]`           protocol authority that admits the verifier
/// 2: `[writable]`         VerifierBond PDA
/// 3: `[]`                 ProtocolConfig PDA
/// 4: `[]`                 system program
pub fn process_register_verifier(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = RegisterVerifierArgs::try_from_bytes(data)?;

    let [verifier, authority, verifier_bond, protocol_config, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(verifier, "verifier")?;
    load_signer(authority, "authority")?;
    load_program(system_program, system_program::id(), "system program")?;

    load_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::id(),
        false,
        "protocol config",
    )?;

    let protocol_config_data = protocol_config.try_borrow_data()?;
    let protocol_config_state =
        ProtocolConfig::try_from_bytes_with_discriminator(
            protocol_config_data.as_ref(),
        )?;

    if protocol_config_state.authority != *authority.key {
        return Err(DlpError::InvalidAuthority.into());
    }

    if *verifier.key == Pubkey::default()
        || args.amount_lamports < protocol_config_state.min_verifier_bond
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    let verifier_bond_bump = load_uninitialized_pda(
        verifier_bond,
        &[VERIFIER_BOND_SEED, verifier.key.as_ref()],
        &crate::id(),
        true,
        "verifier bond",
    )?;

    create_pda(
        verifier_bond,
        &crate::id(),
        VerifierBond::SPACE,
        &[VERIFIER_BOND_SEED, verifier.key.as_ref()],
        verifier_bond_bump,
        system_program,
        verifier,
    )?;

    invoke(
        &system_instruction::transfer(
            verifier.key,
            verifier_bond.key,
            args.amount_lamports,
        ),
        &[
            verifier.clone(),
            verifier_bond.clone(),
            system_program.clone(),
        ],
    )?;

    let verifier_bond_state = VerifierBond {
        verifier_identity: *verifier.key,
        stake_lamports: args.amount_lamports,
        status: VERIFIER_STATUS_ACTIVE,
        registered_slot: Clock::get()?.slot,
        withdraw_requested_slot: None,
    };

    let mut verifier_bond_data = verifier_bond.try_borrow_mut_data()?;
    verifier_bond_state
        .to_bytes_with_discriminator(verifier_bond_data.as_mut())?;

    Ok(())
}
