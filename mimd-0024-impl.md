# MIMD-0024 Implementation Notes

Companion to the
[MIMD-0024 proposal](https://github.com/magicblock-labs/magicblock-validator/discussions/1207).
This file only captures low-level implementation choices and message shapes.
Protocol rationale stays in the MIMD.

## Contents

- [Decisions To Review](#decisions-to-review)
- [Permissioned vs Permissionless](#permissioned-vs-permissionless)
- [Hashes](#hashes)
- [Accounts](#accounts)
- [Instructions](#instructions)
  - [Bootstrap Instructions](#bootstrap-instructions)
  - [Core Runtime Instructions](#core-runtime-instructions)
  - [Low-Priority Instructions](#low-priority-instructions)
  - [Obsolete Or Remove-Candidate Instructions](#obsolete-or-remove-candidate-instructions)
  - [State Buffer Plan](#state-buffer-plan)
  - [Key Instruction Data](#key-instruction-data)
  - [Important Instruction Rules](#important-instruction-rules)
  - [Failure Scenarios](#failure-scenarios)
- [Flows](#flows)
  - [Dispute Resolution](#dispute-resolution)
- [Validator Repo Responsibilities](#validator-repo-responsibilities)
- [Design FAQ](#design-faq)
- [Open Design Points](#open-design-points)

## Decisions To Review

- Select verifiers with round-robin for DLP v2 bootstrap.
- Operator writes and finalizes the state buffer before posting a commitment.
- Start the challenge window when the commitment is posted.
- DLP v2 verifier approvals are normal Solana signer transactions.
- `PendingCommitment` stores hashes/metadata; full data is opened via
  `StateBuffer` for finalize or dispute resolution.
- A multisig resolves disputes. DLP only checks that the configured resolver
  signed `ResolveDispute`, then applies the chosen outcome.
- DLP v2 supports one active challenge per pending commitment.
- `RaiseChallenge` consumes a finalized challenger buffer; no separate
  challenge-phase upload instruction is needed for v2 MVP.

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
- keep buffer writing, commitment, approval, challenge, resolution, and
  finalization logic independent of whether actor admission is permissioned or
  permissionless.

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

challenge_state_hash = H(
  "magicblock.challenge_state.v1",
  challenger_identity,
  account_pubkey,
  commit_id,
  challenger_account_state_hash
)
```

Open parameters: hash function, serialization, DA pointer format,
missing-account representation, economics, timeouts, and thresholds.

## Accounts

Seed strings are placeholders until frozen.

| Account | PDA seeds | Purpose |
| --- | --- | --- |
| `ProtocolConfig` | `["protocol-config"]` | Global params, resolver signer, protocol fee vault. |
| `OperatorBond` | `["operator-bond", operator]` | Slashable operator stake and lifecycle. |
| `VerifierBond` | `["verifier-bond", verifier]` | Slashable verifier stake. |
| `VerifierRegistry` | `["verifier-registry"]` | All registered verifiers. |
| `PendingCommitment` | `["pending-commitment", account, commit_id]` | Main commitment state machine. |
| `StateBuffer` | `["state-buffer", account, commit_id, authority]` | Chunked full account data opened by operator or challenger. |
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
    /// Slots available for approval/challenge after commitment post.
    pub challenge_window_slots: u64,
    /// Delay before a winning challenger can claim payout.
    pub payout_timelock_slots: u64,
    /// Maximum number of verifiers selected for one commitment.
    pub verifiers_per_commitment: u16,
    /// Approvals required for happy-path finalization.
    pub approval_threshold: u16,
    /// Maximum under-approval extensions before the commitment expires.
    pub max_window_extensions: u16,
    /// Penalty charged when a challenged state matches the operator state.
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
    /// Pending commitments store this value when selected verifiers are copied
    /// from this registry.
    pub registry_revision: u64,
    /// Round-robin start cursor used by the next commitment selection.
    pub next_selection_index: u64,
    /// All registered verifiers DLP can select from.
    pub entries: Vec<VerifierRegistryEntry>,
}
// Review: account size must be bounded before implementation. If the verifier
// set can grow large, replace this Vec with a Merkleized or paged registry.

pub struct VerifierRegistryEntry {
    /// Verifier identity selectable by DLP.
    pub verifier_identity: Pubkey,
    /// Bond account proving this verifier has active stake.
    pub verifier_bond: Pubkey,
    /// Keep as 1 for equal-weight selection.
    pub weight: u64,
}
// Review: keep `weight` only if weighted selection is in scope. For equal
// selection, removing it makes selection easier to review.

pub enum PendingCommitmentStatus {
    /// Verifiers were selected and the challenge window is open.
    Active,
    /// A challenger opened a different state and resolver must decide.
    Challenged,
    /// Resolver chose which opened state can finalize.
    Resolved,
    /// Final state was applied to the base layer.
    Finalized,
    /// Commitment can no longer finalize.
    Expired,
    /// Commitment was cancelled before finalization.
    Cancelled,
}
pub enum ResolvedStateSource {
    /// Finalize using the operator state buffer used at PostCommitment.
    OperatorState,
    /// Finalize using challenger-opened state after resolution.
    ChallengerState,
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
    pub verifier_registry_revision: u64,
    /// Monotonic id for this approval/challenge window.
    pub challenge_window_id: u64,
    /// Slot when the commitment was posted.
    pub posted_slot: u64,
    /// Slot when verifier selection and the challenge window started.
    pub activation_slot: u64,
    /// Slot when approval/challenge window closes.
    pub challenge_window_end_slot: u64,
    /// Verifiers selected by round-robin for this commitment.
    /// Later registry changes do not rewrite this list.
    pub selected_verifiers: Vec<Pubkey>,
    /// One bit per selected verifier.
    pub approval_bitmap: Vec<u8>,
    /// Number of unique selected verifiers that approved.
    pub approval_count: u16,
    /// Threshold copied from ProtocolConfig when the commitment is posted.
    pub approval_threshold: u16,
    /// Active Challenge account, if any.
    pub active_challenge: Option<Pubkey>,
    /// Which opened state finalization must use after dispute resolution.
    /// `None` means happy-path finalization uses the operator state.
    pub resolved_state_source: Option<ResolvedStateSource>,
}
// Review: `da_pointer_hash` is not enough by itself. Verifiers/resolver need an
// independent way to fetch replay inputs, not only operator-provided data.
// Review: `challenge_window_id` should be kept only if window extensions or
// retries need an explicit round id.

/// PDA: `["state-buffer", account, commit_id, authority]`
/// Created by: first `WriteStateBuffer`.
/// Frozen by: final `WriteStateBuffer` chunk.
/// Closed by: `CloseTerminalAccounts`.
///
/// Fixed header for opened full account data.
///
/// Raw account data starts immediately after this header in the same account.
pub struct StateBuffer {
    /// Signer allowed to write this buffer.
    /// Operator authority is used for the commitment state.
    /// Challenger authority is used for the challenged state.
    pub authority: Pubkey,

    /// Account whose data is being opened.
    pub account_pubkey: Pubkey,

    /// Commitment this buffer belongs to.
    pub commit_id: u64,

    /// Hash of the raw uploaded data. Zero until finalized.
    pub data_hash: [u8; 32],

    /// Expected total byte length.
    pub total_len: u32,

    /// Bytes written so far.
    pub written_len: u32,

    /// Set only after the full buffer is written and data_hash is computed.
    pub finalized: bool,
}
// Review: `total_len` and account size need hard caps.

pub enum ChallengeStatus {
    /// Challenger state differs from operator state and resolver must decide.
    AwaitingResolver,
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
    /// Reveal matched operator state; challenger pays match penalty.
    MatchingStateChallengerPenalized,
    /// Resolver chose operator; challenger loses stake.
    OperatorCorrectChallengerSlashed,
    /// Resolver chose challenger; operator is slashed.
    ChallengerCorrectOperatorSlashed,
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
    /// Slot when challenge was raised.
    pub raised_slot: u64,
    /// Challenger-opened state being compared against PendingCommitment.
    pub challenger_state: OpenedState,
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

Some instructions are system-level instructions. They are still normal public
Solana instructions, but they are expected to be called by operator, verifier,
challenger, resolver, or cranker services rather than directly by end users.

### Bootstrap Instructions

Authority-gated setup and admission instructions for permissioned v2.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `InitProtocolConfig`<ul><li>ix-data: <code>params</code></li><li>accounts: <strong>authority signer, ProtocolConfig, VerifierRegistry</strong></li></ul> | Protocol authority | Creates the global config account and empty verifier registry. Stores bootstrap params such as resolver, fees, thresholds, and timeouts. |
| `UpdateProtocolConfig`<ul><li>ix-data: <code>params</code></li><li>accounts: <strong>authority signer, ProtocolConfig</strong></li></ul> | Protocol authority | Updates params used by future commitments. Existing pending commitments keep the values copied into their accounts. |
| `RegisterOperator`<ul><li>ix-data: <code>amount_lamports</code></li><li>accounts: <strong>operator signer, protocol authority signer, OperatorBond, ProtocolConfig</strong></li></ul> | Operator, protocol authority | Creates the per-operator `OperatorBond` PDA at `["operator-bond", operator]` and deposits slashable stake. Permissioned v2 requires configured approval before the operator can post commitments. |
| `RegisterVerifier`<ul><li>ix-data: <code>amount_lamports</code></li><li>accounts: <strong>verifier signer, protocol authority signer, VerifierBond, ProtocolConfig</strong></li></ul> | Verifier, protocol authority | Creates the per-verifier `VerifierBond` PDA at `["verifier-bond", verifier]` and deposits slashable stake. Permissioned v2 requires configured approval before the verifier can enter the registry. |
| `UpdateVerifierRegistry`<ul><li>ix-data: <code>update</code></li><li>accounts: <strong>authority signer, VerifierRegistry, VerifierBond accounts</strong></li></ul> | Protocol authority | Adds or removes verifier pubkeys in the single `VerifierRegistry` account and increments `registry_revision`. Invalid, duplicate, unbonded, or inactive verifiers are rejected. |

### Core Runtime Instructions

Commitment, approval, challenge, resolution, and finalization instructions.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `WriteStateBuffer`<ul><li>ix-data: <code>buffer_write</code></li><li>accounts: <strong>payer signer, authority signer, StateBuffer, delegated account, ProtocolConfig, system program</strong></li></ul> | Operator or challenger service | System-level instruction. Creates, grows, writes, and finalizes a DLP-owned state buffer. Operator buffer must be finalized before `PostCommitment`; challenger buffer must be finalized before `RaiseChallenge`. |
| `PostCommitment`<ul><li>ix-data: <code>commitment</code></li><li>accounts: <strong>operator signer, OperatorBond, PendingCommitment, finalized operator StateBuffer, delegated account, DelegationRecord, ProtocolConfig, VerifierRegistry</strong></li></ul> | Operator | Requires the finalized operator buffer, stores its `data_hash`, selects verifiers with round-robin, creates an active commitment, stores the current `registry_revision`, and starts the challenge window. |
| `ApproveCommitment`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>verifier signer, VerifierBond, PendingCommitment</strong></li></ul> | Selected verifier | Records approval from the selected verifier. v2 MVP requires exactly one selected verifier, so no verifier index is needed. Duplicate approvals do not increase the count. |
| `RaiseChallenge`<ul><li>ix-data: <code>challenge</code></li><li>accounts: <strong>challenger signer, Challenge, PendingCommitment, finalized challenger StateBuffer, ProtocolConfig</strong></li></ul> | Challenger | Requires the finalized challenger buffer, locks challenger stake, compares challenger state with operator state, and either penalizes a matching challenge or blocks normal finalization until resolver decides. |
| `ResolveDispute`<ul><li>ix-data: <code>decision</code></li><li>accounts: <strong>resolver signer, Challenge, PendingCommitment, OperatorBond, fee vault, optional PayoutTimelock</strong></li></ul> | Resolver multisig | Applies the multisig decision for a valid mismatch: operator state correct or challenger state correct. |
| `FinalizeCommitment`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>operator or cranker, PendingCommitment, delegated account, DelegationRecord/metadata, selected StateBuffer, optional Challenge</strong></li></ul> | Operator or cranker | Applies the operator state on the happy path, or the resolver-selected state after dispute resolution. |

### Low-Priority Instructions

These instructions can be skipped while implementing the first usable v2 flow.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `RequestStakeWithdrawal`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, OperatorBond or VerifierBond, ProtocolConfig</strong></li></ul> | Operator or verifier | Low priority. Marks bonded stake as exiting. The stake cannot be withdrawn until the configured delay passes and no locks remain. |
| `WithdrawStake`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, OperatorBond or VerifierBond, ProtocolConfig</strong></li></ul> | Operator or verifier | Low priority. Withdraws unlocked stake after the exit delay. Slashed or locked stake stays in the protocol. |
| `FinalizeStateBuffer`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>authority signer, StateBuffer, PendingCommitment</strong></li></ul> | Buffer authority | Low priority. Useful only if we want buffer finalization to be a separate instruction instead of the final `WriteStateBuffer` chunk freezing the buffer. |
| `ExtendChallengeWindow`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, ProtocolConfig</strong></li></ul> | Cranker | Low priority. Extends an under-approved commitment according to config, or expires it after the maximum extensions. Initial v2 can expire under-approved commitments instead. |
| `ClaimPayout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>beneficiary signer, PayoutTimelock</strong></li></ul> | Beneficiary | Low priority. Needed only if dispute payouts are delayed through `PayoutTimelock`; otherwise `ResolveDispute` can pay immediately. |
| `CloseTerminalAccounts`<ul><li>ix-data: <code>close_kind</code></li><li>accounts: <strong>recipient, account to close, terminal parent account</strong></li></ul> | Cranker or recipient | Low priority. Closes terminal records and buffers after their parent commitment or challenge can no longer change. |

### Obsolete Or Remove-Candidate Instructions

These instructions are not part of the current v2 MVP flow.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `OperatorChallengeResponse`<ul><li>ix-data: <code>state</code></li><li>accounts: <strong>operator signer, PendingCommitment, Challenge, optional StateBuffer</strong></li></ul> | Operator | Remove candidate. Operator state is already opened by `WriteStateBuffer` before `PostCommitment`. |
| `ChallengerReveal`<ul><li>ix-data: <code>state, salt</code></li><li>accounts: <strong>challenger signer, PendingCommitment, Challenge, optional StateBuffer, fee vault</strong></li></ul> | Challenger | Remove candidate. `RaiseChallenge` can consume the finalized challenger buffer directly. |
| `MarkOperatorTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, Challenge</strong></li></ul> | Cranker | Remove candidate. There is no post-challenge operator upload step in the simplified challenge flow. |
| `MarkChallengerRevealTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, Challenge, fee vault</strong></li></ul> | Cranker | Remove candidate. There is no separate challenger reveal step in the simplified challenge flow. |

### State Buffer Plan

`WriteStateBuffer` replaces the v1-style committor buffer for DLP v2 flows.
DLP owns the buffer account because the buffer is part of the fraud-proof state
machine, not only temporary transport.

The first write creates the `StateBuffer` PDA, writes the fixed header, and
allocates enough space for the first chunk. Later writes grow the same PDA as
needed and append more bytes. Each write must use `offset == written_len`; this
keeps the MVP simple and avoids a separate chunks bitmap account. Retries can
repeat the same offset if the previous transaction failed.

The account layout is:

```text
StateBuffer header
raw opened account data bytes
```

The writer is authority-specific:

- operator writes the commitment state before `PostCommitment`;
- challenger writes the challenged state before `RaiseChallenge`.

When `written_len == total_len`, `WriteStateBuffer` hashes the raw data and sets
`finalized = true`. After that, the buffer cannot be changed.

Consuming instructions must check:

- buffer PDA seeds match `account_pubkey`, `commit_id`, and `authority`;
- buffer is finalized;
- buffer commitment matches the `PendingCommitment`;
- authority matches the operator for `PostCommitment` and happy-path
  `FinalizeCommitment`;
- authority matches the challenger for `RaiseChallenge`.

Future optimization: allow out-of-order or parallel chunk writes by adding a
small bitmap inside the `StateBuffer` account. Do this only if sequential writes
are too slow.

### Key Instruction Data

```rust
pub struct WriteStateBufferData<'a> {
    pub commit_id: u64,
    pub total_len: u32,
    /// Must equal StateBuffer.written_len.
    pub offset: u32,
    pub chunk: &'a [u8],
}

pub struct PostCommitmentData {
    pub commit_id: u64,
    pub lamports: u64,
    pub owner: Pubkey,
    pub da_pointer_hash: [u8; 32],
    pub er_slot: Option<u64>,
}

pub struct RaiseChallengeData {
    pub lamports: u64,
    pub owner: Pubkey,
    pub stake_lamports: u64,
}

pub enum DisputeDecision {
    OperatorStateCorrect,
    ChallengerStateCorrect,
}

pub struct ResolveDisputeData {
    pub decision: DisputeDecision,
}
```

### Important Instruction Rules

- `WriteStateBuffer` must run before `PostCommitment` for the operator state.
  `PostCommitment` rejects if the operator buffer is missing, unfinished, or
  belongs to another account, commit id, or operator.
- `PostCommitment` reads `data_hash` from the finalized operator buffer,
  computes `account_state_hash` and `state_commitment_hash`, stores the pending
  record, stores the current `VerifierRegistry.registry_revision`, selects
  verifiers with round-robin, increments
  `VerifierRegistry.next_selection_index` by the number of scanned registry
  entries, and starts the challenge window.
- Verifier selection uses all registered verifiers except the commitment
  operator. If no verifier remains, `PostCommitment` rejects.
- v2 MVP requires `ProtocolConfig.verifiers_per_commitment == 1` and
  `ProtocolConfig.approval_threshold == 1`. `PostCommitment` therefore selects
  exactly one verifier; if no verifier other than the operator exists, it
  rejects. Multi-verifier approval can be added later by relaxing config
  validation and adding an index or bitmap to `ApproveCommitment`.
- `UpdateVerifierRegistry` mutates the registry and increments
  `registry_revision`. It affects future commitments only; it does not change
  `selected_verifiers` already stored on a pending commitment.
- `ApproveCommitment` requires the verifier to have an active bond, be the only
  selected verifier for this commitment, and still be inside the challenge
  window. Duplicate approvals do not increment `approval_count`.
- `RaiseChallenge` requires a finalized challenger buffer. If challenger state
  matches operator state, the challenge is terminal and the commitment can
  return to normal finalization. If challenger state differs, the commitment is
  blocked until `ResolveDispute`.
- `ResolveDispute` requires the configured `resolver` signer from
  `ProtocolConfig`. In DLP v2 this signer is expected to be a multisig-controlled
  account.
- `FinalizeCommitment` on the happy path requires closed window, approval
  threshold, no unresolved challenge, and full-state hash match.
- `FinalizeCommitment` after dispute resolution uses
  `PendingCommitment.resolved_state_source` to choose operator or challenger
  state.

### Failure Scenarios

| Scenario | DLP behavior |
| --- | --- |
| Wrong registry account is passed | Reject. The registry account must match `PendingCommitment.verifier_registry`. |
| Registry update includes duplicate, unbonded, or inactive verifiers | Reject. `registry_revision` changes only after a valid update. |
| No eligible verifier exists at `PostCommitment` | Reject. At least one verifier other than the operator must be selectable. |
| Registry changes after `PostCommitment` | No effect on this commitment. Approvals use stored `selected_verifiers`. |
| Selected verifier is slashed or exits before approval | Reject that approval. Under-approval handling decides whether to extend or expire. |
| Challenger buffer matches operator buffer | Penalize the challenger and allow normal finalization. |
| Challenger buffer differs from operator buffer | Block normal finalization until resolver chooses the state source. |

## Flows

### Happy Path

1. Operator writes and finalizes `StateBuffer`.
2. Operator posts commitment using that buffer.
3. DLP selects verifiers with round-robin and starts the challenge window.
4. Selected verifier approves.
5. Window closes without challenge.
6. `FinalizeCommitment` applies the operator state.

### Challenge Paths

| Case | Flow |
| --- | --- |
| Matching state | Challenger writes a buffer that matches operator state, calls `RaiseChallenge`, pays match penalty, and the commitment returns to normal finalization. |
| Mismatch | Challenger writes a different valid buffer, calls `RaiseChallenge`, resolver multisig decides, and `FinalizeCommitment` applies the winning state. |

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
6. Compare the replay result with the operator and challenger buffers.
7. Submit a multisig transaction that calls `ResolveDispute`.

The multisig is where voting/approval happens. DLP only sees the final multisig
signature. If the configured `resolver` signed, DLP applies the outcome:
`OperatorStateCorrect` or `ChallengerStateCorrect`.

If DA is unavailable or replay inputs are insufficient, the resolver should not
guess. The protocol needs a deterministic policy for that case. The likely
policy is operator fault when the operator's own committed DA pointer cannot
support replay, but this remains an open design point.

### Under-Approval

If the window closes below threshold, `ExtendChallengeWindow` applies the
configured extension/threshold policy. If maximum extensions are exceeded, the
commitment expires.

## Validator Repo Responsibilities

- Operator: write DLP state buffers, compute hashes, and post commitments.
- Verifier: watch selections, fetch DA, replay execution, approve or challenge.
- Challenger: detect divergence, write challenger state buffer, and raise
  challenge.
- Resolver tooling: fetch DA, run deterministic replay, present opened states and
  replay result, prepare the multisig transaction.
- Cranker: call extension/expiry, finalization, payout, and close instructions.

## Design FAQ

**Why is there a VerifierRegistry but no OperatorRegistry?**

DLP selects verifiers from a registered verifier list. For v2 MVP the selection
is round-robin, so DLP needs an ordered list plus a cursor.
Operators are not selected by DLP. Operators submit commitments themselves, so
DLP only needs to derive and check the operator's `OperatorBond` PDA.

**Can the same identity be both operator and verifier?**

Yes. The same pubkey can register for both roles, but it must register each role
separately: `RegisterOperator` creates `OperatorBond`, and `RegisterVerifier`
creates `VerifierBond`.

**Can an operator verify its own commitment?**

No. `PostCommitment` must exclude the commitment operator from
`selected_verifiers`, even if the same pubkey is registered as a verifier.

**Why keep VerifierBond separate from VerifierRegistry?**

`VerifierBond` owns verifier stake and lifecycle. `VerifierRegistry` is only the
selectable verifier list used during verifier selection.

**Why should `magicblock-committor-program` disappear after v2 migration?**

The committor program only creates, fills, and closes generic buffer accounts.
In DLP v2, opened state buffers are protocol state: they are tied to commitment
id, account, authority, hashes, dispute status, and cleanup rules.
DLP should own those checks directly instead of trusting a separate buffer
program for part of the lifecycle.

`magicblock-committor-service` should stay. It can still split bytes into
chunks, retry writes, prepare ALTs, and submit transactions. For v1 it can keep
using `magicblock-committor-program`; for v2 it should call DLP's state-buffer
instructions.

## Open Design Points

- Hash function and byte serialization.
- DA pointer wire format.
- Future verifier selection policy after round-robin MVP.
- Bounded vector vs Merkleized verifier registry.
- Resolver no-decision policy.
- DA-unavailable or replay-insufficient policy.
- Operator slash amount and challenger payout amount.
- Whether verifier slashing for bad approvals is in initial DLP v2 or later.
- Which multisig program/account signs as `resolver`.
