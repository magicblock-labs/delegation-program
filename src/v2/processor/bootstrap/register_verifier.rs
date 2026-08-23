use dlp_api::{
    error::DlpError,
    v2::{
        pda::{PROTOCOL_CONFIG_SEED, VERIFIER_BOND_SEED},
        ProtocolConfig, RegisterVerifierArgs, VerifierBond,
        VERIFIER_STATUS_ACTIVE,
    },
};
use pinocchio::{
    address::Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::{Decodable, Encodable},
    require_eq_keys, require_ge, require_n_accounts, require_ne,
    require_signer,
};

use crate::{
    processor::fast::utils::pda::create_pda,
    requires::{
        require_initialized_pda, require_uninitialized_pda, StandardCtx,
    },
};

/// Register one verifier for v2 approvals.
///
/// Accounts:
/// 0: `[signer, writable]` verifier identity and stake payer
/// 1: `[signer]`           protocol authority that admits the verifier
/// 2: `[writable]`         VerifierBond PDA
/// 3: `[]`                 ProtocolConfig PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_register_verifier(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        verifier, // force multi-line
        authority,
        verifier_bond,
        protocol_config,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    let args = RegisterVerifierArgs::decode(data)?;

    require_signer!(verifier);
    require_signer!(authority);

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
    if protocol_config_state.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        &Address::from(protocol_config_state.authority().to_bytes()),
        authority.address(),
        DlpError::InvalidAuthority
    );
    require_ne!(
        args.amount_lamports(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ge!(
        args.amount_lamports(),
        protocol_config_state.min_verifier_bond(),
        ProgramError::InvalidInstructionData
    );
    drop(protocol_config_data);

    let verifier_bond_bump = require_uninitialized_pda(
        verifier_bond,
        &[VERIFIER_BOND_SEED, verifier.address().as_ref()],
        &crate::fast::ID,
        true,
        StandardCtx::new("verifier bond"),
    )?;

    create_pda(
        verifier_bond,
        &crate::fast::ID,
        VerifierBond::DATA_LEN,
        &[Signer::from(&[
            Seed::from(VERIFIER_BOND_SEED),
            Seed::from(verifier.address().as_ref()),
            Seed::from(&[verifier_bond_bump]),
        ])],
        verifier,
    )?;

    system::Transfer {
        from: verifier,
        to: verifier_bond,
        lamports: args.amount_lamports(),
    }
    .invoke()?;

    VerifierBond {
        discriminator: VerifierBond::DISCRIMINATOR,
        verifier_identity: verifier.address().to_bytes().into(),
        stake_lamports: args.amount_lamports(),
        status: VERIFIER_STATUS_ACTIVE,
        registered_slot: Clock::get()?.slot,
        withdraw_requested_slot: None,
    }
    .encode_to(verifier_bond.try_borrow_mut()?.as_mut())?;

    Ok(())
}
