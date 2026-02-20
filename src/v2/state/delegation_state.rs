use bytemuck::{Pod, Zeroable};
use pinocchio::Address;

use crate::{args::Boolean, v2::HeaderWithBuffer};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DelegationStateHeader {
    /// discriminator that identifies this account type
    pub discriminator: [u8; 8],

    /// the original owner of the account
    pub original_owner: Address,

    /// the slot at which the delegation was created
    pub delegation_slot: u64,

    /// the lamports at the time of delegation or from the last state finalization
    /// stored as lamports can be received even if the account is delegated
    pub lamports: u64,

    /// the state update frequency in milliseconds
    pub commit_frequency_ms: u64,

    /// validated, immutable account bindings for this delegation.
    pub bindings: ValidatedDelegationBindings,

    /// the last commit-id account had during delegation update
    /// Deprecated: The last slot at which the delegation was updated
    pub last_commit_id: u64,

    /// The account that paid the rent for the delegation PDAs
    pub rent_payer: Address,

    /// Whether the account can be undelegated or not
    pub is_undelegatable: Boolean,

    pub reserved_padding0: [u8; 7],
    // The seeds of the account, used to reopen it on undelegation
    //pub seeds: Vec<Vec<u8>>,
}

///
/// A fixed, authoritative set of accounts bound to a delegation.
///
/// These bindings are fully validated at delegation time and are immutable
/// for the lifetime of the delegation. After creation, commit and finalize
/// instructions rely on simple key equality checks against these bindings,
/// avoiding repeated expensive PDA derivation and seed/bump validation.
///
/// Security Model
/// ==============
///
/// This design relies on the following Solana runtime guarantees:
///
///   - Account data owned by a program can only be created or modified by that program.
///   - Program-owned account data cannot be forged or mutated by users or other programs.
///   - A `DelegationState` account is considered valid if and only if it is owned by
///     this program and its discriminator matches the expected value.
///
/// Under these assumptions, the bindings stored here are authoritative and can be
/// safely used for fast-path account validation.
///
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ValidatedDelegationBindings {
    pub delegation_account: Address,
    //pub validator_fees_vault: Address,
    pub validator_as_authority: Address,
}

pub type DelegationState<'a> = HeaderWithBuffer<'a, DelegationStateHeader>;

impl DelegationStateHeader {
    pub const SEED: &'static [u8] = b"delegation_state";

    ///
    /// v1 means version 1 of this account type. this allows us to evolve without
    /// account migration.
    ///
    pub const DISCRIMINATOR: [u8; 8] = *b"v1.state";

    pub const DISCRIMINATOR_FAST: u64 = u64::from_le_bytes(Self::DISCRIMINATOR);
}
