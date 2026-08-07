use dlp_api::{
    error::DlpError,
    v2::{
        pda::{OPERATOR_BOND_SEED, PROTOCOL_CONFIG_SEED},
        OperatorBond, ProtocolConfig, RegisterOperatorArgs,
        OPERATOR_STATUS_ACTIVE,
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
        account_info::AccountInfo, entrypoint::ProgramResult, program::invoke,
        program_error::ProgramError, pubkey::Pubkey,
    },
};

/// Register one operator for v2 commitments.
///
/// Accounts:
/// 0: `[signer, writable]` operator identity and stake payer
/// 1: `[signer]`           protocol authority that admits the operator
/// 2: `[writable]`         OperatorBond PDA
/// 3: `[]`                 ProtocolConfig PDA
/// 4: `[]`                 system program
pub fn process_register_operator(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = RegisterOperatorArgs::try_from_bytes(data)?;

    let [operator, authority, operator_bond, protocol_config, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(operator, "operator")?;
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

    if *operator.key == Pubkey::default()
        || args.amount_lamports < protocol_config_state.min_operator_bond
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    let operator_bond_bump = load_uninitialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, operator.key.as_ref()],
        &crate::id(),
        true,
        "operator bond",
    )?;

    create_pda(
        operator_bond,
        &crate::id(),
        OperatorBond::SPACE,
        &[OPERATOR_BOND_SEED, operator.key.as_ref()],
        operator_bond_bump,
        system_program,
        operator,
    )?;

    invoke(
        &system_instruction::transfer(
            operator.key,
            operator_bond.key,
            args.amount_lamports,
        ),
        &[
            operator.clone(),
            operator_bond.clone(),
            system_program.clone(),
        ],
    )?;

    let operator_bond_state = OperatorBond {
        operator_identity: *operator.key,
        stake_lamports: args.amount_lamports,
        locked_lamports: 0,
        status: OPERATOR_STATUS_ACTIVE,
        withdraw_requested_slot: None,
    };

    let mut operator_bond_data = operator_bond.try_borrow_mut_data()?;
    operator_bond_state
        .to_bytes_with_discriminator(operator_bond_data.as_mut())?;

    Ok(())
}
