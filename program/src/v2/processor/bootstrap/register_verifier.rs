use delegation_program_interface::{
    error::DlpError,
    v2::{
        pda::{DELEGATION_CLUSTER_SEED, VERIFIER_BOND_SEED},
        DelegationCluster, RegisterVerifierArgs, VerifierBond, VerifierStatus,
    },
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
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

/// Register one verifier for v2 approvals.
///
/// Accounts:
/// 0: `[signer, writable]` verifier identity and stake payer
/// 1: `[signer]`           cluster config authority
/// 2: `[writable]`         VerifierBond PDA
/// 3: `[]`                 DelegationCluster PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_register_verifier(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        verifier, // force multi-line
        config_authority,
        verifier_bond,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    require_signer!(verifier);
    require_signer!(config_authority);

    let args = RegisterVerifierArgs::decode(data)?;

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
        challenge_config.min_verifier_bond(),
        ProgramError::InvalidInstructionData
    );

    drop(delegation_cluster_data);

    let verifier_bond_bump = require_uninitialized_pda(
        verifier_bond,
        &[
            VERIFIER_BOND_SEED,
            delegation_cluster.address().as_ref(),
            verifier.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        StandardCtx::new("verifier bond"),
    )?;

    create_pda(
        verifier_bond,
        &crate::fast::ID,
        AccountSpace::CurrentRent(VerifierBond::DATA_LEN),
        &[Signer::from(&[
            Seed::from(VERIFIER_BOND_SEED),
            Seed::from(delegation_cluster.address().as_ref()),
            Seed::from(verifier.address().as_ref()),
            Seed::from(&[verifier_bond_bump]),
        ])],
        verifier,
    )?;

    system::Transfer {
        from: verifier,
        to: verifier_bond,
        lamports: args.stake_lamports(),
    }
    .invoke()?;

    VerifierBond {
        discriminator: VerifierBond::DISCRIMINATOR,
        bump: verifier_bond_bump,
        delegation_cluster: *delegation_cluster.address(),
        verifier_identity: verifier.address().to_bytes().into(),
        stake_lamports: args.stake_lamports(),
        status: VerifierStatus::Active.value(),
        registered_slot: Clock::get()?.slot,
        withdraw_requested_slot: None,
    }
    .encode_to(verifier_bond.try_borrow_mut()?.as_mut())?;

    Ok(())
}
