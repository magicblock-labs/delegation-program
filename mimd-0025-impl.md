# MIMD-0025 Implementation Notes

Companion to `mimd-0025.md`. This file only captures low-level implementation
choices and message shapes. Protocol rationale stays in the MIMD.

## Contents

- [Decisions To Review](#decisions-to-review)
- [Permissioned vs Permissionless](#permissioned-vs-permissionless)
- [Hashes](#hashes)
- [Accounts](#accounts)
- [Instructions](#instructions)
  - [Bootstrap Instructions](#bootstrap-instructions)
  - [Runtime Instructions](#runtime-instructions)
  - [Key Instruction Data](#key-instruction-data)
  - [Important Instruction Rules](#important-instruction-rules)
  - [Failure Scenarios](#failure-scenarios)
- [Flows](#flows)
  - [Dispute Resolution](#dispute-resolution)
- [Validator Repo Responsibilities](#validator-repo-responsibilities)
- [Design FAQ](#design-faq)
- [Open Design Points](#open-design-points)

## Decisions To Review

- Use `ephemeral-vrf`; DLP requests randomness and receives a callback.
- Start the challenge window after VRF activation, not at initial post.
- DLP v2 verifier approvals are normal Solana signer transactions.
- `PendingCommitment` stores hashes/metadata; full data is opened via
  `StateBuffer` for finalize or dispute resolution.
- A multisig resolves disputes. DLP only checks that the configured resolver
  signed `ResolveDispute`, then applies the chosen outcome.
- DLP v2 supports one active challenge per pending commitment.
- Operator timeout is evidence only. Challenger must still reveal state+salt.

## Permissioned vs Permissionless

Our implementation target is **DLP v2**. DLP v1 is the current program. DLP v2
should start permissioned, but with the permissionless account shape:

- `Permissionless` means any participant can become an operator, verifier, or
  challenger by satisfying on-chain bond/stake rules. Registration, withdrawals,
  slashing, verifier registry updates, and payouts are enforced without admin
  curation.
- `Permissioned` means the protocol uses the same accounts and state machine,
  but bootstrap actions such as operator admission, verifier admission,
  verifier registry changes, and dispute resolution are controlled by configured
  signers.

So our DLP v2 "permissioned" means: 

- use `OperatorBond`, `VerifierBond`, `VerifierRegistry`,
  `PendingCommitment`, `Challenge`, and payout accounts from the start;
- do not treat legacy fee vaults or whitelists as slashable protocol stake;
- allow controlled authorities to admit operators/verifiers and update the
  verifier registry during bootstrap;
- keep commitment, approval, challenge, reveal, resolution, and finalization logic
  independent of whether actor admission is permissioned or permissionless.

The later permissionless version should mainly replace admission/registry policy,
not redesign commitment or challenge accounts.

## Hashes

```text
data_hash = H("magicblock.account_data.v1", account_data)

account_state_hash = H(
  "magicblock.account_state.v1",
  lamports,
  owner,
  data_hash
)

da_pointer_hash = H("magicblock.da_pointer.v1", da_pointer_bytes)

state_commitment_hash = H(
  "magicblock.state_commitment.v1",
  operator_identity,
  account_pubkey,
  commit_id,
  delegation_record,
  da_pointer_hash,
  account_state_hash,
  verifier_registry,
  challenge_window_id
)

challenge_hash = H(
  "magicblock.challenge.v1",
  state_commitment_hash,
  operator_identity,
  challenger_identity,
  account_pubkey,
  commit_id,
  challenger_account_state_hash,
  salt
)
```

Open parameters: hash function, serialization, DA pointer format,
missing-account representation, economics, timeouts, and thresholds.

## Accounts

Seed strings are placeholders until frozen.

| Account | PDA seeds | Purpose |
| --- | --- | --- |
| `ProtocolConfig` | `["protocol-config"]` | Global params, VRF config, resolver signer, protocol fee vault. |
| `OperatorBond` | `["operator-bond", operator]` | Slashable operator stake and lifecycle. |
| `VerifierBond` | `["verifier-bond", verifier]` | Slashable verifier stake. |
| `VerifierRegistry` | `["verifier-registry"]` | All registered verifiers. |
| `PendingCommitment` | `["pending-commitment", account, commit_id]` | Main commitment state machine. |
| `StateBuffer` | `["state-buffer", account, commit_id, role, authority]` | Chunked full account data opened for finalize/reveal. |
| `Challenge` | `["challenge", account, commit_id, challenger]` | One challenge against one pending commitment. |
| `PayoutTimelock` | `["payout-timelock", challenge]` | Delayed payout for correct challenger. |

### Essential Fields

```rust
pub enum ActorStatus {
    /// Actor can participate normally.
    Active,
    /// Actor has requested withdrawal and cannot take on new work.
    Exiting,
    /// Actor lost stake for protocol fault.
    Slashed,
    /// Actor is temporarily blocked without necessarily losing all stake.
    Jailed,
}

/// PDA: `["protocol-config"]`
/// Created by: `InitProtocolConfig`.
/// Closed by: not normally closed.
pub struct ProtocolConfig {
    /// Signer allowed to update config and permissioned-v2 bootstrap state.
    pub authority: Pubkey,
    /// Emergency stop for new commitments and other non-exit activity.
    pub paused: bool,
    /// Oracle queue DLP uses when requesting randomness.
    pub vrf_oracle_queue: Pubkey,
    /// Multisig-controlled signer allowed to call ResolveDispute.
    pub resolver: Pubkey,
    /// Vault receiving protocol fees, penalties, or slashed funds.
    pub protocol_fee_vault: Pubkey,
    /// Minimum stake required for an operator to register and stay active.
    pub min_operator_bond: u64,
    /// Minimum stake required for a verifier to register and stay active.
    pub min_verifier_bond: u64,
    /// Minimum stake locked by RaiseChallenge to prevent cheap spam.
    pub min_challenger_stake: u64,
    /// Slots available for approval/challenge after VRF activation.
    pub challenge_window_slots: u64,
    /// Slots the operator gets to open state after a challenge.
    pub operator_response_timeout_slots: u64,
    /// Slots the challenger gets to reveal after operator response or timeout.
    pub challenger_reveal_timeout_slots: u64,
    /// Delay before a winning challenger can claim payout.
    pub payout_timelock_slots: u64,
    /// Number of verifiers randomly picked from VerifierRegistry.
    pub selected_verifier_count: u16,
    /// Approvals required for happy-path finalization.
    pub approval_threshold: u16,
    /// Maximum under-approval extensions before the commitment expires.
    pub max_window_extensions: u16,
    /// Penalty for a valid reveal that matches the operator state.
    pub match_penalty_bps: u16,
}
// Review: `protocol_fee_vault` may be too vague if it also holds slashed funds
// and penalties.
// Review: fields copied into PendingCommitment or Challenge must not be read
// from ProtocolConfig later for already-open records.

/// PDA: `["operator-bond", operator]`
/// Created by: `RegisterOperator`.
/// Closed by: `WithdrawStake` after exit, zero stake, and no locks.
///
/// One account per operator. `PostCommitment` checks this account.
pub struct OperatorBond {
    /// Operator identity this bond belongs to.
    pub operator_identity: Pubkey,
    /// Slashable stake currently credited to this operator.
    pub stake_lamports: u64,
    /// Stake temporarily unavailable for withdrawal.
    pub locked_lamports: u64,
    /// Whether this operator can post new commitments.
    pub status: ActorStatus,
    /// Slot when exit was requested; None means no pending withdrawal.
    pub withdraw_requested_slot: Option<u64>,
}
// Review: `locked_lamports` needs exact lock/unlock rules when one operator has
// multiple pending commitments or disputes.

/// PDA: `["verifier-bond", verifier]`
/// Created by: `RegisterVerifier`.
/// Closed by: `WithdrawStake` after exit, zero stake, no locks, and registry removal.
///
/// One account per verifier. Selection also requires presence in VerifierRegistry.
pub struct VerifierBond {
    /// Verifier identity this bond belongs to.
    pub verifier_identity: Pubkey,
    /// Slashable stake currently credited to this verifier.
    pub stake_lamports: u64,
    /// Whether this verifier can be selected and approve commitments.
    pub status: ActorStatus,
    /// Slot when the verifier bond was created.
    pub registered_slot: u64,
    /// Slot when exit was requested; None means no pending withdrawal.
    pub withdraw_requested_slot: Option<u64>,
}
// Review: `registered_slot` is only useful if verifier warm-up/cooldown rules
// exist. Remove it if no such rule is planned.

/// PDA: `["verifier-registry"]`
/// Created by: `InitProtocolConfig` as an empty registry.
/// Updated by: `UpdateVerifierRegistry`.
/// Closed by: not normally closed.
///
/// Single registry containing all verifiers that can be selected.
pub struct VerifierRegistry {
    /// Increments every time `entries` changes.
    /// A pending commitment stores this value before requesting VRF. The VRF
    /// callback must see the same value before it can select verifiers.
    pub registry_revision: u64,
    /// All registered verifiers DLP can select from.
    pub entries: Vec<VerifierRegistryEntry>,
}
// Review: account size must be bounded before implementation. If the verifier
// set can grow large, replace this Vec with a Merkleized or paged registry.

pub struct VerifierRegistryEntry {
    /// Verifier identity selectable by VRF.
    pub verifier_identity: Pubkey,
    /// Bond account proving this verifier has active stake.
    pub verifier_bond: Pubkey,
    /// Keep as 1 for equal-weight selection.
    pub weight: u64,
}
// Review: keep `weight` only if weighted selection is in scope. For equal
// selection, removing it makes selection easier to review.

pub enum PendingCommitmentStatus {
    /// Posted and waiting for VRF callback.
    AwaitingRandomness,
    /// VRF selected verifiers and the challenge window is open.
    Active,
    /// Challenge raised and waiting for operator state.
    AwaitingOperatorResponse,
    /// Operator responded and challenger must reveal.
    AwaitingChallengerReveal,
    /// Operator timed out and challenger must still reveal.
    AwaitingChallengerRevealAfterOperatorTimeout,
    /// Operator/challenger states differ and resolver must decide.
    AwaitingDisputeResolution,
    /// Resolver chose operator-opened state.
    ResolvedOperator,
    /// Resolver chose challenger-opened state.
    ResolvedChallenger,
    /// Final state was applied to the base layer.
    Finalized,
    /// Commitment can no longer finalize.
    Expired,
    /// Commitment was cancelled before activation.
    Cancelled,
}
// Review: `ResolvedOperator` and `ResolvedChallenger` overlap with
// ResolvedStateSource. Keep both only if status and finalization source need to
// evolve independently.

pub enum ResolvedStateSource {
    /// Finalize using the operator commitment/happy-path state.
    OperatorCommitment,
    /// Finalize using challenger-opened state after resolution.
    ChallengerReveal,
}

/// PDA: `["pending-commitment", account, commit_id]`
/// Created by: `PostCommitment`.
/// Closed by: `CloseTerminalAccounts` after finalize, cancel, or expiry.
///
/// One account per delegated account and commit id.
pub struct PendingCommitment {
    /// Current state-machine state for this commitment.
    pub status: PendingCommitmentStatus,
    /// Operator identity that posted the commitment.
    pub operator_identity: Pubkey,
    /// OperatorBond checked when the commitment was posted.
    pub operator_bond: Pubkey,
    /// Delegated account whose base-layer state will be finalized.
    pub account_pubkey: Pubkey,
    /// Operator-chosen nonce for this account commitment.
    pub commit_id: u64,
    /// Delegation metadata tying this account to the ER context.
    pub delegation_record: Pubkey,
    /// Hash of replay/data-availability pointer bytes.
    pub da_pointer_hash: [u8; 32],
    /// Hash of lamports, owner, and data_hash.
    pub account_state_hash: [u8; 32],
    /// Hash of full account data.
    pub data_hash: [u8; 32],
    /// Lamports committed by the operator.
    pub lamports: u64,
    /// Owner committed by the operator.
    pub owner: Pubkey,
    /// Hash binding operator, account, commit id, delegation, DA, and state.
    pub state_commitment_hash: [u8; 32],
    /// Registry account used when this commitment was posted.
    pub verifier_registry: Pubkey,
    /// Copy of `VerifierRegistry.registry_revision` at post time.
    /// If the registry changes before VRF activation, this commitment is
    /// cancelled and the operator must repost against the latest registry.
    pub verifier_registry_revision: u64,
    /// Monotonic id for this approval/challenge window.
    pub challenge_window_id: u64,
    /// Slot when the commitment was posted.
    pub posted_slot: u64,
    /// Set by the VRF callback.
    pub activation_slot: Option<u64>,
    /// Slot when approval/challenge window closes after VRF activation.
    pub challenge_window_end_slot: Option<u64>,
    /// Verifiers selected by VRF for this commitment.
    /// Later registry changes do not rewrite this list.
    pub selected_verifiers: Vec<Pubkey>,
    /// One bit per selected verifier.
    pub approval_bitmap: Vec<u8>,
    /// Number of unique selected verifiers that approved.
    pub approval_count: u16,
    /// Threshold copied from ProtocolConfig at activation time.
    pub approval_threshold: u16,
    /// Active Challenge account, if any.
    pub active_challenge: Option<Pubkey>,
    /// VRF request id returned when DLP asks for randomness.
    pub vrf_request_id: Option<[u8; 32]>,
    /// VRF output bytes used to select verifiers.
    pub vrf_randomness: Option<[u8; 32]>,
    /// Which opened state finalization must use after dispute resolution.
    pub resolved_state_source: Option<ResolvedStateSource>,
}
// Review: `da_pointer_hash` is not enough by itself. Verifiers/resolver need an
// independent way to fetch replay inputs, not only operator-provided data.
// Review: `challenge_window_id` should be kept only if window extensions or
// retries need an explicit round id.

pub enum StateBufferRole {
    /// Full data opened for normal finalization.
    OperatorFinalize,
    /// Full data opened by operator during challenge response.
    OperatorChallengeResponse,
    /// Full data opened by challenger during reveal.
    ChallengerReveal,
}

/// PDA: `["state-buffer", account, commit_id, role, authority]`
/// Created by: first `WriteStateBuffer`.
/// Frozen by: `FinalizeStateBuffer`.
/// Closed by: `CloseTerminalAccounts`.
///
/// Stores opened full account data when it does not fit in one instruction.
pub struct StateBuffer {
    /// Why this full account data is being opened.
    pub role: StateBufferRole,
    /// Signer allowed to write/finalize this buffer.
    pub authority: Pubkey,
    /// Account whose data is being opened.
    pub account_pubkey: Pubkey,
    /// Commitment this buffer belongs to.
    pub commit_id: u64,
    /// Hash the finished buffer must match.
    pub expected_data_hash: [u8; 32],
    /// Expected total byte length.
    pub total_len: u32,
    /// Bytes written so far.
    pub written_len: u32,
    /// Once true, buffer content cannot change.
    pub finalized: bool,
    /// Chunked full account data.
    pub data: Vec<u8>,
}
// Review: `total_len` and account size need hard caps. An unbounded Vec is not
// implementable safely as a Solana account.

pub enum ChallengeStatus {
    /// Challenge raised and waiting for operator state.
    AwaitingOperatorResponse,
    /// Operator responded and challenger must reveal.
    AwaitingChallengerReveal,
    /// Operator timed out and challenger must still reveal.
    AwaitingChallengerRevealAfterOperatorTimeout,
    /// Opened states differ and resolver must decide.
    AwaitingDisputeResolution,
    /// Challenge has an outcome and can be closed when allowed.
    Terminal,
}

pub struct OpenedState {
    /// Opened lamports value.
    pub lamports: u64,
    /// Opened owner value.
    pub owner: Pubkey,
    /// Hash of opened account data.
    pub data_hash: [u8; 32],
    /// Hash of opened lamports, owner, and data_hash.
    pub account_state_hash: [u8; 32],
    /// Finalized buffer containing the full account data, when needed.
    pub state_buffer: Option<Pubkey>,
}

pub enum ChallengeOutcome {
    /// Reveal did not match challenge_hash; challenger loses stake.
    InvalidRevealChallengerSlashed,
    /// Reveal matched operator state; challenger pays match penalty.
    MatchingStateChallengerPenalized,
    /// Resolver or timeout path chose operator; challenger loses stake.
    OperatorCorrectChallengerSlashed,
    /// Resolver chose challenger; operator is slashed.
    ChallengerCorrectOperatorSlashed,
    /// Challenger failed to reveal after operator response/timeout.
    ChallengerRevealTimeout,
}

/// PDA: `["challenge", account, commit_id, challenger]`
/// Created by: `RaiseChallenge`.
/// Closed by: `CloseTerminalAccounts` after a terminal challenge outcome.
///
/// One active challenge is allowed per pending commitment in DLP v2.
pub struct Challenge {
    /// Current state-machine state for this challenge.
    pub status: ChallengeStatus,
    /// PendingCommitment being challenged.
    pub pending_commitment: Pubkey,
    /// Challenger that posted the hidden challenge hash.
    pub challenger_identity: Pubkey,
    /// Stake locked by this challenge.
    pub challenger_stake_lamports: u64,
    /// Salted hash binding the challenger reveal to this commitment.
    pub challenge_hash: [u8; 32],
    /// Slot when challenge was raised.
    pub raised_slot: u64,
    /// Last slot for operator to open state.
    pub operator_response_deadline_slot: u64,
    /// Last slot for challenger reveal; set after operator response/timeout.
    pub challenger_reveal_deadline_slot: Option<u64>,
    /// Operator-opened state after response, if any.
    pub operator_state: Option<OpenedState>,
    /// Challenger-opened state after reveal, if any.
    pub challenger_state: Option<OpenedState>,
    /// Terminal outcome once challenge can no longer change.
    pub outcome: Option<ChallengeOutcome>,
}
// Review: PDA includes `challenger`, but DLP v2 allows only one active
// challenge per PendingCommitment. Either enforce `active_challenge` strictly or
// remove `challenger` from the PDA seeds.

/// PDA: `["payout-timelock", challenge]`
/// Created by: `ResolveDispute` when challenger payout is delayed.
/// Closed by: `ClaimPayout` after payout.
pub struct PayoutTimelock {
    /// Challenge that created this payout.
    pub challenge: Pubkey,
    /// Account allowed to claim payout.
    pub beneficiary: Pubkey,
    /// Payout amount.
    pub amount_lamports: u64,
    /// First slot where ClaimPayout can succeed.
    pub unlock_slot: u64,
    /// Prevents double payout.
    pub claimed: bool,
}
// Review: keep this account only if payout delay is required. Otherwise
// ResolveDispute can pay immediately and avoid another account.
```

## Instructions

Account lists include only protocol-relevant accounts. Add payer/system/rent
accounts where account creation or lamport movement requires them.

### Bootstrap Instructions

Authority-gated setup and admission instructions for permissioned v2.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `InitProtocolConfig`<ul><li>ix-data: <code>params</code></li><li>accounts: <strong>authority signer, ProtocolConfig, VerifierRegistry</strong></li></ul> | Protocol authority | Creates the global config account and empty verifier registry. Stores bootstrap params such as VRF oracle queue, resolver, fees, thresholds, and timeouts. |
| `UpdateProtocolConfig`<ul><li>ix-data: <code>params</code></li><li>accounts: <strong>authority signer, ProtocolConfig</strong></li></ul> | Protocol authority | Updates params used by future commitments. Existing pending commitments keep the values copied into their accounts. |
| `RegisterOperator`<ul><li>ix-data: <code>amount_lamports</code></li><li>accounts: <strong>operator signer, protocol authority signer, OperatorBond, ProtocolConfig</strong></li></ul> | Operator, protocol authority | Creates the per-operator `OperatorBond` PDA at `["operator-bond", operator]` and deposits slashable stake. Permissioned v2 requires configured approval before the operator can post commitments. |
| `RegisterVerifier`<ul><li>ix-data: <code>amount_lamports</code></li><li>accounts: <strong>verifier signer, protocol authority signer, VerifierBond, ProtocolConfig</strong></li></ul> | Verifier, protocol authority | Creates the per-verifier `VerifierBond` PDA at `["verifier-bond", verifier]` and deposits slashable stake. Permissioned v2 requires configured approval before the verifier can enter the registry. |
| `UpdateVerifierRegistry`<ul><li>ix-data: <code>update</code></li><li>accounts: <strong>authority signer, VerifierRegistry, VerifierBond accounts</strong></li></ul> | Protocol authority | Adds or removes verifier pubkeys in the single `VerifierRegistry` account and increments `registry_revision`. Invalid, duplicate, unbonded, or inactive verifiers are rejected. |

### Runtime Instructions

Normal actor lifecycle, commitment, challenge, resolution, and cleanup
instructions.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `RequestStakeWithdrawal`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, OperatorBond or VerifierBond, ProtocolConfig</strong></li></ul> | Operator or verifier | Marks bonded stake as exiting. The stake cannot be withdrawn until the configured delay passes and no locks remain. |
| `WithdrawStake`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, OperatorBond or VerifierBond, ProtocolConfig</strong></li></ul> | Operator or verifier | Withdraws unlocked stake after the exit delay. Slashed or locked stake stays in the protocol. |
| `PostCommitment`<ul><li>ix-data: <code>commitment</code></li><li>accounts: <strong>operator signer, OperatorBond, PendingCommitment, delegated account, DelegationRecord, ProtocolConfig, VerifierRegistry, DLP identity PDA, VRF oracle queue, VRF program</strong></li></ul> | Operator | Creates an `AwaitingRandomness` commitment, stores the current `registry_revision`, locks any commitment-local stake if needed, and requests VRF. |
| `ConsumeCommitmentRandomness`<ul><li>ix-data: <code>randomness</code></li><li>accounts: <strong>VRF program identity signer, PendingCommitment, ProtocolConfig, VerifierRegistry</strong></li></ul> | VRF callback | Verifies the VRF callback signer and registry revision, selects verifiers from the registry excluding the commitment operator, and starts the challenge window. |
| `CancelUnactivatedCommitment`<ul><li>ix-data: <code>reason</code></li><li>accounts: <strong>cranker/operator signer, PendingCommitment, ProtocolConfig, VerifierRegistry</strong></li></ul> | Operator or cranker | Cancels a commitment that is still waiting for randomness but can no longer activate, such as after registry change or VRF timeout. |
| `ApproveCommitment`<ul><li>ix-data: <code>selected_verifier_index</code></li><li>accounts: <strong>verifier signer, VerifierBond, PendingCommitment</strong></li></ul> | Selected verifier | Records approval from one selected verifier. Duplicate approvals do not increase the count. |
| `WriteStateBuffer`<ul><li>ix-data: <code>chunk</code></li><li>accounts: <strong>authority signer, StateBuffer, PendingCommitment</strong></li></ul> | Buffer authority | Writes a chunk of opened account data for finalize, operator response, or challenger reveal. |
| `FinalizeStateBuffer`<ul><li>ix-data: <code>role</code></li><li>accounts: <strong>authority signer, StateBuffer, PendingCommitment</strong></li></ul> | Buffer authority | Freezes a completed buffer after length and hash checks. Frozen buffers can be used by later instructions. |
| `RaiseChallenge`<ul><li>ix-data: <code>challenge</code></li><li>accounts: <strong>challenger signer, Challenge, PendingCommitment, ProtocolConfig</strong></li></ul> | Challenger | Locks challenger stake, records the hidden challenge hash, and blocks normal finalization until the challenge is resolved. |
| `OperatorChallengeResponse`<ul><li>ix-data: <code>state</code></li><li>accounts: <strong>operator signer, PendingCommitment, Challenge, optional StateBuffer</strong></li></ul> | Operator | Opens the operator's claimed state for the challenged commitment and starts the challenger reveal timeout. |
| `MarkOperatorTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, Challenge</strong></li></ul> | Cranker | Records that the operator missed the response deadline. The challenger must still reveal the challenge preimage. |
| `ChallengerReveal`<ul><li>ix-data: <code>state, salt</code></li><li>accounts: <strong>challenger signer, PendingCommitment, Challenge, optional StateBuffer, fee vault</strong></li></ul> | Challenger | Verifies the challenge preimage and opened state. It slashes invalid reveals, penalizes matching reveals, or moves mismatches to resolver decision. |
| `MarkChallengerRevealTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, Challenge, fee vault</strong></li></ul> | Cranker | Slashes challenger stake when the reveal deadline passes without a valid reveal. |
| `ResolveDispute`<ul><li>ix-data: <code>decision</code></li><li>accounts: <strong>resolver signer, Challenge, PendingCommitment, OperatorBond, fee vault, optional PayoutTimelock</strong></li></ul> | Resolver multisig | Applies the multisig decision for a valid mismatch: operator correct or challenger correct. |
| `FinalizeCommitment`<ul><li>ix-data: <code>state_source</code></li><li>accounts: <strong>finalizer, PendingCommitment, delegated account, DelegationRecord/metadata, StateBuffer, optional Challenge, ProtocolConfig</strong></li></ul> | Finalizer or cranker | Applies the final state after the happy path or after dispute resolution. |
| `ExtendChallengeWindow`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, ProtocolConfig</strong></li></ul> | Cranker | Extends an under-approved commitment according to config, or expires it after the maximum extensions. |
| `ClaimPayout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>beneficiary signer, PayoutTimelock</strong></li></ul> | Beneficiary | Pays the challenger reward after the timelock for a challenger-correct dispute. |
| `CloseTerminalAccounts`<ul><li>ix-data: <code>close_kind</code></li><li>accounts: <strong>recipient, account to close, terminal parent account</strong></li></ul> | Cranker or recipient | Closes terminal records and buffers after their parent commitment or challenge can no longer change. |

### Key Instruction Data

```rust
pub struct PostCommitmentData {
    pub commit_id: u64,
    pub lamports: u64,
    pub owner: Pubkey,
    pub data_hash: [u8; 32],
    pub da_pointer_hash: [u8; 32],
    pub er_slot: Option<u64>,
}

pub struct ConsumeCommitmentRandomnessData {
    /// VRF output bytes used to select verifiers.
    pub randomness: [u8; 32],
    pub pending_commitment: Pubkey,
}

pub enum CancelUnactivatedReason {
    RegistryRevisionChanged,
    VrfTimeout,
}

pub struct CancelUnactivatedCommitmentData {
    pub reason: CancelUnactivatedReason,
}

pub struct ApproveCommitmentData {
    pub selected_verifier_index: u32,
}

pub struct RaiseChallengeData {
    pub challenge_hash: [u8; 32],
    pub stake_lamports: u64,
}

pub struct OperatorChallengeResponseData {
    pub lamports: u64,
    pub owner: Pubkey,
    pub data_hash: [u8; 32],
    pub state_buffer: Option<Pubkey>,
}

pub struct ChallengerRevealData {
    pub lamports: u64,
    pub owner: Pubkey,
    pub data_hash: [u8; 32],
    /// Salt used by the challenger when computing challenge_hash.
    pub salt: [u8; 32],
    pub state_buffer: Option<Pubkey>,
}

pub enum DisputeDecision {
    OperatorCorrect,
    ChallengerCorrect,
}

pub struct ResolveDisputeData {
    pub decision: DisputeDecision,
}

pub enum FinalizeCommitmentStateSource {
    PendingOperatorState,
    ResolvedOperatorState,
    ResolvedChallengerState,
}

pub struct FinalizeCommitmentData {
    pub state_source: FinalizeCommitmentStateSource,
}
```

### Important Instruction Rules

- `PostCommitment` computes `state_commitment_hash`, stores the pending record,
  stores the current `VerifierRegistry.registry_revision`, and requests VRF.
  `dlp_program_identity_pda` is signed by DLP with `invoke_signed`.
  The VRF program id and DLP identity seed are fixed constants used by this
  instruction, not fields in `ProtocolConfig`.
- `ConsumeCommitmentRandomness` verifies the VRF identity signer, reads the
  `VerifierRegistry`, and requires its `registry_revision` to match the value
  stored on the pending commitment. If it matches, DLP derives
  `selected_verifiers` from randomness and starts the challenge window. The
  operator identity for this commitment must be excluded from selection, even if
  the same pubkey is also registered as a verifier.
  The VRF program identity is a fixed constant used by this instruction, not a
  field in `ProtocolConfig`.
- `CancelUnactivatedCommitment` handles commitments that are still
  `AwaitingRandomness` but can no longer activate. The common reasons are:
  registry changed before the VRF callback, or the VRF callback did not arrive
  before timeout. DLP verifies the reason on-chain before cancellation.
  Cancellation releases commitment-local locks, if any, and the operator must
  repost.
- `UpdateVerifierRegistry` mutates the registry and increments
  `registry_revision`. It affects future commitments and unactivated
  commitments only; it does not change `selected_verifiers` already stored on an
  active commitment.
- `ApproveCommitment` requires the verifier to have an active bond, be selected
  for this commitment, and still be inside the challenge window. The instruction
  data points to the verifier's index in `selected_verifiers`. Duplicate
  approvals do not increment `approval_count`.
- `ChallengerReveal` has four terminal branches:
  invalid hash, matching state, mismatch after operator response, valid reveal
  after operator timeout.
- `ResolveDispute` requires the configured `resolver` signer from
  `ProtocolConfig`. In DLP v2 this signer is expected to be a multisig-controlled
  account.
- `FinalizeCommitment` on the happy path requires closed window, approval
  threshold, no unresolved challenge, and full-state hash match.
- `ResolvedOperator` finalizes operator-opened state.
- `ResolvedChallenger` finalizes challenger-opened state.

### Failure Scenarios

| Scenario | DLP behavior |
| --- | --- |
| Registry changes after `PostCommitment` but before VRF activation | `CancelUnactivatedCommitment` cancels the pending record. The operator reposts against the latest `registry_revision`. |
| VRF callback never arrives | `CancelUnactivatedCommitment` cancels after timeout. No verifier selection happens. |
| Wrong registry account is passed | Reject. The registry account must match `PendingCommitment.verifier_registry`. |
| Registry update includes duplicate, unbonded, or inactive verifiers | Reject. `registry_revision` changes only after a valid update. |
| Registry changes after VRF activation | No effect on this commitment. Approvals use stored `selected_verifiers`. |
| Selected verifier is slashed or exits before approval | Reject that approval. Under-approval handling decides whether to extend or expire. |

## Flows

### Happy Path

1. Operator posts commitment.
2. DLP requests VRF.
3. VRF callback selects verifiers and starts the challenge window.
4. Selected verifiers approve.
5. Window closes without challenge.
6. Finalizer opens full state and finalizes.

### Challenge Paths

| Case | Flow |
| --- | --- |
| Invalid reveal | Challenger raises, operator responds, challenger preimage fails, challenger stake slashed, commitment returns to normal finalization. |
| Matching state | Challenger raises, operator responds, challenger reveals same state, challenger pays match penalty, commitment returns to normal finalization. |
| Mismatch | Challenger raises, operator responds, challenger reveals different valid state, resolver multisig decides, winning state finalizes. |
| Operator timeout | Challenger raises, operator misses deadline, timeout recorded, challenger reveals, resolver multisig decides with non-response evidence. |
| Challenger timeout | Challenger raises, operator responds, challenger misses reveal deadline, challenger stake slashed, commitment returns to normal finalization. |

### Dispute Resolution

DLP v2 uses a multisig as the resolver. DLP does not know which state is correct
by itself. The people behind the multisig must verify the dispute before signing
`ResolveDispute`.

For each mismatch case, the resolver process should:

1. Fetch the DA record referenced by the commitment.
2. Verify the DA bytes match the committed `da_pointer_hash`.
3. Reconstruct the pre-commit state from the last finalized base-layer state.
4. Replay the ER transactions/events with the deterministic runtime and config.
5. Produce canonical `lamports`, `owner`, and `data_hash`.
6. Compare the replay result with the operator-opened and challenger-opened
   states.
7. Submit a multisig transaction that calls `ResolveDispute`.

The multisig is where voting/approval happens. DLP only sees the final multisig
signature. If the configured `resolver` signed, DLP applies the outcome:
`OperatorCorrect` or `ChallengerCorrect`.

If DA is unavailable or replay inputs are insufficient, the resolver should not
guess. The protocol needs a deterministic policy for that case. The likely
policy is operator fault when the operator's own committed DA pointer cannot
support replay, but this remains an open design point.

### Under-Approval

If the window closes below threshold, `ExtendChallengeWindow` applies the
configured extension/threshold policy. If maximum extensions are exceeded, the
commitment expires.

## Validator Repo Responsibilities

- Operator: compute hashes, post commitments, write state buffers, respond to
  challenges.
- Verifier: watch selections, fetch DA, replay execution, approve or challenge.
- Challenger: detect divergence, generate salted challenge hash, write reveal
  buffer, reveal before timeout.
- Resolver tooling: fetch DA, run deterministic replay, present opened states and
  replay result, prepare the multisig transaction.
- Cranker: call timeout, extension, resolution, finalization, payout, and close
  instructions.

## Design FAQ

**Why is there a VerifierRegistry but no OperatorRegistry?**

DLP randomly selects verifiers, so it needs a selectable verifier list.
Operators are not selected by DLP. Operators submit commitments themselves, so
DLP only needs to derive and check the operator's `OperatorBond` PDA.

**Can the same identity be both operator and verifier?**

Yes. The same pubkey can register for both roles, but it must register each role
separately: `RegisterOperator` creates `OperatorBond`, and `RegisterVerifier`
creates `VerifierBond`.

**Can an operator verify its own commitment?**

No. `ConsumeCommitmentRandomness` must exclude the commitment operator from
`selected_verifiers`, even if the same pubkey is registered as a verifier.

**Why keep VerifierBond separate from VerifierRegistry?**

`VerifierBond` owns verifier stake and lifecycle. `VerifierRegistry` is only the
selectable verifier list used during verifier selection.

## Open Design Points

- Hash function and byte serialization.
- DA pointer wire format.
- Uniform vs stake-weighted verifier selection.
- Bounded vector vs Merkleized verifier registry.
- Resolver no-decision policy.
- DA-unavailable or replay-insufficient policy.
- Operator slash amount and challenger payout amount.
- Whether verifier slashing for bad approvals is in initial DLP v2 or later.
- Which multisig program/account signs as `resolver`.
