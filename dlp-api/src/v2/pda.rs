use crate::compat::Pubkey;

pub const DELEGATION_CLUSTER_SEED: &[u8] = b"delegation-cluster";
pub const OPERATOR_BOND_SEED: &[u8] = b"operator-bond";
pub const VERIFIER_BOND_SEED: &[u8] = b"verifier-bond";
pub const VERIFIER_REGISTRY_SEED: &[u8] = b"verifier-registry";
pub const STATE_BUFFER_SEED: &[u8] = b"state-buffer";
pub const PENDING_COMMITMENT_SEED: &[u8] = b"pending-commitment";

// TODO (snawaz): Precompute these addresses if PDA derivation becomes const-safe.

pub fn delegation_cluster_pda(cluster_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[DELEGATION_CLUSTER_SEED, cluster_id.as_ref()],
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
    commit_id: u64,
    authority: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            STATE_BUFFER_SEED,
            account.as_ref(),
            &commit_id.to_le_bytes(),
            authority.as_ref(),
        ],
        &crate::id(),
    )
    .0
}

pub fn pending_commitment_pda(account: &Pubkey, commit_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            PENDING_COMMITMENT_SEED,
            account.as_ref(),
            &commit_id.to_le_bytes(),
        ],
        &crate::id(),
    )
    .0
}
