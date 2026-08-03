# MIMD-0025 Implementation Notes

Companion to `mimd-0025.md`. This file only captures low-level implementation
choices and message shapes. Protocol rationale stays in the MIMD.

## Contents

- [Decisions To Review](#decisions-to-review)
- [Permissioned vs Permissionless](#permissioned-vs-permissionless)
- [Hashes](#hashes)
- [Accounts](#accounts)
- [Instructions](#instructions)
  - [Key Instruction Data](#key-instruction-data)
  - [Important Instruction Rules](#important-instruction-rules)
  - [Failure Scenarios](#failure-scenarios)
- [Flows](#flows)
  - [Dispute Resolution](#dispute-resolution)
- [Validator Repo Responsibilities](#validator-repo-responsibilities)
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
| `ProtocolConfig` | `["mimd-protocol-config"]` | Global params, VRF config, resolver signer, protocol fee vault. |
| `OperatorBond` | `["mimd-operator-bond", operator]` | Slashable operator stake and lifecycle. |
| `VerifierBond` | `["mimd-verifier-bond", verifier]` | Slashable verifier stake. |
| `VerifierRegistry` | `["mimd-verifier-registry"]` | All registered verifiers. |
| `PendingCommitment` | `["mimd-pending-commitment", account, commit_id]` | Main commitment state machine. |
| `StateBuffer` | `["mimd-state-buffer", account, commit_id, role, authority]` | Chunked full account data opened for finalize/reveal. |
| `Challenge` | `["mimd-challenge", account, commit_id, challenger]` | One challenge against one pending commitment. |
| `PayoutTimelock` | `["mimd-payout-timelock", challenge]` | Delayed payout for correct challenger. |

### Essential Fields

```rust
pub enum ActorStatus {
    Active,
    Exiting,
    Slashed,
    Jailed,
}

pub struct ProtocolConfig {
    pub authority: Pubkey,
    pub paused: bool,
    pub vrf_program: Pubkey,
    pub vrf_oracle_queue: Pubkey,
    /// Multisig-controlled signer allowed to call ResolveDispute.
    pub resolver: Pubkey,
    pub protocol_fee_vault: Pubkey,
    pub min_operator_bond: u64,
    pub min_verifier_bond: u64,
    pub min_challenger_stake: u64,
    pub challenge_window_slots: u64,
    pub operator_response_timeout_slots: u64,
    pub challenger_reveal_timeout_slots: u64,
    pub payout_timelock_slots: u64,
    /// Number of verifiers randomly picked from VerifierRegistry.
    pub selected_verifier_count: u16,
    pub approval_threshold: u16,
    pub max_window_extensions: u16,
    pub match_penalty_bps: u16,
}

pub struct OperatorBond {
    pub operator_identity: Pubkey,
    pub stake_lamports: u64,
    /// Stake temporarily unavailable for withdrawal.
    pub locked_lamports: u64,
    pub status: ActorStatus,
    pub withdraw_requested_slot: Option<u64>,
}

pub struct VerifierBond {
    pub verifier_identity: Pubkey,
    pub stake_lamports: u64,
    pub status: ActorStatus,
    pub registered_slot: u64,
    pub withdraw_requested_slot: Option<u64>,
}

pub struct VerifierRegistry {
    /// Increments every time `entries` changes.
    /// A pending commitment stores this value before requesting VRF. The VRF
    /// callback must see the same value before it can select verifiers.
    pub registry_revision: u64,
    /// All registered verifiers DLP can select from.
    pub entries: Vec<VerifierRegistryEntry>,
}

pub struct VerifierRegistryEntry {
    pub verifier_identity: Pubkey,
    /// Bond account proving this verifier has active stake.
    pub verifier_bond: Pubkey,
    /// Keep as 1 for equal-weight selection.
    pub weight: u64,
}

pub enum PendingCommitmentStatus {
    AwaitingRandomness,
    Active,
    AwaitingOperatorResponse,
    AwaitingChallengerReveal,
    AwaitingChallengerRevealAfterOperatorTimeout,
    AwaitingDisputeResolution,
    ResolvedOperator,
    ResolvedChallenger,
    Finalized,
    Expired,
    Cancelled,
}

pub enum ResolvedStateSource {
    OperatorCommitment,
    ChallengerReveal,
}

pub struct PendingCommitment {
    pub status: PendingCommitmentStatus,
    pub operator_identity: Pubkey,
    pub operator_bond: Pubkey,
    pub account_pubkey: Pubkey,
    pub commit_id: u64,
    pub delegation_record: Pubkey,
    pub da_pointer_hash: [u8; 32],
    pub account_state_hash: [u8; 32],
    pub data_hash: [u8; 32],
    pub lamports: u64,
    pub owner: Pubkey,
    pub state_commitment_hash: [u8; 32],
    /// Registry account used when this commitment was posted.
    pub verifier_registry: Pubkey,
    /// Copy of `VerifierRegistry.registry_revision` at post time.
    /// If the registry changes before VRF activation, this commitment is
    /// cancelled and the operator must repost against the latest registry.
    pub verifier_registry_revision: u64,
    pub challenge_window_id: u64,
    pub posted_slot: u64,
    /// Set by the VRF callback.
    pub activation_slot: Option<u64>,
    pub challenge_window_end_slot: Option<u64>,
    /// Verifiers selected by VRF for this commitment.
    /// Later registry changes do not rewrite this list.
    pub selected_verifiers: Vec<Pubkey>,
    /// One bit per selected verifier.
    pub approval_bitmap: Vec<u8>,
    pub approval_count: u16,
    pub approval_threshold: u16,
    pub active_challenge: Option<Pubkey>,
    /// VRF request id returned when DLP asks for randomness.
    pub vrf_request_id: Option<[u8; 32]>,
    /// VRF output bytes used to select verifiers.
    pub vrf_randomness: Option<[u8; 32]>,
    pub resolved_state_source: Option<ResolvedStateSource>,
}

pub enum StateBufferRole {
    OperatorFinalize,
    OperatorChallengeResponse,
    ChallengerReveal,
}

pub struct StateBuffer {
    pub role: StateBufferRole,
    pub authority: Pubkey,
    pub account_pubkey: Pubkey,
    pub commit_id: u64,
    pub expected_data_hash: [u8; 32],
    pub total_len: u32,
    pub written_len: u32,
    pub finalized: bool,
    pub data: Vec<u8>,
}

pub enum ChallengeStatus {
    AwaitingOperatorResponse,
    AwaitingChallengerReveal,
    AwaitingChallengerRevealAfterOperatorTimeout,
    AwaitingDisputeResolution,
    Terminal,
}

pub struct OpenedState {
    pub lamports: u64,
    pub owner: Pubkey,
    pub data_hash: [u8; 32],
    pub account_state_hash: [u8; 32],
    /// Finalized buffer containing the full account data, when needed.
    pub state_buffer: Option<Pubkey>,
}

pub enum ChallengeOutcome {
    InvalidRevealChallengerSlashed,
    MatchingStateChallengerPenalized,
    OperatorCorrectChallengerSlashed,
    ChallengerCorrectOperatorSlashed,
    ChallengerRevealTimeout,
}

pub struct Challenge {
    pub status: ChallengeStatus,
    pub pending_commitment: Pubkey,
    pub challenger_identity: Pubkey,
    pub challenger_stake_lamports: u64,
    pub challenge_hash: [u8; 32],
    pub raised_slot: u64,
    pub operator_response_deadline_slot: u64,
    pub challenger_reveal_deadline_slot: Option<u64>,
    pub operator_state: Option<OpenedState>,
    pub challenger_state: Option<OpenedState>,
    pub outcome: Option<ChallengeOutcome>,
}

pub struct PayoutTimelock {
    pub challenge: Pubkey,
    pub beneficiary: Pubkey,
    pub amount_lamports: u64,
    pub unlock_slot: u64,
    pub claimed: bool,
}
```

## Instructions

Account lists include only protocol-relevant accounts. Add payer/system/rent
accounts where account creation or lamport movement requires them.

| Instruction | Expected invoker | Description |
| --- | --- | --- |
| `InitProtocolConfig`<ul><li>ix-data: <code>params</code></li><li>accounts: <strong>authority, config</strong></li></ul> | Protocol authority | Creates the global config account and stores bootstrap params such as VRF config, resolver, fees, thresholds, and timeouts. |
| `UpdateProtocolConfig`<ul><li>ix-data: <code>params</code></li><li>accounts: <strong>authority, config</strong></li></ul> | Protocol authority | Updates params used by future commitments. Existing pending commitments keep the values copied into their accounts. |
| `RegisterOperator`<ul><li>ix-data: <code>amount_lamports</code></li><li>accounts: <strong>operator signer, protocol authority, operator bond, config</strong></li></ul> | Operator, protocol authority | Creates an operator bond and deposits slashable stake. Permissioned v2 requires configured approval before the operator can post commitments. |
| `RegisterVerifier`<ul><li>ix-data: <code>amount_lamports</code></li><li>accounts: <strong>verifier signer, protocol authority, verifier bond, config</strong></li></ul> | Verifier, protocol authority | Creates a verifier bond and deposits slashable stake. Permissioned v2 requires configured approval before the verifier can enter the registry. |
| `RequestStakeWithdrawal`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, bond, config</strong></li></ul> | Operator or verifier | Marks bonded stake as exiting. The stake cannot be withdrawn until the configured delay passes and no locks remain. |
| `WithdrawStake`<ul><li>ix-data: <code>actor_kind</code></li><li>accounts: <strong>actor signer, bond, config</strong></li></ul> | Operator or verifier | Withdraws unlocked stake after the exit delay. Slashed or locked stake stays in the protocol. |
| `UpdateVerifierRegistry`<ul><li>ix-data: <code>update</code></li><li>accounts: <strong>authority, verifier registry, verifier bonds</strong></li></ul> | Protocol authority | Adds or removes registered verifiers and increments `registry_revision`. Invalid, duplicate, unbonded, or inactive verifiers are rejected. |
| `PostCommitment`<ul><li>ix-data: <code>commitment</code></li><li>accounts: <strong>operator, operator bond, pending commitment, delegated account, delegation record, config, verifier registry, DLP identity PDA, VRF queue/program</strong></li></ul> | Operator | Creates an `AwaitingRandomness` commitment, stores the current `registry_revision`, locks any commitment-local stake if needed, and requests VRF. |
| `ConsumeCommitmentRandomness`<ul><li>ix-data: <code>randomness</code></li><li>accounts: <strong>VRF identity signer, pending commitment, config, verifier registry</strong></li></ul> | VRF callback | Verifies the VRF caller and registry revision, selects verifiers from the registry, and starts the challenge window. |
| `CancelUnactivatedCommitment`<ul><li>ix-data: <code>reason</code></li><li>accounts: <strong>cranker/operator, pending commitment, config, verifier registry</strong></li></ul> | Operator or cranker | Cancels a commitment that is still waiting for randomness but can no longer activate, such as after registry change or VRF timeout. |
| `ApproveCommitment`<ul><li>ix-data: <code>selected_verifier_index</code></li><li>accounts: <strong>verifier signer, verifier bond, pending commitment</strong></li></ul> | Selected verifier | Records approval from one selected verifier. Duplicate approvals do not increase the count. |
| `WriteStateBuffer`<ul><li>ix-data: <code>chunk</code></li><li>accounts: <strong>authority signer, state buffer, pending commitment</strong></li></ul> | Buffer authority | Writes a chunk of opened account data for finalize, operator response, or challenger reveal. |
| `FinalizeStateBuffer`<ul><li>ix-data: <code>role</code></li><li>accounts: <strong>authority signer, state buffer, pending commitment</strong></li></ul> | Buffer authority | Freezes a completed buffer after length and hash checks. Frozen buffers can be used by later instructions. |
| `RaiseChallenge`<ul><li>ix-data: <code>challenge</code></li><li>accounts: <strong>challenger signer, challenge, pending commitment, config</strong></li></ul> | Challenger | Locks challenger stake, records the hidden challenge hash, and blocks normal finalization until the challenge is resolved. |
| `OperatorChallengeResponse`<ul><li>ix-data: <code>state</code></li><li>accounts: <strong>operator signer, pending commitment, challenge, optional state buffer</strong></li></ul> | Operator | Opens the operator's claimed state for the challenged commitment and starts the challenger reveal timeout. |
| `MarkOperatorTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, pending commitment, challenge</strong></li></ul> | Cranker | Records that the operator missed the response deadline. The challenger must still reveal the challenge preimage. |
| `ChallengerReveal`<ul><li>ix-data: <code>state, salt</code></li><li>accounts: <strong>challenger signer, pending commitment, challenge, optional buffer, fee vault</strong></li></ul> | Challenger | Verifies the challenge preimage and opened state. It slashes invalid reveals, penalizes matching reveals, or moves mismatches to resolver decision. |
| `MarkChallengerRevealTimeout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, pending commitment, challenge, fee vault</strong></li></ul> | Cranker | Slashes challenger stake when the reveal deadline passes without a valid reveal. |
| `ResolveDispute`<ul><li>ix-data: <code>decision</code></li><li>accounts: <strong>resolver signer, challenge, pending commitment, operator bond, fee vault, optional payout timelock</strong></li></ul> | Resolver multisig | Applies the multisig decision for a valid mismatch: operator correct or challenger correct. |
| `FinalizeCommitment`<ul><li>ix-data: <code>state_source</code></li><li>accounts: <strong>finalizer, pending commitment, delegated account, delegation record/metadata, state buffer, optional challenge, config</strong></li></ul> | Finalizer or cranker | Applies the final state after the happy path or after dispute resolution. |
| `ExtendChallengeWindow`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>cranker, pending commitment, config</strong></li></ul> | Cranker | Extends an under-approved commitment according to config, or expires it after the maximum extensions. |
| `ClaimPayout`<ul><li>ix-data: <code>empty</code></li><li>accounts: <strong>beneficiary signer, payout timelock</strong></li></ul> | Beneficiary | Pays the challenger reward after the timelock for a challenger-correct dispute. |
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
- `ConsumeCommitmentRandomness` verifies the VRF identity signer, reads the
  `VerifierRegistry`, and requires its `registry_revision` to match the value
  stored on the pending commitment. If it matches, DLP derives
  `selected_verifiers` from randomness and starts the challenge window.
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
