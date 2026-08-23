use dlp_api::{
    error::DlpError,
    state::DelegationRecord,
    v2::{
        pda::{
            OPERATOR_BOND_SEED, PENDING_COMMITMENT_SEED, PROTOCOL_CONFIG_SEED,
            VERIFIER_REGISTRY_SEED,
        },
        OperatorBond, PendingCommitment, PostCommitmentArgs, ProtocolConfig,
        SelectedVerifier, VerifierRegistry, VerifierRegistryEntry,
        OPERATOR_STATUS_ACTIVE, PENDING_COMMITMENT_STATUS_ACTIVE,
    },
};
use pinocchio::{
    address::Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_eq_keys, require_ge, require_n_accounts,
    require_signer,
};

use crate::{
    processor::fast::{to_pinocchio_program_error, utils::pda::create_pda},
    requires::{
        require_initialized_delegation_record, require_initialized_pda,
        require_owned_pda, require_uninitialized_pda, StandardCtx,
    },
};

/// Post one v2 account-state commitment.
///
/// Accounts:
/// 0: `[signer, writable]` operator identity and pending account rent payer
/// 1: `[]`                 OperatorBond PDA
/// 2: `[writable]`         PendingCommitment PDA
/// 3: `[]`                 delegated account
/// 4: `[]`                 DelegationRecord PDA
/// 5: `[]`                 ProtocolConfig PDA
/// 6: `[writable]`         VerifierRegistry PDA
/// 7: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_post_commitment(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        operator, // force multi-line
        operator_bond,
        pending_commitment,
        delegated_account,
        delegation_record,
        protocol_config,
        verifier_registry,
        _system_program,
    ] = require_n_accounts!(accounts, 8);

    let args = PostCommitmentArgs::decode(data)?;

    require_signer!(operator);
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;

    require_initialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, operator.address().as_ref()],
        &crate::fast::ID,
        false,
        "operator bond",
    )?;
    require_initialized_delegation_record(
        delegated_account,
        delegation_record,
        false,
    )?;
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

    let operator_bond_data = operator_bond.try_borrow()?;
    let operator_bond_state =
        OperatorBond::decode(operator_bond_data.as_ref())?;
    validate_operator_bond(&operator_bond_state, operator)?;

    let protocol_config_data = protocol_config.try_borrow()?;
    let protocol_config_state =
        ProtocolConfig::decode(protocol_config_data.as_ref())?;
    validate_protocol_config(&protocol_config_state)?;

    require_ge!(
        operator_bond_state.stake_lamports(),
        protocol_config_state.min_operator_bond(),
        ProgramError::InvalidInstructionData
    );
    drop(operator_bond_data);

    let delegation_record_data = delegation_record.try_borrow()?;
    let delegation_record_state =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;
    require_eq_keys!(
        &Address::from(delegation_record_state.authority.to_bytes()),
        operator.address(),
        DlpError::InvalidAuthority
    );
    drop(delegation_record_data);

    let verifier_registry_data = verifier_registry.try_borrow()?;
    let verifier_registry_view =
        VerifierRegistry::decode(verifier_registry_data.as_ref())?;
    if verifier_registry_view.discriminator() != VerifierRegistry::DISCRIMINATOR
    {
        return Err(ProgramError::InvalidAccountData);
    }

    let entries = copy_verifier_entries(&verifier_registry_view);
    let (selected_verifiers, scanned_count) = select_verifiers_round_robin(
        &entries,
        verifier_registry_view.next_selection_index(),
        protocol_config_state.verifiers_per_commitment() as usize,
        operator.address(),
    )?;

    require_ge!(
        selected_verifiers.len(),
        protocol_config_state.approval_threshold() as usize,
        ProgramError::InvalidInstructionData
    );

    let updated_registry = VerifierRegistry {
        discriminator: VerifierRegistry::DISCRIMINATOR,
        registry_revision: verifier_registry_view.registry_revision(),
        next_selection_index: verifier_registry_view
            .next_selection_index()
            .checked_add(scanned_count as u64)
            .ok_or(DlpError::Overflow)?,
        entries,
    };
    drop(verifier_registry_data);

    let clock = Clock::get()?;
    let challenge_window_id = 0;
    let account_state_hash =
        account_state_hash(args.lamports(), args.owner(), args.data_hash());
    let state_commitment_hash =
        state_commitment_hash(StateCommitmentHashInput {
            operator_identity: operator.address(),
            account_pubkey: delegated_account.address(),
            commit_id: args.commit_id(),
            delegation_record: delegation_record.address(),
            da_pointer_hash: args.da_pointer_hash(),
            account_state_hash: &account_state_hash,
            verifier_registry: verifier_registry.address(),
            challenge_window_id,
        });

    let pending_commitment_state = PendingCommitment {
        discriminator: PendingCommitment::DISCRIMINATOR,
        status: PENDING_COMMITMENT_STATUS_ACTIVE,
        operator_identity: operator.address().to_bytes().into(),
        operator_bond: operator_bond.address().to_bytes().into(),
        account_pubkey: delegated_account.address().to_bytes().into(),
        commit_id: args.commit_id(),
        delegation_record: delegation_record.address().to_bytes().into(),
        da_pointer_hash: *args.da_pointer_hash(),
        account_state_hash,
        data_hash: *args.data_hash(),
        lamports: args.lamports(),
        owner: *args.owner(),
        state_commitment_hash,
        verifier_registry: verifier_registry.address().to_bytes().into(),
        verifier_registry_revision: updated_registry.registry_revision,
        challenge_window_id,
        posted_slot: clock.slot,
        activation_slot: clock.slot,
        challenge_window_end_slot: clock
            .slot
            .checked_add(protocol_config_state.challenge_window_slots())
            .ok_or(DlpError::Overflow)?,
        approval_count: 0,
        approval_threshold: protocol_config_state.approval_threshold(),
        active_challenge: None,
        resolved_state_source: None,
        er_slot: args.er_slot(),
        _pad_before_selected_verifiers: [0; 7],
        selected_verifiers,
    };

    drop(protocol_config_data);

    let commit_id_bytes = args.commit_id().to_le_bytes();
    let pending_commitment_bump = require_uninitialized_pda(
        pending_commitment,
        &[
            PENDING_COMMITMENT_SEED,
            delegated_account.address().as_ref(),
            &commit_id_bytes,
        ],
        &crate::fast::ID,
        true,
        StandardCtx::new("pending commitment"),
    )?;

    create_pda(
        pending_commitment,
        &crate::fast::ID,
        pending_commitment_state.encoded_len()?,
        &[Signer::from(&[
            Seed::from(PENDING_COMMITMENT_SEED),
            Seed::from(delegated_account.address().as_ref()),
            Seed::from(&commit_id_bytes),
            Seed::from(&[pending_commitment_bump]),
        ])],
        operator,
    )?;

    pending_commitment_state
        .encode_to(pending_commitment.try_borrow_mut()?.as_mut())?;
    updated_registry.encode_to(verifier_registry.try_borrow_mut()?.as_mut())?;

    Ok(())
}

