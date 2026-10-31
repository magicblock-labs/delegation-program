use crate::compat::Pubkey;

pub const DELEGATION_CLUSTER_SEED: &[u8] = b"delegation-cluster";
pub const DELEGATION_STATE_SEED: &[u8] = b"delegation-state";
pub const OPERATOR_BOND_SEED: &[u8] = b"operator-bond";
pub const VERIFIER_BOND_SEED: &[u8] = b"verifier-bond";
pub const VERIFIER_REGISTRY_SEED: &[u8] = b"verifier-registry";
pub const STATE_BUFFER_SEED: &[u8] = b"state-buffer";
pub const PENDING_COMMITMENT_SEED: &[u8] = b"pending-commitment";
pub const CHALLENGE_SEED: &[u8] = b"challenge";
pub const UNDELEGATE_BUFFER_SEED: &[u8] = b"undelegate-buffer";

// TODO (snawaz): Precompute these addresses if PDA derivation becomes const-safe.

pub fn delegation_cluster_pda(cluster_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[DELEGATION_CLUSTER_SEED, cluster_id.as_ref()],
        &crate::id(),
    )
    .0
}

pub fn delegation_state_pda(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[DELEGATION_STATE_SEED, delegated_account.as_ref()],
        &crate::id(),
    )
    .0
}

pub fn verifier_registry_pda(delegation_cluster: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[VERIFIER_REGISTRY_SEED, delegation_cluster.as_ref()],
        &crate::id(),
    )
    .0
}

pub fn operator_bond_pda(delegation_cluster: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[OPERATOR_BOND_SEED, delegation_cluster.as_ref()],
        &crate::id(),
    )
    .0
}

pub fn verifier_bond_pda(
    delegation_cluster: &Pubkey,
    verifier: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            VERIFIER_BOND_SEED,
            delegation_cluster.as_ref(),
            verifier.as_ref(),
        ],
        &crate::id(),
    )
    .0
}

pub fn state_buffer_pda(
    account: &Pubkey,
    commit_seq: u32,
    authority: &Pubkey,
) -> Pubkey {
    state_buffer_pda_with_bump(account, commit_seq, authority).0
}

pub fn state_buffer_pda_with_bump(
    account: &Pubkey,
    commit_seq: u32,
    authority: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            STATE_BUFFER_SEED,
            account.as_ref(),
            &commit_seq.to_le_bytes(),
            authority.as_ref(),
        ],
        &crate::id(),
    )
}

pub fn pending_commitment_pda(account: &Pubkey, commit_seq: u32) -> Pubkey {
    pending_commitment_pda_with_bump(account, commit_seq).0
}

pub fn pending_commitment_pda_with_bump(
    account: &Pubkey,
    commit_seq: u32,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            PENDING_COMMITMENT_SEED,
            account.as_ref(),
            &commit_seq.to_le_bytes(),
        ],
        &crate::id(),
    )
}

pub fn challenge_pda(
    account: &Pubkey,
    commit_seq: u32,
    challenger: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            CHALLENGE_SEED,
            account.as_ref(),
            &commit_seq.to_le_bytes(),
            challenger.as_ref(),
        ],
        &crate::id(),
    )
    .0
}

pub fn undelegate_buffer_pda(delegated_account: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[UNDELEGATE_BUFFER_SEED, delegated_account.as_ref()],
        &crate::id(),
    )
}
