use delegation_program_interface::{
    error::DlpError,
    v2::{
        pda::{DELEGATION_CLUSTER_SEED, OPERATOR_BOND_SEED},
        DelegationCluster, OperatorBond, OperatorStatus, RegisterOperatorArgs,
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
    require_some,
};

use crate::{
    processor::fast::utils::pda::{create_pda, AccountSpace},
    requires::{
        require_initialized_pda, require_uninitialized_pda, StandardCtx,
    },
};

/// Register one operator for v2 commitments.
///
/// Accounts:
/// 0: `[signer, writable]` current validator and stake payer
/// 1: `[signer]`           cluster config authority
/// 2: `[writable]`         OperatorBond PDA
/// 3: `[]`                 DelegationCluster PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_register_operator(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        current_validator, // force multi-line
        config_authority,
        operator_bond,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    require_signer!(current_validator);
    require_signer!(config_authority);

    let args = RegisterOperatorArgs::decode(data)?;

    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let delegation_cluster_state =
        DelegationCluster::decode(delegation_cluster_data.as_ref())?;
    require_initialized_pda(
        delegation_cluster,
        &[
            DELEGATION_CLUSTER_SEED,
            delegation_cluster_state.cluster_id().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "delegation cluster",
    )?;
    require!(
        delegation_cluster_state.discriminator()
            == DelegationCluster::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        delegation_cluster_state.current_validator(),
        current_validator.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        delegation_cluster_state.config_authority(),
        config_authority.address(),
        DlpError::InvalidAuthority
    );
    require!(
        !delegation_cluster_state.paused(),
        ProgramError::InvalidAccountData
    );
    let challenge_config = require_some!(
        delegation_cluster_state.challenge_config(),
        ProgramError::InvalidAccountData
    );
    require_ge!(
        args.stake_lamports(),
        challenge_config.min_operator_bond(),
        ProgramError::InvalidInstructionData
    );

    drop(delegation_cluster_data);

    let operator_bond_bump = require_uninitialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, delegation_cluster.address().as_ref()],
        &crate::fast::ID,
        true,
        StandardCtx::new("operator bond"),
    )?;

    create_pda(
        operator_bond,
        &crate::fast::ID,
        AccountSpace::CurrentRent(OperatorBond::DATA_LEN),
        &[Signer::from(&[
            Seed::from(OPERATOR_BOND_SEED),
            Seed::from(delegation_cluster.address().as_ref()),
            Seed::from(&[operator_bond_bump]),
        ])],
        current_validator,
    )?;

    system::Transfer {
        from: current_validator,
        to: operator_bond,
        lamports: args.stake_lamports(),
    }
    .invoke()?;

    OperatorBond {
        discriminator: OperatorBond::DISCRIMINATOR,
        bump: operator_bond_bump,
        delegation_cluster: *delegation_cluster.address(),
        stake_lamports: args.stake_lamports(),
        locked_lamports: 0,
        status: OperatorStatus::Active.value(),
        withdraw_requested_slot: None,
    }
    .encode_to(operator_bond.try_borrow_mut()?.as_mut())?;

    Ok(())
}
