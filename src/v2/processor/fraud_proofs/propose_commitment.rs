use dlp_api::{
    error::DlpError,
    state::{DelegationMetadataRef, DelegationRecord, UndelegationRequester},
    v2::{
        pda::{
            DELEGATION_CLUSTER_SEED, OPERATOR_BOND_SEED,
            PENDING_COMMITMENT_SEED, STATE_BUFFER_SEED, VERIFIER_REGISTRY_SEED,
        },
        ChallengeConfigView, DelegationCluster, OperatorBond, OperatorStatus,
        PendingCommitment, ProposeCommitmentArgs, SelectedVerifier,
        StateBuffer, VerifierRegistry, PENDING_COMMITMENT_STATUS_ACTIVE,
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
    require_signer, require_some,
};

use crate::{
    processor::fast::{
        to_pinocchio_program_error,
        utils::pda::{create_pda, AccountSpace},
    },
    requires::{
        require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_initialized_pda,
        require_owned_pda, require_uninitialized_pda, StandardCtx,
    },
};

/// Propose one v2 account-state commitment.
///
/// Accounts:
/// 0: `[signer, writable]` current validator and pending account rent payer
/// 1: `[]`                 OperatorBond PDA
/// 2: `[writable]`         PendingCommitment PDA
/// 3: `[]`                 finalized StateBuffer PDA
/// 4: `[]`                 delegated account
/// 5: `[]`                 DelegationRecord PDA
/// 6: `[]`                 DelegationMetadata PDA
/// 7: `[]`                 DelegationCluster PDA
/// 8: `[writable]`         VerifierRegistry PDA
/// 9: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_propose_commitment(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        current_validator, // force multi-line
        operator_bond,
        pending_commitment,
        state_buffer,
        delegated_account,
        delegation_record,
        delegation_metadata,
        delegation_cluster,
        verifier_registry,
        _system_program,
    ] = require_n_accounts!(accounts, 10);

    let args = ProposeCommitmentArgs::decode(data)?;

    require_signer!(current_validator);
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;

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
        current_validator,
    )?;

    require_initialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, delegation_cluster.address().as_ref()],
        &crate::fast::ID,
        false,
        "operator bond",
    )?;
    let commit_id_bytes = args.commit_id().to_le_bytes();
    require_initialized_pda(
        state_buffer,
        &[
            STATE_BUFFER_SEED,
            delegated_account.address().as_ref(),
            &commit_id_bytes,
            current_validator.address().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "state buffer",
    )?;
    require_initialized_delegation_record(
        delegated_account,
        delegation_record,
        false,
    )?;
    require_initialized_delegation_metadata(
        delegated_account,
        delegation_metadata,
        false,
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

    let operator_bond_data = operator_bond.try_borrow()?;
    let operator_bond_state =
        OperatorBond::decode(operator_bond_data.as_ref())?;
    validate_operator_bond(&operator_bond_state, delegation_cluster)?;

    require_ge!(
        operator_bond_state.stake_lamports(),
        challenge_config.min_operator_bond(),
        ProgramError::InvalidInstructionData
    );
    drop(operator_bond_data);

    let state_buffer_data = state_buffer.try_borrow()?;
    let state_buffer_state = StateBuffer::decode(state_buffer_data.as_ref())?;
    let data_hash = validate_state_buffer(
        &state_buffer_state,
        current_validator,
        delegated_account,
        args.commit_id(),
    )?;
    drop(state_buffer_data);

    let delegation_record_data = delegation_record.try_borrow()?;
    let delegation_record_state =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;
    require_eq_keys!(
        &delegation_record_state.authority,
        delegation_cluster.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        &delegation_record_state.owner,
        args.owner(),
        ProgramError::InvalidAccountData
    );
    drop(delegation_record_data);

    let delegation_metadata =
        DelegationMetadataRef::from_account(delegation_metadata)?;
    require_eq!(
        args.commit_id(),
        delegation_metadata
            .last_commit_id()
            .checked_add(1)
            .ok_or(DlpError::Overflow)?,
        DlpError::NonceOutOfOrder
    );
    match delegation_metadata.undelegation_requester()? {
        UndelegationRequester::None => {}
        UndelegationRequester::Validator => {
            return Err(DlpError::AlreadyUndelegated.into());
        }
        UndelegationRequester::OwnerProgram => {
            return Err(DlpError::OwnerRequestedUndelegation.into());
        }
    }

    let verifier_registry_data = verifier_registry.try_borrow()?;
    let verifier_registry_view =
        VerifierRegistry::decode(verifier_registry_data.as_ref())?;
    if verifier_registry_view.discriminator() != VerifierRegistry::DISCRIMINATOR
    {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        verifier_registry_view.delegation_cluster(),
        delegation_cluster.address(),
        ProgramError::InvalidAccountData
    );

    let (selected_verifiers, scanned_count) = select_verifiers_round_robin(
        &verifier_registry_view,
        verifier_registry_view.next_selection_index(),
        challenge_config.verifiers_per_commitment() as usize,
        current_validator.address(),
        challenge_config.min_verifier_bond(),
    )?;

    require_ge!(
        selected_verifiers.len(),
        challenge_config.approval_threshold() as usize,
        ProgramError::InvalidInstructionData
    );

    let next_selection_index = verifier_registry_view
        .next_selection_index()
        .checked_add(scanned_count as u64)
        .ok_or(DlpError::Overflow)?;
    drop(verifier_registry_data);

    let clock = Clock::get()?;
    let challenge_window_id = 0;
    let account_state_hash =
        account_state_hash(args.lamports(), args.owner(), &data_hash);
    let state_commitment_hash =
        state_commitment_hash(StateCommitmentHashInput {
            delegation_cluster: delegation_cluster.address(),
            operator_identity: current_validator.address(),
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
        delegation_cluster: *delegation_cluster.address(),
        operator_identity: *current_validator.address(),
        operator_bond: operator_bond.address().to_bytes().into(),
        account_pubkey: delegated_account.address().to_bytes().into(),
        commit_id: args.commit_id(),
        delegation_record: delegation_record.address().to_bytes().into(),
        da_pointer_hash: *args.da_pointer_hash(),
        account_state_hash,
        data_hash,
        lamports: args.lamports(),
        owner: *args.owner(),
        state_commitment_hash,
        verifier_registry: verifier_registry.address().to_bytes().into(),
        challenge_window_id,
        posted_slot: clock.slot,
        activation_slot: clock.slot,
        challenge_window_end_slot: clock
            .slot
            .checked_add(challenge_config.challenge_window_slots())
            .ok_or(DlpError::Overflow)?,
        approval_count: 0,
        approval_threshold: challenge_config.approval_threshold(),
        resolver: *challenge_config.resolver(),
        min_challenger_stake: challenge_config.min_challenger_stake(),
        challenger_reveal_timeout_slots: challenge_config
            .challenger_reveal_timeout_slots(),
        match_penalty_bps: challenge_config.match_penalty_bps(),
        active_challenge: None,
        resolved_state_source: None,
        er_slot: args.er_slot(),
        _pad_before_selected_verifiers: [0; 5],
        selected_verifiers,
    };

    drop(delegation_cluster_data);

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
        AccountSpace::CurrentRent(pending_commitment_state.encoded_len()?),
        &[Signer::from(&[
            Seed::from(PENDING_COMMITMENT_SEED),
            Seed::from(delegated_account.address().as_ref()),
            Seed::from(&commit_id_bytes),
            Seed::from(&[pending_commitment_bump]),
        ])],
        current_validator,
    )?;

    pending_commitment_state
        .encode_to(pending_commitment.try_borrow_mut()?.as_mut())?;
    VerifierRegistry::decode_mut(verifier_registry)?
        .next_selection_index_mut()?
        .set(next_selection_index)?;

    Ok(())
}

fn validate_operator_bond(
    operator_bond: &dlp_api::v2::OperatorBondView<'_>,
    delegation_cluster: &AccountView,
) -> ProgramResult {
    if operator_bond.discriminator() != OperatorBond::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        operator_bond.delegation_cluster(),
        delegation_cluster.address(),
        ProgramError::InvalidAccountData
    );
    require_eq!(
        operator_bond.status(),
        OperatorStatus::Active.value(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        operator_bond.withdraw_requested_slot().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_delegation_cluster<'a>(
    delegation_cluster: &dlp_api::v2::DelegationClusterView<'a>,
    current_validator: &AccountView,
) -> Result<ChallengeConfigView<'a>, ProgramError> {
    if delegation_cluster.discriminator() != DelegationCluster::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        delegation_cluster.current_validator(),
        current_validator.address(),
        DlpError::InvalidAuthority
    );
    require_eq!(
        delegation_cluster.paused(),
        false,
        ProgramError::InvalidAccountData
    );

    Ok(require_some!(
        delegation_cluster.challenge_config(),
        ProgramError::InvalidAccountData
    ))
}

