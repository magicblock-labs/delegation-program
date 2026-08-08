use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            PROTOCOL_CONFIG_SEED, VERIFIER_BOND_SEED, VERIFIER_REGISTRY_SEED,
        },
        ProtocolConfig, UpdateVerifierRegistryArgs, VerifierBond,
        VerifierRegistry, VerifierRegistryEntry, VERIFIER_REGISTRY_ACTION_ADD,
        VERIFIER_STATUS_ACTIVE,
    },
};
use solana_sdk_ids::system_program;

use crate::{
    processor::utils::{
        loaders::{
            load_initialized_pda, load_owned_pda, load_program, load_signer,
        },
        pda::resize_pda,
    },
    solana_program::{
        account_info::AccountInfo, entrypoint::ProgramResult,
        program_error::ProgramError, pubkey::Pubkey,
    },
};

/// Update the verifier registry used by v2 verifier selection.
///
/// Accounts:
/// 0: `[signer, writable]` protocol authority and registry rent payer
/// 1: `[]`                 ProtocolConfig PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 VerifierBond PDA
/// 4: `[]`                 system program
pub fn process_update_verifier_registry(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = UpdateVerifierRegistryArgs::try_from_bytes(data)?;

    let [authority, protocol_config, verifier_registry, verifier_bond, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(authority, "authority")?;
    load_program(system_program, system_program::id(), "system program")?;

    load_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::id(),
        false,
        "protocol config",
    )?;
    load_initialized_pda(
        verifier_registry,
        &[VERIFIER_REGISTRY_SEED],
        &crate::id(),
        true,
        "verifier registry",
    )?;
    load_owned_pda(verifier_bond, &crate::id(), "verifier bond")?;

    let protocol_config_data = protocol_config.try_borrow_data()?;
    let protocol_config_state =
        ProtocolConfig::try_from_bytes_with_discriminator(
            protocol_config_data.as_ref(),
        )?;
    if protocol_config_state.authority != *authority.key {
        return Err(DlpError::InvalidAuthority.into());
    }

    let verifier_bond_data = verifier_bond.try_borrow_data()?;
    let verifier_bond_state = VerifierBond::try_from_bytes_with_discriminator(
        verifier_bond_data.as_ref(),
    )?;
    drop(verifier_bond_data);

    load_initialized_pda(
        verifier_bond,
        &[
            VERIFIER_BOND_SEED,
            verifier_bond_state.verifier_identity.as_ref(),
        ],
        &crate::id(),
        false,
        "verifier bond",
    )?;

    if args.action != VERIFIER_REGISTRY_ACTION_ADD {
        // CHECKPOINT: implement `VERIFIER_REGISTRY_ACTION_REMOVE` when
        // withdrawal/removal rules are finalized.
        return Err(ProgramError::InvalidInstructionData);
    }

    validate_add_args(&args, &protocol_config_state, &verifier_bond_state)?;

    let verifier_registry_data = verifier_registry.try_borrow_data()?;
    let mut verifier_registry_state =
        VerifierRegistry::try_from_bytes_with_discriminator(
            verifier_registry_data.as_ref(),
        )?;
    drop(verifier_registry_data);

    if verifier_registry_state.entries.iter().any(|entry| {
        entry.verifier_identity == verifier_bond_state.verifier_identity
            || entry.verifier_bond == *verifier_bond.key
    }) {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    verifier_registry_state.entries.push(VerifierRegistryEntry {
        verifier_identity: verifier_bond_state.verifier_identity,
        verifier_bond: *verifier_bond.key,
        weight: args.weight,
    });
    verifier_registry_state.registry_revision = verifier_registry_state
        .registry_revision
        .checked_add(1)
        .ok_or(DlpError::Overflow)?;

    // CHECKPOINT: before registry size grows beyond bootstrap needs, define a
    // max entry count or switch to a paged/Merkle registry.
    resize_pda(
        authority,
        verifier_registry,
        system_program,
        verifier_registry_state.size_with_discriminator(),
    )?;

    let mut verifier_registry_data = verifier_registry.try_borrow_mut_data()?;
    verifier_registry_state
        .to_bytes_with_discriminator(verifier_registry_data.as_mut())?;

    Ok(())
}

fn validate_add_args(
    args: &UpdateVerifierRegistryArgs,
    protocol_config: &ProtocolConfig,
    verifier_bond: &VerifierBond,
) -> ProgramResult {
    if args.weight == 0
        || verifier_bond.status != VERIFIER_STATUS_ACTIVE
        || verifier_bond.stake_lamports < protocol_config.min_verifier_bond
        || verifier_bond.withdraw_requested_slot.is_some()
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    Ok(())
}
