use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            DELEGATION_CLUSTER_SEED, VERIFIER_BOND_SEED, VERIFIER_REGISTRY_SEED,
        },
        ChallengeConfigView, DelegationCluster, DelegationClusterView,
        UpdateVerifierRegistryArgs, UpdateVerifierRegistryArgsView,
        VerifierBond, VerifierBondView, VerifierRegistry,
        VerifierRegistryAction, VerifierRegistryEntry, VerifierStatus,
    },
};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys, require_ge,
    require_n_accounts, require_signer, require_some,
};

use crate::{
    processor::fast::utils::pda::top_up_pda_rent,
    requires::{require_initialized_pda, require_owned_pda, require_pda},
};

/// Update the verifier registry used by v2 verifier selection.
///
/// Accounts:
/// 0: `[signer, writable]` cluster config authority and registry rent payer
/// 1: `[]`                 DelegationCluster PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 VerifierBond PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_update_verifier_registry(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        config_authority, // force multi-line
        delegation_cluster,
        verifier_registry,
        verifier_bond,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    require_signer!(config_authority);

    let args = UpdateVerifierRegistryArgs::decode(data)?;
    validate_update_args(&args)?;

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
    let challenge_config = validate_delegation_cluster(
        &delegation_cluster_state,
        config_authority,
    )?;
    require_initialized_pda(
        verifier_registry,
        &[
            VERIFIER_REGISTRY_SEED,
            delegation_cluster.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        "verifier registry",
    )?;
    require_owned_pda(verifier_bond, &crate::fast::ID, "verifier bond")?;

    let verifier_bond_data = verifier_bond.try_borrow()?;
    let verifier_bond_state =
        VerifierBond::decode(verifier_bond_data.as_ref())?;
    validate_verifier_bond(
        &verifier_bond_state,
        verifier_bond,
        delegation_cluster,
    )?;
    validate_verifier_can_be_added(&challenge_config, &verifier_bond_state)?;

    let verifier_identity = verifier_bond_state.verifier_identity();

    {
        let verifier_registry_data = verifier_registry.try_borrow()?;
        let verifier_registry_view =
            VerifierRegistry::decode(verifier_registry_data.as_ref())?;
        require!(
            verifier_registry_view.discriminator()
                == VerifierRegistry::DISCRIMINATOR,
            ProgramError::InvalidAccountData
        );
        require_eq_keys!(
            verifier_registry_view.delegation_cluster(),
            delegation_cluster.address(),
            ProgramError::InvalidAccountData
        );
        require!(
            !verifier_registry_view
                .entries()
                .iter()
                .any(|entry| entry.verifier_identity() == verifier_identity),
            ProgramError::AccountAlreadyInitialized
        );
    }

    // CHECKPOINT: this single-PDA Vec is only suitable while the verifier set
    // is small. Before allowing unbounded growth, cap the entry count or
    // replace this with paged storage / Merkle-root based membership.
    let old_registry_len = verifier_registry.data_len();
    let mut verifier_registry_state =
        VerifierRegistry::decode_mut(verifier_registry)?;

    verifier_registry_state
        .entries_mut()?
        .push(&VerifierRegistryEntry {
            verifier_identity: *verifier_identity,
            stake_lamports: verifier_bond_state.stake_lamports(),
            weight: args.weight(),
        })?;

    let new_registry_len = verifier_registry.data_len();
    if new_registry_len > old_registry_len {
        top_up_pda_rent(config_authority, verifier_registry, new_registry_len)?;
    }

    Ok(())
}

fn validate_update_args(
    args: &UpdateVerifierRegistryArgsView<'_>,
) -> ProgramResult {
    // CHECKPOINT: implement `VerifierRegistryAction::Remove` when
    // withdrawal/removal rules are finalized.
    require!(
        args.action() == VerifierRegistryAction::Add.value(),
        ProgramError::InvalidInstructionData
    );
    // MVP verifier selection is equal-weight round-robin, so the only
    // meaningful weight until weighted selection exists is 1.
    require_eq!(args.weight(), 1_u64, ProgramError::InvalidInstructionData);

    Ok(())
}

fn validate_delegation_cluster<'a>(
    delegation_cluster: &DelegationClusterView<'a>,
    config_authority: &AccountView,
) -> Result<ChallengeConfigView<'a>, ProgramError> {
    require!(
        delegation_cluster.discriminator() == DelegationCluster::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        delegation_cluster.config_authority(),
        config_authority.address(),
        DlpError::InvalidAuthority
    );
    require!(
        !delegation_cluster.paused(),
        ProgramError::InvalidAccountData
    );

    Ok(require_some!(
        delegation_cluster.challenge_config(),
        ProgramError::InvalidAccountData
    ))
}

fn validate_verifier_bond(
    verifier_bond: &VerifierBondView<'_>,
    verifier_bond_account: &AccountView,
    delegation_cluster: &AccountView,
) -> ProgramResult {
    require!(
        verifier_bond.discriminator() == VerifierBond::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_pda(
        verifier_bond_account,
        &[
            VERIFIER_BOND_SEED,
            delegation_cluster.address().as_ref(),
            verifier_bond.verifier_identity().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "verifier bond",
    )?;
    require_eq_keys!(
        verifier_bond.delegation_cluster(),
        delegation_cluster.address(),
        ProgramError::InvalidAccountData
    );

    Ok(())
}

fn validate_verifier_can_be_added(
    challenge_config: &ChallengeConfigView<'_>,
    verifier_bond: &VerifierBondView<'_>,
) -> ProgramResult {
    require_eq!(
        verifier_bond.status(),
        VerifierStatus::Active.value(),
        ProgramError::InvalidInstructionData
    );
    require_ge!(
        verifier_bond.stake_lamports(),
        challenge_config.min_verifier_bond(),
        ProgramError::InvalidInstructionData
    );
    require!(
        verifier_bond.withdraw_requested_slot().is_none(),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
