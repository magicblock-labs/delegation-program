use dlp_api::{
    error::DlpError,
    v2::{
        pda::{PENDING_COMMITMENT_SEED, VERIFIER_BOND_SEED},
        PendingCommitment, SelectedVerifier, VerifierBond,
        PENDING_COMMITMENT_STATUS_ACTIVE, VERIFIER_STATUS_ACTIVE,
    },
};
use pinocchio::{
    address::Address,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_eq_keys, require_le, require_n_accounts,
    require_signer,
};

use crate::requires::{require_initialized_pda, require_owned_pda};

/// Approve one v2 account-state commitment.
///
/// Accounts:
/// 0: `[signer]`   selected verifier identity
/// 1: `[]`         VerifierBond PDA
/// 2: `[writable]` PendingCommitment PDA
#[inline(never)]
pub fn process_approve_commitment(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        verifier, // force multi-line
        verifier_bond,
        pending_commitment,
    ] = require_n_accounts!(accounts, 3);

    require_eq!(data.len(), 0, ProgramError::InvalidInstructionData);
    require_signer!(verifier);

    require_initialized_pda(
        verifier_bond,
        &[VERIFIER_BOND_SEED, verifier.address().as_ref()],
        &crate::fast::ID,
        false,
        "verifier bond",
    )?;
    require_owned_pda(
        pending_commitment,
        &crate::fast::ID,
        "pending commitment",
    )?;
    if !pending_commitment.is_writable() {
        return Err(ProgramError::Immutable);
    }

    let verifier_bond_data = verifier_bond.try_borrow()?;
    let verifier_bond_state =
        VerifierBond::decode(verifier_bond_data.as_ref())?;
    validate_verifier_bond(&verifier_bond_state, verifier)?;
    drop(verifier_bond_data);

    let pending_data = pending_commitment.try_borrow()?;
    let pending_state = PendingCommitment::decode(pending_data.as_ref())?;
    validate_pending_commitment(&pending_state, pending_commitment, verifier)?;

    let selected_verifier = pending_state
        .selected_verifiers()
        .get(0)
        .ok_or(ProgramError::InvalidAccountData)?;
    if selected_verifier.approved() {
        require_eq!(
            pending_state.approval_count(),
            1,
            ProgramError::InvalidAccountData
        );
        return Ok(());
    }

    require_eq!(
        pending_state.approval_count(),
        0,
        ProgramError::InvalidAccountData
    );

    let updated_pending = PendingCommitment {
        discriminator: PendingCommitment::DISCRIMINATOR,
        status: pending_state.status(),
        operator_identity: *pending_state.operator_identity(),
        operator_bond: *pending_state.operator_bond(),
        account_pubkey: *pending_state.account_pubkey(),
        commit_id: pending_state.commit_id(),
        delegation_record: *pending_state.delegation_record(),
        da_pointer_hash: *pending_state.da_pointer_hash(),
        account_state_hash: *pending_state.account_state_hash(),
        data_hash: *pending_state.data_hash(),
        lamports: pending_state.lamports(),
        owner: *pending_state.owner(),
        state_commitment_hash: *pending_state.state_commitment_hash(),
        verifier_registry: *pending_state.verifier_registry(),
        verifier_registry_revision: pending_state.verifier_registry_revision(),
        challenge_window_id: pending_state.challenge_window_id(),
        posted_slot: pending_state.posted_slot(),
        activation_slot: pending_state.activation_slot(),
        challenge_window_end_slot: pending_state.challenge_window_end_slot(),
        approval_count: 1,
        approval_threshold: pending_state.approval_threshold(),
        active_challenge: pending_state.active_challenge().cloned(),
        resolved_state_source: pending_state.resolved_state_source(),
        er_slot: pending_state.er_slot(),
        _pad_before_selected_verifiers: [0; 7],
        selected_verifiers: vec![SelectedVerifier {
            verifier_identity: *selected_verifier.verifier_identity(),
            approved: true,
            _pad_after_approved: [0; 7],
        }],
    };
    drop(pending_data);

    updated_pending.encode_to(pending_commitment.try_borrow_mut()?.as_mut())?;

    Ok(())
}

fn validate_verifier_bond(
    verifier_bond: &dlp_api::v2::VerifierBondView<'_>,
    verifier: &AccountView,
) -> ProgramResult {
    if verifier_bond.discriminator() != VerifierBond::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        &Address::from(verifier_bond.verifier_identity().to_bytes()),
        verifier.address(),
        DlpError::InvalidAuthority
    );
    require_eq!(
        verifier_bond.status(),
        VERIFIER_STATUS_ACTIVE,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        verifier_bond.withdraw_requested_slot().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_pending_commitment(
    pending_commitment: &dlp_api::v2::PendingCommitmentView<'_>,
    pending_commitment_account: &AccountView,
    verifier: &AccountView,
) -> ProgramResult {
    if pending_commitment.discriminator() != PendingCommitment::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    let commit_id_bytes = pending_commitment.commit_id().to_le_bytes();
    require_initialized_pda(
        pending_commitment_account,
        &[
            PENDING_COMMITMENT_SEED,
            pending_commitment.account_pubkey().as_ref(),
            &commit_id_bytes,
        ],
        &crate::fast::ID,
        true,
        "pending commitment",
    )?;

    require_eq!(
        pending_commitment.status(),
        PENDING_COMMITMENT_STATUS_ACTIVE,
        ProgramError::InvalidInstructionData
    );
    require_le!(
        Clock::get()?.slot,
        pending_commitment.challenge_window_end_slot(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending_commitment.selected_verifiers().len(),
        1,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        pending_commitment.approval_threshold(),
        1,
        ProgramError::InvalidAccountData
    );

    let selected_verifier = pending_commitment
        .selected_verifiers()
        .get(0)
        .ok_or(ProgramError::InvalidAccountData)?;
    require_eq_keys!(
        &Address::from(selected_verifier.verifier_identity().to_bytes()),
        verifier.address(),
        DlpError::InvalidAuthority
    );

    Ok(())
}
