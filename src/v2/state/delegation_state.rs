use bytemuck::{Pod, Zeroable};
use pinocchio::{error::ProgramError, AccountView, Address};

use crate::{
    args::Boolean,
    pod_view::PodView,
    v2::{HeaderWithBuffer, HeaderWithBufferMut},
    v2_require_eq, v2_require_ge, v2_require_owned_by,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DelegationStateHeader {
    /// discriminator that identifies this account type
    pub discriminator: [u8; 8],

    /// validated, immutable account bindings for this delegation.
    pub bindings: ValidatedDelegationBindings,

    /// the lamports at the time of delegation or from the last state finalization
    /// stored as lamports can be received even if the account is delegated
    pub original_lamports: u64,

    /// the last commit-id account had during delegation update
    /// Deprecated: The last slot at which the delegation was updated
    pub last_commit_id: u64,

    /// Whether the account can be undelegated or not
    pub is_undelegatable: Boolean,

    pub reserved_padding0: [u8; 7],
    // The seeds of the account, used to reopen it on undelegation
    /// the original owner of the account
    pub original_owner: Address,

    /// the slot at which the delegation was created
    pub delegation_slot: u64,

    /// the state update frequency in milliseconds
    pub commit_frequency_ms: u64,

    /// The account that paid the rent for the delegation PDAs
    pub rent_payer: Address,
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
    pub delegated_account: Address,
    pub validator_as_authority: Address,
    pub validator_fees_vault: Address,
}

pub type DelegationState<'a> = HeaderWithBuffer<'a, DelegationStateHeader>;
pub type DelegationStateMut<'a> =
    HeaderWithBufferMut<'a, DelegationStateHeader>;

impl DelegationStateHeader {
    pub const SEED: &'static [u8] = b"delegation_state";

    ///
    /// Account discriminator and state kind identifier.
    ///
    /// The suffix `00` in `state.00` identifies a specific delegation state kind.
    /// Different values (e.g, `state.01`, `state.02`, …) may coexist and represent
    /// distinct delegation models with different layouts and semantics, optimized
    /// for different performance or validation tradeoffs.
    ///
    /// Up to 256 state kinds are supported in the range `[0x00, 0xff]`.
    ///
    pub const DISCRIMINATOR: [u8; 8] = *b"state.00";

    /// Highest supported delegation state kind.
    pub const MAX_STATE_KIND: u8 = 0x00;

    pub const DISCRIMINATOR_FAST: u64 = u64::from_le_bytes(Self::DISCRIMINATOR);

    #[inline(always)]
    pub fn validate(
        delegation_state: &AccountView,
    ) -> Result<(), ProgramError> {
        v2_require_owned_by!(delegation_state, &crate::fast::ID);

        #[cfg(feature = "unsafe")]
        let data = unsafe { delegation_state.borrow_unchecked() };

        #[cfg(not(feature = "unsafe"))]
        let data = delegation_state.try_borrow()?;

        v2_require_ge!(
            data.len(),
            DelegationStateHeader::SPACE,
            ProgramError::InvalidAccountData
        );

        v2_require_eq!(
            unsafe { (data.as_ptr() as *const u64).read() },
            Self::DISCRIMINATOR_FAST,
            ProgramError::InvalidAccountData
        );

        Ok(())
    }
}
