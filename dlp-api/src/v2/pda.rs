use crate::compat::Pubkey;

pub const PROTOCOL_CONFIG_SEED: &[u8] = b"protocol-config";
pub const OPERATOR_BOND_SEED: &[u8] = b"operator-bond";
pub const VERIFIER_BOND_SEED: &[u8] = b"verifier-bond";
pub const VERIFIER_REGISTRY_SEED: &[u8] = b"verifier-registry";
pub const STATE_BUFFER_SEED: &[u8] = b"state-buffer";
pub const PENDING_COMMITMENT_SEED: &[u8] = b"pending-commitment";
pub const CHALLENGE_SEED: &[u8] = b"challenge";

// TODO (snawaz): Precompute these addresses if PDA derivation becomes const-safe.

pub fn protocol_config_pda() -> Pubkey {
    Pubkey::find_program_address(&[PROTOCOL_CONFIG_SEED], &crate::id()).0
}

pub fn verifier_registry_pda() -> Pubkey {
    Pubkey::find_program_address(&[VERIFIER_REGISTRY_SEED], &crate::id()).0
}

pub fn operator_bond_pda(operator: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[OPERATOR_BOND_SEED, operator.as_ref()],
        &crate::id(),
    )
    .0
}

pub fn verifier_bond_pda(verifier: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[VERIFIER_BOND_SEED, verifier.as_ref()],
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

pub fn challenge_pda(
    account: &Pubkey,
    commit_id: u64,
    challenger: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            CHALLENGE_SEED,
            account.as_ref(),
            &commit_id.to_le_bytes(),
            challenger.as_ref(),
        ],
        &crate::id(),
    )
    .0
}
