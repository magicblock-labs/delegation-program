use crate::compat::Pubkey;

pub const DELEGATION_CLUSTER_SEED: &[u8] = b"delegation-cluster";
pub const OPERATOR_BOND_SEED: &[u8] = b"operator-bond";
pub const VERIFIER_REGISTRY_SEED: &[u8] = b"verifier-registry";

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

pub fn operator_bond_pda(operator: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[OPERATOR_BOND_SEED, operator.as_ref()],
        &crate::id(),
    )
    .0
}