fn validate_operator_bond(
    operator_bond: &dlp_api::v2::OperatorBondView<'_>,
    operator: &AccountView,
) -> ProgramResult {
    if operator_bond.discriminator() != OperatorBond::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        &Address::from(operator_bond.operator_identity().to_bytes()),
        operator.address(),
        DlpError::InvalidAuthority
    );
    require_eq!(
        operator_bond.status(),
        OPERATOR_STATUS_ACTIVE,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        operator_bond.withdraw_requested_slot().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_protocol_config(
    protocol_config: &dlp_api::v2::ProtocolConfigView<'_>,
) -> ProgramResult {
    if protocol_config.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq!(
        protocol_config.paused(),
        false,
        ProgramError::InvalidAccountData
    );

    Ok(())
}

fn copy_verifier_entries(
    registry: &dlp_api::v2::VerifierRegistryView<'_>,
) -> Vec<VerifierRegistryEntry> {
    registry
        .entries()
        .iter()
        .map(|entry| VerifierRegistryEntry {
            verifier_identity: *entry.verifier_identity(),
            verifier_bond: *entry.verifier_bond(),
            weight: entry.weight(),
        })
        .collect()
}

fn select_verifiers_round_robin(
    entries: &[VerifierRegistryEntry],
    next_selection_index: u64,
    verifiers_per_commitment: usize,
    operator: &Address,
) -> Result<(Vec<SelectedVerifier>, usize), ProgramError> {
    if entries.is_empty() || verifiers_per_commitment == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let start = (next_selection_index % entries.len() as u64) as usize;
    let mut selected = Vec::new();
    let mut scanned_count = 0;

    while scanned_count < entries.len()
        && selected.len() < verifiers_per_commitment
    {
        let index = (start + scanned_count) % entries.len();
        let entry = &entries[index];

        if Address::from(entry.verifier_identity.to_bytes()) != *operator {
            selected.push(SelectedVerifier {
                verifier_identity: entry.verifier_identity,
                approved: false,
                _pad_after_approved: [0; 7],
            });
        }

        scanned_count += 1;
    }

    if selected.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    Ok((selected, scanned_count))
}

fn account_state_hash(
    lamports: u64,
    owner: &dlp_api::compat::Pubkey,
    data_hash: &[u8; 32],
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.account_state.v1",
        &lamports.to_le_bytes(),
        owner.as_ref(),
        data_hash,
    ])
    .to_bytes()
}

struct StateCommitmentHashInput<'a> {
    operator_identity: &'a Address,
    account_pubkey: &'a Address,
    commit_id: u64,
    delegation_record: &'a Address,
    da_pointer_hash: &'a [u8; 32],
    account_state_hash: &'a [u8; 32],
    verifier_registry: &'a Address,
    challenge_window_id: u64,
}

fn state_commitment_hash(input: StateCommitmentHashInput<'_>) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.state_commitment.v1",
        input.operator_identity.as_ref(),
        input.account_pubkey.as_ref(),
        &input.commit_id.to_le_bytes(),
        input.delegation_record.as_ref(),
        input.da_pointer_hash,
        input.account_state_hash,
        input.verifier_registry.as_ref(),
        &input.challenge_window_id.to_le_bytes(),
    ])
    .to_bytes()
}