fn validate_state_buffer(
    state_buffer: &dlp_api::v2::StateBufferView<'_>,
    operator: &AccountView,
    delegated_account: &AccountView,
    commit_id: u64,
) -> Result<[u8; 32], ProgramError> {
    if state_buffer.discriminator() != StateBuffer::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        state_buffer.authority(),
        operator.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        state_buffer.account_pubkey(),
        delegated_account.address(),
        ProgramError::InvalidAccountData
    );
    require_eq!(
        state_buffer.commit_id(),
        commit_id,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state_buffer.finalized(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state_buffer.payload().len(),
        state_buffer.total_len() as usize,
        ProgramError::InvalidInstructionData
    );

    Ok(*state_buffer.data_hash())
}

fn select_verifiers_round_robin(
    registry: &dlp_api::v2::VerifierRegistryView<'_>,
    next_selection_index: u64,
    verifiers_per_commitment: usize,
    operator: &Address,
    min_verifier_bond: u64,
) -> Result<(Vec<SelectedVerifier>, usize), ProgramError> {
    let entries = registry.entries();
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
        let entry =
            entries.get(index).ok_or(ProgramError::InvalidAccountData)?;

        if entry.verifier_identity() != operator
            && entry.stake_lamports() >= min_verifier_bond
        {
            selected.push(SelectedVerifier {
                verifier_identity: *entry.verifier_identity(),
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
    delegation_cluster: &'a Address,
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
        input.delegation_cluster.as_ref(),
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
