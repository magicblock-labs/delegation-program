use dlp_api::{
    error::DlpError,
    v2::{
        pda::{OPERATOR_BOND_SEED, PROTOCOL_CONFIG_SEED},
        OperatorBond, OperatorStatus, ProtocolConfig, RegisterOperatorArgs,
    },
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::{Decodable, Encodable},
    require, require_eq_keys, require_ge, require_n_accounts, require_signer,
};

use crate::{
    processor::fast::utils::pda::create_pda,
    requires::{
        require_initialized_pda, require_uninitialized_pda, StandardCtx,
    },
};

/// Register one operator for v2 commitments.
///
/// Accounts:
/// 0: `[signer, writable]` operator identity and stake payer
/// 1: `[signer]`           protocol authority that admits the operator
/// 2: `[writable]`         OperatorBond PDA
/// 3: `[]`                 ProtocolConfig PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_register_operator(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        operator, // force multi-line
        authority,
        operator_bond,
        protocol_config,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    require_signer!(operator);
    require_signer!(authority);

    let args = RegisterOperatorArgs::decode(data)?;

    require_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        false,
        "protocol config",
    )?;
    let protocol_config_data = protocol_config.try_borrow()?;
    let protocol_config_state =
        ProtocolConfig::decode(protocol_config_data.as_ref())?;
    require!(
        protocol_config_state.discriminator() == ProtocolConfig::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );

    require_eq_keys!(
        protocol_config_state.authority(),
        authority.address(),
        DlpError::InvalidAuthority
    );
    require_ge!(
        args.stake_lamports(),
        protocol_config_state.min_operator_bond(),
        ProgramError::InvalidInstructionData
    );

    drop(protocol_config_data);

    let operator_bond_bump = require_uninitialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, operator.address().as_ref()],
        &crate::fast::ID,
        true,
        StandardCtx::new("operator bond"),
    )?;

    create_pda(
        operator_bond,
        &crate::fast::ID,
        OperatorBond::DATA_LEN,
        &[Signer::from(&[
            Seed::from(OPERATOR_BOND_SEED),
            Seed::from(operator.address().as_ref()),
            Seed::from(&[operator_bond_bump]),
        ])],
        operator,
    )?;

    system::Transfer {
        from: operator,
        to: operator_bond,
        lamports: args.stake_lamports(),
    }
    .invoke()?;

    OperatorBond {
        discriminator: OperatorBond::DISCRIMINATOR,
        bump: operator_bond_bump,
        operator_identity: operator.address().to_bytes().into(),
        stake_lamports: args.stake_lamports(),
        locked_lamports: 0,
        status: OperatorStatus::Active.value(),
        withdraw_requested_slot: None,
    }
    .encode_to(operator_bond.try_borrow_mut()?.as_mut())?;

    Ok(())
}
