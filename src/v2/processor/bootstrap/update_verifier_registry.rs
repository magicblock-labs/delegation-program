use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            PROTOCOL_CONFIG_SEED, VERIFIER_BOND_SEED, VERIFIER_REGISTRY_SEED,
        },
        ProtocolConfig, ProtocolConfigView, UpdateVerifierRegistryArgs,
        UpdateVerifierRegistryArgsView, VerifierBond, VerifierBondView,
        VerifierRegistry, VerifierRegistryEntry, VERIFIER_REGISTRY_ACTION_ADD,
        VERIFIER_STATUS_ACTIVE,
    },
};
use pinocchio::{
    address::Address, error::ProgramError, AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_eq_keys, require_ge, require_n_accounts, require_ne,
    require_signer,
};

use crate::{
    processor::fast::utils::pda::resize_pda,
    requires::{require_initialized_pda, require_owned_pda},
};

/// Update the verifier registry used by v2 verifier selection.
///
/// Accounts:
/// 0: `[signer, writable]` protocol authority and registry rent payer
/// 1: `[]`                 ProtocolConfig PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 VerifierBond PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_update_verifier_registry(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        authority, // force multi-line
        protocol_config,
        verifier_registry,
        verifier_bond,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    let args = UpdateVerifierRegistryArgs::decode(data)?;

    require_signer!(authority);

    require_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        false,
        "protocol config",
    )?;
    require_initialized_pda(
        verifier_registry,
        &[VERIFIER_REGISTRY_SEED],
        &crate::fast::ID,
        true,
        "verifier registry",
    )?;
    require_owned_pda(verifier_bond, &crate::fast::ID, "verifier bond")?;

    let protocol_config_data = protocol_config.try_borrow()?;
    let protocol_config_state =
        ProtocolConfig::decode(protocol_config_data.as_ref())?;
    validate_protocol_config(&protocol_config_state, authority)?;

    let verifier_bond_data = verifier_bond.try_borrow()?;
    let verifier_bond_state =
        VerifierBond::decode(verifier_bond_data.as_ref())?;
    validate_verifier_bond(&verifier_bond_state, verifier_bond)?;

    if args.action() != VERIFIER_REGISTRY_ACTION_ADD {
        // CHECKPOINT: implement `VERIFIER_REGISTRY_ACTION_REMOVE` when
        // withdrawal/removal rules are finalized.
        return Err(ProgramError::InvalidInstructionData);
    }

    validate_add_args(&args, &protocol_config_state, &verifier_bond_state)?;
    drop(protocol_config_data);

    let verifier_identity = *verifier_bond_state.verifier_identity();
    drop(verifier_bond_data);

    let verifier_registry_data = verifier_registry.try_borrow()?;
    let verifier_registry_view =
        VerifierRegistry::decode(verifier_registry_data.as_ref())?;
    if verifier_registry_view.discriminator() != VerifierRegistry::DISCRIMINATOR
    {
        return Err(ProgramError::InvalidAccountData);
    }

    let verifier_bond_key = verifier_bond.address().to_bytes().into();
    let mut entries =
        Vec::with_capacity(verifier_registry_view.entries().len() + 1);
    for entry in verifier_registry_view.entries().iter() {
        if *entry.verifier_identity() == verifier_identity
            || *entry.verifier_bond() == verifier_bond_key
        {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        entries.push(VerifierRegistryEntry {
            verifier_identity: *entry.verifier_identity(),
            verifier_bond: *entry.verifier_bond(),
            weight: entry.weight(),
        });
    }

    entries.push(VerifierRegistryEntry {
        verifier_identity,
        verifier_bond: verifier_bond_key,
        weight: args.weight(),
    });

    let updated_registry = VerifierRegistry {
        discriminator: VerifierRegistry::DISCRIMINATOR,
        registry_revision: verifier_registry_view
            .registry_revision()
            .checked_add(1)
            .ok_or(DlpError::Overflow)?,
        next_selection_index: verifier_registry_view.next_selection_index(),
        entries,
    };
    drop(verifier_registry_data);

    // CHECKPOINT: before registry size grows beyond bootstrap needs, define a
    // max entry count or switch to a paged/Merkle registry.
    resize_pda(
        authority,
        verifier_registry,
        updated_registry.encoded_len()?,
    )?;
    updated_registry.encode_to(verifier_registry.try_borrow_mut()?.as_mut())?;

    Ok(())
}

fn validate_protocol_config(
    protocol_config: &ProtocolConfigView<'_>,
    authority: &AccountView,
) -> ProgramResult {
    if protocol_config.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        &Address::from(protocol_config.authority().to_bytes()),
        authority.address(),
        DlpError::InvalidAuthority
    );

    Ok(())
}

fn validate_verifier_bond(
    verifier_bond: &VerifierBondView<'_>,
    verifier_bond_account: &AccountView,
) -> ProgramResult {
    if verifier_bond.discriminator() != VerifierBond::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_initialized_pda(
        verifier_bond_account,
        &[
            VERIFIER_BOND_SEED,
            verifier_bond.verifier_identity().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "verifier bond",
    )?;

    Ok(())
}

fn validate_add_args(
    args: &UpdateVerifierRegistryArgsView<'_>,
    protocol_config: &ProtocolConfigView<'_>,
    verifier_bond: &VerifierBondView<'_>,
) -> ProgramResult {
    require_ne!(args.weight(), 0, ProgramError::InvalidInstructionData);
    require_eq!(
        verifier_bond.status(),
        VERIFIER_STATUS_ACTIVE,
        ProgramError::InvalidInstructionData
    );
    require_ge!(
        verifier_bond.stake_lamports(),
        protocol_config.min_verifier_bond(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        verifier_bond.withdraw_requested_slot().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
