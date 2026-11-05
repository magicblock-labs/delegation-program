# MIMD-0024 Implementation Notes

Companion to the
[MIMD-0024 proposal](https://github.com/magicblock-labs/magicblock-validator/discussions/1207).
This file only captures low-level implementation choices and message shapes.
Protocol rationale stays in the MIMD.

## Contents

- [Decisions To Review](#decisions-to-review)
- [MIMD Drift](#mimd-drift)
- [Delegation Clusters](#delegation-clusters)
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
  - [Happy Path](#happy-path)
  - [Challenge Paths](#challenge-paths)
  - [Dispute Resolution](#dispute-resolution)
  - [Under-Approval](#under-approval)
- [Validator Repo Responsibilities](#validator-repo-responsibilities)
- [Design FAQ](#design-faq)
- [Open Design Points](#open-design-points)

## Decisions To Review

- Select verifiers with round-robin for DLP v2 bootstrap.
- Operator writes and finalizes the state buffer before posting a commitment,
  collapsing the MIMD's later operator-response phase for the v2 MVP.
- Start the challenge window when the commitment is posted.
- DLP v2 verifier approvals are normal Solana signer transactions.
- `PendingCommitment` stores hashes/metadata; full data is opened via
  `StateBuffer` for finalize or dispute resolution.
- A multisig resolves disputes. DLP only checks that the configured resolver
  signed `ResolveDispute`, then applies the chosen outcome.
- DLP v2 supports one active challenge per pending commitment.
- Keep challenger commit/reveal from the MIMD: `RaiseChallenge` records a
  salted challenge hash and locks stake; `ChallengerReveal` opens the
  challenged state later.
- Bind every v2 delegation to a `DelegationCluster`, which supplies the active
  validator and either omits or stores an immutable challenge configuration.

## MIMD Drift

This implementation plan intentionally differs from MIMD-0024 in one major
operator-side place.

The MIMD describes an operator response after a challenge if the pending
commitment does not already expose full state. For the v2 MVP, the operator must
write and finalize the DLP-owned `StateBuffer` before `ProposeCommitment`; the
commitment stores hashes and the buffer stays available for finalization or
dispute review. The operator/validator already has to upload these bytes for
finalization, so this mostly changes ordering. It also simplifies the state
machine and avoids a separate operator-response timeout path. This does not make
`WriteStateBuffer` operator-only; the buffer remains authority-specific, and
each consumer validates whether that authority is valid for its flow.

We should not apply the same collapse to challengers yet. `RaiseChallenge`
should stay close to the MIMD: it records a salted `challenge_hash`, locks
challenger stake, and blocks happy-path finalization. `ChallengerReveal` then
opens lamports, owner, salt, and data or a buffered data hash. This preserves
the purpose of commit/reveal and avoids forcing full challenger data upload
before the challenge is accepted.

## Delegation Clusters

The DLP program serves multiple ER deployments with different security and
finalization policies. There is therefore no program-wide protocol config.
Each ER security domain has a `DelegationCluster` account, and every v2
delegation references that account instead of a validator identity directly.

The `DelegationCluster` address is the stable delegation authority. Its
`cluster_id` is fixed when the account is initialized and is independent of the
validator key. The account stores the current validator identity, so rotating
that identity updates authorization for every account delegated to the cluster
without undelegating and redelegating those accounts. This follows the agreed
[validator-rotation discussion](https://github.com/magicblock-labs/dev-kanban/issues/194).

DLP is responsible for checking the current validator on commits,
undelegations, and other validator-authorized operations. A future
`RotateValidator` instruction will update the validator when invoked by the
configured rotation authority. Rotation orchestration, temporary encryption
keys, key handover, and transaction forwarding stay outside DLP.

Each cluster chooses one immutable finalization route through the presence of
its challenge configuration:

- `challenge_config: None` is Direct mode. It uses the v2 direct
  commit/finalize path and does not create challenge-specific accounts.
- `challenge_config: Some(...)` is Challenge mode. It requires
  `ProposeCommitment`, verifier approval, the challenge window, and
  `FinalizeCommitment`.

`FinalizationMode` is therefore a conceptual distinction, not a separately
serialized field. The presence of `challenge_config` is fixed when the cluster
is initialized, so an update cannot convert an existing Direct cluster into a
Challenge cluster or vice versa.

The existing v1 direct instructions continue to compare their signer with the
validator pubkey stored by a v1 delegation. They cannot authorize a v2
delegation because its authority is a PDA. The v2 direct commit/finalize path
must instead load that PDA, require `Direct`, and authorize
`current_validator`.

The mode is immutable because changing a challenge-enabled cluster to direct
finalization could bypass protection expected by existing delegations or open
commitments. Other policy fields may be updated for future commitments; values
needed by an already-open commitment must be copied into its protocol state.

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

So our DLP v2 "permissioned" challenge flow means:

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
  delegation_cluster,
  operator_identity,
  account_pubkey,
  commit_seq,
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
  commit_seq,
  challenger_lamports,
  challenger_owner,
  challenger_data_hash,
  salt
)
```

Open parameters: hash function, serialization, DA pointer format,
missing-account representation, economics, timeouts, and thresholds.

## Accounts

Seed strings are placeholders until frozen.

| Account | PDA seeds | Purpose |
| --- | --- | --- |
| `DelegationCluster` | `["delegation-cluster", cluster_id]` | Stable ER identity, active validator, and optional challenge policy. |
| `OperatorBond` | `["operator-bond", delegation_cluster]` | Cluster-scoped operator stake for the challenge flow that survives validator rotation. |
| `VerifierBond` | `["verifier-bond", delegation_cluster, verifier]` | Verifier stake isolated to one cluster. |
| `VerifierRegistry` | `["verifier-registry", delegation_cluster]` | Verifiers selectable for one challenge-enabled cluster. |
| `PendingCommitment` | `["pending-commitment", account, commit_seq]` | Main commitment state machine. |
| `StateBuffer` | `["state-buffer", account, commit_seq, authority]` | Chunked state payload opened by an operator or challenger. The payload may be full `Account::data` bytes or an encoded diff. |
| `Challenge` | `["challenge", account, commit_seq, challenger]` | One challenge against one pending commitment. |
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

/// Challenge-only policy stored by clusters that opt into fraud proofs.
pub struct ChallengeConfig {
    /// Multisig-controlled signer allowed to call ResolveDispute.
    pub resolver: Pubkey,
    /// Minimum stake required for an operator to register and stay active.
    pub min_operator_bond: u64,
    /// Minimum stake required for a verifier to register and stay active.
    pub min_verifier_bond: u64,
    /// Minimum stake locked by RaiseChallenge to prevent cheap spam.
    pub min_challenger_stake: u64,
    /// Slots available for approval/challenge after commitment post.
    pub challenge_window_slots: u64,
    /// Operator-response timeout retained in the configuration schema.
    pub operator_response_timeout_slots: u64,
    /// Slots available for the challenger to reveal its committed state.
    pub challenger_reveal_timeout_slots: u64,
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

/// PDA: `["delegation-cluster", cluster_id]`
/// Created by: `InitDelegationCluster`.
/// Closed by: not normally closed.
pub struct DelegationCluster {
    /// Account type marker.
    pub discriminator: [u8; 8],
    /// Canonical PDA bump for this account.
    pub bump: u8,
    /// Stable identifier used to derive this cluster account.
    pub cluster_id: Pubkey,
    /// Signer allowed to update cluster policy and permissioned bootstrap state.
    pub config_authority: Pubkey,
    /// Signer allowed to replace `current_validator`.
    pub validator_rotation_authority: Pubkey,
    /// Validator currently authorized to act for this cluster.
    pub current_validator: Pubkey,
    /// Emergency stop for new commitments and other non-exit activity.
    pub paused: bool,
    /// Vault receiving protocol fees, penalties, or slashed funds.
    pub protocol_fee_vault: Pubkey,
    /// `None` selects Direct mode; `Some` selects Challenge mode.
    /// The field is trailing and flexible, so Direct clusters do not allocate
    /// account bytes for challenge-only policy.
    #[flexible]
    pub challenge_config: Option<ChallengeConfig>,
}
// Review: `protocol_fee_vault` may be too vague if it also holds slashed funds
// and penalties.
// Review: fields copied into PendingCommitment or Challenge must not be read
// from DelegationCluster later for already-open records.

/// PDA: `["operator-bond", delegation_cluster]`
/// Created by: `RegisterOperator`.
/// Closed by: `WithdrawStake` after exit, zero stake, and no locks.
///
/// One account per cluster. Validator rotation does not move the bonded stake.
pub struct OperatorBond {
    /// Delegation cluster whose operator stake this account holds.
    pub delegation_cluster: Pubkey,
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

/// PDA: `["verifier-bond", delegation_cluster, verifier]`
/// Created by: `RegisterVerifier`.
/// Closed by: `WithdrawStake` after exit, zero stake, no locks, and registry removal.
///
/// One account per verifier per cluster. Selection also requires presence in
/// that cluster's VerifierRegistry.
pub struct VerifierBond {
    /// Cluster in which this verifier and stake are registered.
    pub delegation_cluster: Pubkey,
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

/// PDA: `["verifier-registry", delegation_cluster]`
/// Created by: `InitDelegationCluster` for `Challenge` mode.
/// Updated by: `UpdateVerifierRegistry`.
/// Closed by: not normally closed.
///
/// One cluster-scoped registry containing all verifiers that can be selected.
pub struct VerifierRegistry {
    /// Cluster whose commitments select from this registry.
    pub delegation_cluster: Pubkey,
    /// Round-robin start cursor used by the next commitment selection.
    pub next_selection_index: u64,
    /// All registered verifiers DLP can select for this cluster.
    pub entries: Vec<VerifierRegistryEntry>,
}
// Review: the MVP registry is not bounded. Before production, cap its entry
// count or replace this Vec with a Merkleized or paged registry.

pub struct VerifierRegistryEntry {
    /// Verifier identity selectable by DLP.
    pub verifier_identity: Pubkey,
    /// Bonded stake copied when this entry was last updated.
    pub stake_lamports: u64,
    /// Keep as 1 for equal-weight selection.
    pub weight: u64,
}
// Review: every bond-changing instruction must keep `stake_lamports`
// synchronized until selection reads the bond accounts directly.
// Review: keep `weight` only if weighted selection is in scope. For equal
// selection, removing it makes selection easier to review.

pub enum PendingCommitmentStatus {
    /// Verifiers were selected and the challenge window is open.
    Active,
    /// Reserved for the MIMD operator-response phase omitted by this MVP.
    AwaitingOperatorResponse,
    /// A challenge was raised and its hidden state must be revealed.
    AwaitingChallengerReveal,
    /// Challenger state differs and the resolver must decide.
    AwaitingDisputeResolution,
    /// Resolver selected the operator commitment.
    ResolvedOperator,
    /// Resolver selected the challenger reveal.
    ResolvedChallenger,
    /// Final state was applied to the base layer.
    Finalized,
    /// Commitment can no longer finalize.
    Expired,
    /// Commitment was cancelled before finalization.
    Cancelled,
}
pub enum ResolvedStateSource {
    /// Finalize using the operator state buffer used at ProposeCommitment.
    OperatorCommitment,
    /// Finalize using challenger-opened state after resolution.
    ChallengerReveal,
}

/// PDA: `["pending-commitment", account, commit_seq]`
/// Created by: `ProposeCommitment`.
/// Closed by: `CloseTerminalAccounts` after finalize, cancel, or expiry.
///
/// One account per delegated account and commit sequence number.
pub struct PendingCommitment {
    /// Current state-machine state for this commitment.
    pub status: PendingCommitmentStatus,
    /// Cluster under which this commitment was posted.
    pub delegation_cluster: Pubkey,
    /// Validator identity that posted the commitment.
    pub operator_identity: Pubkey,
    /// Cluster OperatorBond checked when the commitment was posted.
    pub operator_bond: Pubkey,
    /// Delegated account whose base-layer state will be finalized.
    pub account_pubkey: Pubkey,
    /// Monotonically increasing sequence number for this account's commitments.
    pub commit_seq: u32,
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
    /// Hash binding cluster, validator, account, commit sequence number,
    /// delegation, DA, and state.
    pub state_commitment_hash: [u8; 32],
    /// Registry account used when this commitment was posted.
    pub verifier_registry: Pubkey,
    /// Monotonic id for this approval/challenge window.
    pub challenge_window_id: u64,
    /// Slot when the commitment was posted.
    pub posted_slot: u64,
    /// Slot when verifier selection and the challenge window started.
    pub activation_slot: u64,
    /// Slot when approval/challenge window closes.
    pub challenge_window_end_slot: u64,
    /// Number of unique selected verifiers that approved.
    pub approval_count: u16,
    /// Threshold copied from DelegationCluster when the commitment is posted.
    pub approval_threshold: u16,
    /// Resolver copied from DelegationCluster when the commitment is posted.
    pub resolver: Pubkey,
    /// Minimum challenge stake copied when the commitment is posted.
    pub min_challenger_stake: u64,
    /// Reveal timeout copied when the commitment is posted.
    pub challenger_reveal_timeout_slots: u64,
    /// Matching-state penalty copied when the commitment is posted.
    pub match_penalty_bps: u16,
    /// Active Challenge account, if any.
    pub active_challenge: Option<Pubkey>,
    /// Which opened state finalization must use after dispute resolution.
    /// `None` means happy-path finalization uses the operator state.
    pub resolved_state_source: Option<ResolvedStateSource>,
    /// ER slot that produced this commitment, when available.
    pub er_slot: Option<u64>,
    /// Verifiers selected by round-robin for this commitment.
    /// Later registry changes do not rewrite this list.
    pub selected_verifiers: Vec<SelectedVerifier>,
}

pub struct SelectedVerifier {
    /// Selected verifier identity.
    pub verifier_identity: Pubkey,
    /// Whether this verifier approved this commitment.
    pub approved: bool,
}
// Review: `da_pointer_hash` is not enough by itself. Verifiers/resolver need an
// independent way to fetch replay inputs, not only operator-provided data.
// Review: `challenge_window_id` should be kept only if window extensions or
// retries need an explicit round id.

/// PDA: `["state-buffer", account, commit_seq, authority]`
/// Created by: first `WriteStateBuffer`.
/// Frozen by: final `WriteStateBuffer` chunk.
/// Closed by: `CloseStateBuffer` when no live commitment needs it.
///
/// Header and extendable bytes for an opened state payload.
pub struct StateBuffer {
    /// Signer allowed to write this buffer.
    /// Operator authority is used for the commitment state.
    /// Challenger authority is used for the challenged state.
    pub authority: Pubkey,

    /// Initial payer that receives the buffer's reclaimed rent.
    pub rent_payer: Pubkey,

    /// Account whose state payload is being opened.
    pub account_pubkey: Pubkey,

    /// Commitment this buffer belongs to.
    pub commit_seq: u32,

    /// Hash of the uploaded payload. Zero until finalized.
    pub data_hash: [u8; 32],

    /// Expected total byte length.
    pub total_len: u32,

    /// Set only after the full buffer is written and data_hash is computed.
    pub finalized: bool,

    /// Active payload bytes written so far. `payload.len()` is the written
    /// length; account-backed capacity may be larger while writing proceeds.
    pub payload: Vec<u8>,
}
// The implementation caps both total account size and growth per write.

pub enum ChallengeStatus {
    /// Challenger has committed to a hash and must reveal the state.
    AwaitingReveal,
    /// Challenger state differs from operator state and resolver must decide.
    AwaitingResolver,
    /// Challenge has an outcome and can be closed when allowed.
    Terminal,
}

pub enum ChallengeOutcome {
    /// No terminal outcome has been selected yet.
    None,
    /// Reveal did not open the committed challenge hash.
    InvalidReveal,
    /// Reveal matched operator state; challenger pays match penalty.
    MatchingStateChallengerPenalized,
    /// Resolver chose operator; challenger loses stake.
    OperatorCorrectChallengerSlashed,
    /// Resolver chose challenger; operator is slashed.
    ChallengerCorrectOperatorSlashed,
}

/// PDA: `["challenge", account, commit_seq, challenger]`
/// Created by: `RaiseChallenge`.
/// Closed by: `CloseTerminalAccounts` after a terminal challenge outcome.
///
/// One active challenge is allowed per pending commitment in DLP v2.
pub struct Challenge {
    /// Current state-machine state for this challenge.
    pub status: ChallengeStatus,
    /// Terminal result, or None before resolution.
    pub outcome: ChallengeOutcome,
    /// PendingCommitment being challenged.
    pub pending_commitment: Pubkey,
    /// Challenger that posted the hidden challenge hash.
    pub challenger_identity: Pubkey,
    /// State commitment hash copied from PendingCommitment at raise time.
    pub state_commitment_hash: [u8; 32],
    /// Salted hash binding the challenger state to this challenge.
    pub challenge_hash: [u8; 32],
    /// Challenger-opened lamports value. Zero until reveal.
    pub challenger_lamports: u64,
    /// Challenger-opened owner. Default pubkey until reveal.
    pub challenger_owner: Pubkey,
    /// Challenger-opened payload hash. Zero until reveal.
    pub challenger_data_hash: [u8; 32],
    /// Challenger StateBuffer used for reveal. Default pubkey until reveal.
    pub challenger_state_buffer: Pubkey,
    /// Stake locked by this challenge.
    pub challenger_stake_lamports: u64,
    /// Slot when challenge was raised.
    pub raised_slot: u64,
    /// Slot after which an unrevealed challenge can be timed out.
    pub reveal_deadline_slot: u64,
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

Each v2 instruction starts with a single `u8` tag. The `ix-data` values below
describe only the instruction args after that tag. Existing v1 instruction
framing and account discriminators remain 8 bytes.

Some instructions are system-level instructions. They are still normal public
Solana instructions, but they are expected to be called by operator, verifier,
challenger, resolver, or cranker services rather than directly by end users.

### Bootstrap Instructions

Authority-gated setup and admission instructions for permissioned v2.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `InitDelegationCluster`<ul><li>ix-data: <code>current_validator, validator_rotation_authority, challenge_config</code></li><li>accounts: <strong>config authority signer, cluster identity signer, DelegationCluster, system program, optional VerifierRegistry</strong></li></ul> | Cluster config authority and cluster identity | Creates one cluster and stores its stable identity, active validator, and optional challenge policy. The signing cluster identity supplies `cluster_id`. `Some(challenge_config)` requires and creates the trailing cluster registry; `None` omits that account. |
| `UpdateDelegationCluster`<ul><li>ix-data: <code>challenge_config</code></li><li>accounts: <strong>config authority signer, DelegationCluster</strong></li></ul> | Cluster config authority | Updates mutable policy used by future commitments. It cannot change `cluster_id`, switch the presence of `challenge_config`, or replace the validator through this instruction. Existing pending commitments keep the values copied into their accounts. |
| `RegisterOperator`<ul><li>ix-data: <code>stake_lamports</code></li><li>accounts: <strong>current validator signer, config authority signer, OperatorBond, DelegationCluster</strong></li></ul> | Current validator, cluster config authority | Requires `Challenge` mode, creates the cluster's `OperatorBond` PDA at `["operator-bond", delegation_cluster]`, and deposits slashable stake. The bond remains with the cluster when its validator rotates. |
| `RegisterVerifier`<ul><li>ix-data: <code>stake_lamports</code></li><li>accounts: <strong>verifier signer, config authority signer, VerifierBond, DelegationCluster</strong></li></ul> | Verifier, cluster config authority | Requires `Challenge` mode, creates a cluster-scoped `VerifierBond` PDA at `["verifier-bond", delegation_cluster, verifier]`, and deposits slashable stake. Permissioned v2 requires configured approval before the verifier can enter that cluster's registry. |
| `UpdateVerifierRegistry`<ul><li>ix-data: <code>action, weight</code></li><li>accounts: <strong>config authority signer, DelegationCluster, VerifierRegistry, VerifierBond, system program</strong></li></ul> | Cluster config authority | Requires `Challenge` mode. The MVP implements only `Add` with weight 1; `Remove` remains deferred. Cross-cluster bonds, invalid entries, duplicates, and inactive verifiers are rejected. |

### Core Runtime Instructions

Commitment, approval, challenge, resolution, and finalization instructions.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `WriteStateBuffer`<ul><li>ix-data: <code>commit_seq, total_len, offset, chunk</code></li><li>accounts: <strong>payer signer, authority signer, StateBuffer, delegated account, system program</strong></li></ul> | Authority for a known state-buffer flow | System-level instruction. Creates, grows, writes, and finalizes a DLP-owned `StateBuffer` PDA using the fixed `["state-buffer", account, commit_seq, authority]` seed pattern. The instruction remains cluster-neutral; consuming instructions validate the payload, authority, and cluster relationship. |
| `CloseStateBuffer`<ul><li>ix-data: <code>commit_seq, state_buffer_bump, pending_commitment_bump</code></li><li>accounts: <strong>authority signer, StateBuffer, delegated account, stored rent payer, PendingCommitment</strong></li></ul> | Buffer authority | Closes a consumed or abandoned buffer and returns its rent to the payer recorded by the first write. Cleanup is idempotent and rejects buffers still needed by a live commitment. |
| `ProposeCommitment`<ul><li>ix-data: <code>commit_seq, lamports, owner, da_pointer_hash, er_slot</code></li><li>accounts: <strong>current validator signer, OperatorBond, PendingCommitment, finalized operator StateBuffer, delegated account, DelegationRecord, DelegationMetadata, DelegationCluster, VerifierRegistry, system program</strong></li></ul> | Current validator | Requires `Challenge` mode, verifies the delegation's cluster and current validator, consumes only cluster-scoped bond and registry accounts, selects verifiers, and starts the challenge window. `DelegationRecord` and `DelegationMetadata` are temporary v1-layout dependencies until their v2 replacement exists. |
| `ApproveCommitment`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>verifier signer, VerifierBond, PendingCommitment</strong></li></ul> | Selected verifier | Records approval from the selected verifier. v2 MVP requires exactly one selected verifier, so no verifier index is needed. Duplicate approvals do not increase the count. |
| `RaiseChallenge`<ul><li>ix-data: <code>state_commitment_hash, challenge_hash, stake_lamports</code></li><li>accounts: <strong>challenger signer, Challenge, PendingCommitment, DelegationCluster, system program</strong></li></ul> | Challenger | Requires the cluster stored by the pending commitment, locks challenger stake, stores the salted `challenge_hash`, and blocks normal finalization until reveal, timeout, or resolution. It does not consume challenger state bytes. |
| `ChallengerReveal`<ul><li>ix-data: <code>lamports, owner, data_hash, salt</code></li><li>accounts: <strong>challenger signer, Challenge, PendingCommitment, finalized operator StateBuffer, finalized challenger StateBuffer, DelegationCluster, protocol fee vault</strong></li></ul> | Challenger | Opens the challenged state and salt, validates both state buffers and the stored `challenge_hash`, penalizes invalid or matching reveals, or moves mismatches to resolver decision. |
| `ResolveDispute`<ul><li>ix-data: <code>decision</code></li><li>accounts: <strong>resolver signer, Challenge, PendingCommitment, OperatorBond, challenger, DelegationCluster, protocol fee vault</strong></li></ul> | Resolver multisig | Requires the resolver and cluster stored by the pending commitment, then applies the decision immediately. Operator-correct sends challenger stake to the fee vault; challenger-correct refunds challenger stake and sends the operator's recorded stake to the fee vault. No payout timelock is created by the MVP. |
| `FinalizeCommitment`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>selected state authority signer, PendingCommitment, delegated account, DelegationRecord, DelegationMetadata, selected StateBuffer, DelegationCluster, system program, optional Challenge</strong></li></ul> | Operator on the happy/operator-resolved path; challenger on the challenger-resolved path | Requires `Challenge` mode and the delegation-bound cluster, validates that the signer owns the selected state buffer, then applies the operator state on the happy path or the resolver-selected state after dispute resolution. |

### Low-Priority Instructions

These instructions can be skipped while implementing the first usable v2 flow.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `RotateValidator`<ul><li>ix-data: <code>new_validator</code></li><li>accounts: <strong>validator rotation authority signer, DelegationCluster</strong></li></ul> | External rotation controller | Deferred. Replaces the cluster's current validator. DLP enforces the signer and account update; key handover and transaction forwarding are external responsibilities. |
| `RequestStakeWithdrawal`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, OperatorBond or VerifierBond, DelegationCluster</strong></li></ul> | Operator or verifier | Low priority. Marks cluster-scoped stake as exiting. The stake cannot be withdrawn until the configured delay passes and no locks remain. |
| `WithdrawStake`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, OperatorBond or VerifierBond, DelegationCluster</strong></li></ul> | Operator or verifier | Low priority. Withdraws unlocked stake after the exit delay. Slashed or locked stake stays in the protocol. |
| `ExtendChallengeWindow`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, DelegationCluster</strong></li></ul> | Cranker | Low priority. Extends an under-approved commitment according to the commitment's cluster policy, or expires it after the maximum extensions. Initial v2 can expire under-approved commitments instead. |
| `MarkChallengerRevealTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, Challenge, fee vault</strong></li></ul> | Cranker | Deferred. Closes an unrevealed challenge after the reveal timeout, applies the configured challenger penalty, and unblocks or expires the pending commitment according to final policy. |
| `ClaimPayout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>beneficiary signer, PayoutTimelock</strong></li></ul> | Beneficiary | Low priority. Needed only if dispute payouts are delayed through `PayoutTimelock`; otherwise `ResolveDispute` can pay immediately. |
| `CloseTerminalAccounts`<ul><li>ix-data: <code>close_kind</code></li><li>accounts: <strong>recipient, account to close, terminal parent account</strong></li></ul> | Cranker or recipient | Low priority. Closes the remaining terminal commitment and challenge accounts after their parent state can no longer change. `StateBuffer` cleanup uses `CloseStateBuffer`. |

### Obsolete Or Remove-Candidate Instructions

These instructions are not part of the current v2 MVP flow.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `OperatorChallengeResponse`<ul><li>ix-data: <code>state</code></li><li>accounts: <strong>operator signer, PendingCommitment, Challenge, optional StateBuffer</strong></li></ul> | Operator | Remove candidate. Operator state is already opened by `WriteStateBuffer` before `ProposeCommitment`. |
| `MarkOperatorTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, PendingCommitment, Challenge</strong></li></ul> | Cranker | Remove candidate. There is no post-challenge operator upload step in the simplified challenge flow. |

### State Buffer Plan

`WriteStateBuffer` replaces the v1-style committor buffer for DLP v2 flows.
DLP owns the buffer account because the buffer is part of the fraud-proof state
machine, not only temporary transport.

The first write creates the `StateBuffer` PDA, records its rent payer, writes the
fixed fields, and allocates up to the maximum account growth allowed in one
instruction. Later writes grow the same PDA by at most that limit and append
more bytes. Each new write must use `offset == payload.len()`; this keeps the MVP
simple and avoids a separate chunks bitmap account. An exact old chunk may be
repeated as an idempotent retry.

The account layout is:

```text
StateBuffer fixed fields
extendable payload length and capacity metadata
active payload bytes
```

`WriteStateBuffer` is authority-specific, not operator-specific. It must only
create and write accounts in the `StateBuffer` namespace, using the fixed seed
pattern:

```text
["state-buffer", account, commit_seq, authority]
```

The header must bind the same `authority`, `account_pubkey`, and `commit_seq`.
The buffer is inert until a consuming instruction validates the authority role:

- `ProposeCommitment` accepts a finalized buffer whose authority is the operator;
- happy-path `FinalizeCommitment` uses the operator buffer recorded by the
  commitment path;
- `ChallengerReveal` accepts a challenger buffer only when large revealed data
  needs buffered transport, and validates authority against the challenger.

If future protocol flows need multiple buffer purposes for the same
`account + commit_seq + authority`, add an explicit `buffer_kind` or purpose seed
and matching header field. Reusing the same address space for unrelated opened
state can create collisions or confused-consumer bugs.

When `payload.len() == total_len`, `WriteStateBuffer` hashes the payload and sets
`finalized = true`. After that, the buffer cannot accept new bytes; exact
duplicate retries remain valid.

`CloseStateBuffer` requires the buffer authority to sign and always returns rent
to the payer recorded by the first write. It can close partial abandoned uploads,
finalized abandoned uploads, and buffers consumed by Direct settlement. If the
canonical `PendingCommitment` exists, cleanup is allowed only after that
commitment is finalized, expired, or cancelled. Repeating cleanup after the
buffer is already closed succeeds without moving lamports again.

Consuming instructions must check:

- buffer PDA seeds match `account_pubkey`, `commit_seq`, and `authority`;
- buffer is finalized;
- buffer commitment matches the `PendingCommitment`;
- authority matches the operator for `ProposeCommitment` and happy-path
  `FinalizeCommitment`;
- authority matches the challenger for the challenger buffer consumed by
  `ChallengerReveal`.

Future optimization: allow out-of-order or parallel chunk writes by adding a
small bitmap inside the `StateBuffer` account. Do this only if sequential writes
are too slow.

### Key Instruction Data

```rust
pub struct WriteStateBufferArgs<'a> {
    pub commit_seq: u32,
    pub total_len: u32,
    /// Must equal StateBuffer.payload.len(), unless retrying an exact old chunk.
    pub offset: u32,
    pub chunk: &'a [u8],
}

pub struct ProposeCommitmentArgs {
    pub commit_seq: u32,
    pub lamports: u64,
    pub owner: Pubkey,
    pub da_pointer_hash: [u8; 32],
    pub er_slot: Option<u64>,
}

pub struct RaiseChallengeArgs {
    pub state_commitment_hash: [u8; 32],
    pub challenge_hash: [u8; 32],
    pub stake_lamports: u64,
}

pub struct ChallengerRevealArgs {
    pub lamports: u64,
    pub owner: Pubkey,
    pub data_hash: [u8; 32],
    pub salt: [u8; 32],
}

pub enum DisputeDecision {
    OperatorStateCorrect,
    ChallengerStateCorrect,
}

pub struct ResolveDisputeArgs {
    pub decision: DisputeDecision,
}
```

### Important Instruction Rules

- A v2 `DelegationRecord` stores its `DelegationCluster` address as the
  delegation authority. Validator-authorized instructions load that exact
  cluster and require its `current_validator` signer. A caller cannot choose a
  different cluster or policy at runtime.
- `Direct` and `Challenge` are separate v2 finalization routes. The direct
  commit/finalize path requires `challenge_config == None`, while
  `ProposeCommitment` and `FinalizeCommitment` require
  `challenge_config == Some(...)`. Existing v1 delegations and their direct
  instructions retain their validator-key authorization model.
- `WriteStateBuffer` must run before `ProposeCommitment` for the operator state.
  `ProposeCommitment` rejects if the operator buffer is missing, unfinished, or
  belongs to another account, commit sequence number, or current validator.
- `ProposeCommitment` reads `data_hash` from the finalized operator buffer,
  verifies that the delegation, operator bond, and verifier registry belong to
  the same cluster, computes `account_state_hash` and
  `state_commitment_hash`, stores the cluster and posting validator in the
  pending record, copies `approval_threshold`, `resolver`,
  `min_challenger_stake`, `challenger_reveal_timeout_slots`, and
  `match_penalty_bps`, selects verifiers with round-robin, increments
  `VerifierRegistry.next_selection_index` by the number of scanned registry
  entries, and starts the challenge window.
- Verifier selection uses registered verifiers whose synchronized stake still
  satisfies the cluster's current minimum, excluding the commitment operator.
  If no eligible verifier remains, `ProposeCommitment` rejects.
- v2 MVP requires
  `DelegationCluster.challenge_config.verifiers_per_commitment == 1` and
  `DelegationCluster.challenge_config.approval_threshold == 1`.
  `ProposeCommitment` therefore selects exactly one verifier; if no verifier other
  than the operator exists, it rejects. Multi-verifier approval can be added
  later by relaxing config validation and extending approval lookup over the
  stored `SelectedVerifier` entries.
- `UpdateVerifierRegistry` mutates the registry. It affects future commitments
  only; it does not change `selected_verifiers` already stored on a pending
  commitment.
- `ApproveCommitment` requires the verifier to have an active bond, be the only
  selected verifier for this commitment, and still be inside the challenge
  window. Duplicate approvals do not increment `approval_count`.
- `RaiseChallenge` requires an active pending commitment, an open challenge
  window, no active challenge, and at least
  `PendingCommitment.min_challenger_stake`. It creates the `Challenge`, stores
  `challenge_hash`, derives the reveal deadline from
  `PendingCommitment.challenger_reveal_timeout_slots`, locks stake, sets
  `PendingCommitment.active_challenge`, and blocks normal finalization.
- `ChallengerReveal` validates the salted `challenge_hash` against the opened
  challenger state. If challenger state matches operator state, the challenge is
  terminal, the challenger pays `PendingCommitment.match_penalty_bps`, and the
  commitment can return to normal finalization. If challenger state differs,
  the commitment remains blocked until `ResolveDispute`.
- `ResolveDispute` requires the `PendingCommitment.resolver` signer copied when
  the commitment was posted. In DLP v2 this signer is expected to be a
  multisig-controlled account.
- `FinalizeCommitment` on the happy path requires closed window, approval
  threshold, no unresolved challenge, and full-state hash match.
- `FinalizeCommitment` after dispute resolution uses
  `PendingCommitment.resolved_state_source` to choose operator or challenger
  state.

### Failure Scenarios

| Scenario | DLP behavior |
| --- | --- |
| Delegation and supplied cluster differ | Reject. The delegation fixes the cluster for every v2 operation. |
| Rotated-out validator starts a new validator-authorized operation | Reject. Only `DelegationCluster.current_validator` is authorized. |
| Direct instruction receives a `Challenge` cluster | Reject. Challenge-enabled delegations cannot bypass the pending-commitment state machine. |
| Challenge instruction receives a `Direct` cluster | Reject. Direct clusters do not create challenge protocol state. |
| Bond or registry belongs to another cluster | Reject before using its stake or verifier entries. |
| Wrong registry account is passed | Reject. The registry account must match `PendingCommitment.verifier_registry`. |
| Registry update includes duplicate, unbonded, or inactive verifiers | Reject without changing the registry. |
| No eligible verifier exists at `ProposeCommitment` | Reject. At least one verifier other than the operator must be selectable. |
| Registry changes after `ProposeCommitment` | No effect on this commitment. Approvals use stored `selected_verifiers`. |
| Selected verifier is slashed or exits before approval | Reject that approval. Under-approval handling decides whether to extend or expire. |
| Revealed challenger state matches operator state | Penalize the challenger and allow normal finalization. |
| Revealed challenger state differs from operator state | Block normal finalization until resolver chooses the state source. |

## Flows

### Happy Path

1. The cluster's current validator writes and finalizes `StateBuffer`.
2. The current validator posts a commitment using that buffer and the
   delegation-bound `DelegationCluster`.
3. DLP selects verifiers with round-robin and starts the challenge window.
4. Selected verifier approves.
5. Window closes without challenge.
6. `FinalizeCommitment` applies the operator state.

### Challenge Paths

| Case | Flow |
| --- | --- |
| Matching state | Challenger calls `RaiseChallenge` with a salted hash, reveals matching state and salt, pays match penalty, and the commitment returns to normal finalization. |
| Mismatch | Challenger calls `RaiseChallenge` with a salted hash, reveals a different valid state and salt, resolver multisig decides, and `FinalizeCommitment` applies the winning state. |

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

- Current validator: resolve its `DelegationCluster`, write DLP state buffers,
  compute hashes, and post commitments.
- Rotation controller: invoke `RotateValidator` after the external handover
  process is ready. Temporary encryption keys and transaction forwarding are
  outside DLP.
- Verifier: watch selections, fetch DA, replay execution, approve or challenge.
- Challenger: detect divergence, raise a challenge with a salted hash, then
  reveal the challenged state and salt. Large revealed data may use a challenger
  state buffer.
- Resolver tooling: fetch DA, run deterministic replay, present opened states and
  replay result, prepare the multisig transaction.
- Cranker: call extension/expiry, finalization, payout, and close instructions.

## Design FAQ

**Why is there a VerifierRegistry but no OperatorRegistry?**

DLP selects verifiers from a cluster-scoped registered verifier list. For v2
MVP the selection is round-robin, so DLP needs an ordered list plus a cursor.
The operator is the `DelegationCluster.current_validator`, so DLP resolves it
from the delegation-bound cluster and checks that cluster's `OperatorBond` PDA.

**Can the same identity be both operator and verifier?**

Yes. A cluster's current validator can also register as a verifier, but the
roles and bonds remain separate and cluster-scoped.

**Can an operator verify its own commitment?**

No. `ProposeCommitment` must exclude the posting validator from
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
- How stable `cluster_id` values are allocated and authenticated before
  `InitDelegationCluster`.
