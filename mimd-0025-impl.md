# MIMD-0025 Implementation Design

## Status

Draft implementation design for discussion.

This document expands `mimd-0025.md` into concrete program accounts,
instructions, instruction data, account lists, off-chain responsibilities, and
protocol flows. It intentionally describes the target design instead of trying
to preserve compatibility with the current Delegation Program commit path.

Configurable values such as stake amounts, timeouts, thresholds, and maximum
sizes are named as parameters. They should be finalized before implementation,
but their exact values do not change the protocol flow described here.

## Design Goals

- A delegated account state can be finalized only through a pending commitment
  that survives its challenge window.
- Happy-path finalization requires approval from randomly selected bonded
  verifiers.
- Any sufficiently staked challenger can block a pending commitment during the
  active challenge window.
- A challenge reveal proves either invalid challenge material, unnecessary
  disagreement, or a real state mismatch.
- A state mismatch is resolved by an on-chain security council in the first
  implementation.
- Program state is shaped for permissionless operators, verifiers, and
  challengers even if a deployment starts with controlled membership.
- The validator repo owns off-chain execution, replay, monitoring, and cranking.
  The Delegation Program remains the source of truth for state transitions.

## Components

### Delegation Program

The Delegation Program stores protocol configuration, bonded actors, verifier
snapshots, pending commitments, challenges, council votes, stake custody, and
payout timelocks. It is the only component that can finalize delegated account
state under this protocol.

### Ephemeral VRF Program

The VRF program provides unpredictable and on-chain-verifiable randomness.

The Delegation Program requests randomness after a pending commitment is posted.
The VRF program later invokes a Delegation Program callback with a random
32-byte output. The callback activates the pending commitment, derives the
selected verifier committee from a verifier-set snapshot, and starts the
challenge window.

### Operator

The operator runs the Ephemeral Rollup session and posts pending state
commitments for delegated accounts. In practice this is part of
`magicblock-validator` or a committor service.

### Verifier

A verifier is a bonded participant eligible for random selection. A selected
verifier independently checks the DA record and replay result before approving
the commitment on-chain.

### Challenger

A challenger observes operator output and replica execution. A challenger locks
stake and submits a salted challenge hash when it believes a pending commitment
is wrong.

### Security Council

The security council is a stake-weighted on-chain resolver used only for the
first mismatch-resolution implementation. It is not the happy-path verifier
committee. Council members vote on which opened state is correct: operator or
challenger.

An off-chain council service may help members inspect disputes and submit votes,
but it must not be the source of truth. Membership, weights, vote records,
quorum, and final resolution are on-chain.

## Data Model

### Scalar Types

```text
Hash32      = [u8; 32]
Pubkey      = [u8; 32]
Slot        = u64
UnixTime    = i64
CommitId    = u64
Lamports    = u64
BasisPoints = u16
```

All integer fields are little-endian in serialized instruction data and account
data. All hashes are exactly 32 bytes.

### Canonical Account State

The canonical state for a delegatable account is:

```text
AccountStateV1 {
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
}
```

`executable` is excluded because executable accounts are not delegatable in this
design.

Full account data is not stored in `PendingCommitment`. Full data is opened only
when finalizing or resolving a challenge, usually through one or more
`StateBuffer` accounts.

### State Hash

```text
account_state_hash = H(
  "magicblock.account_state.v1",
  lamports,
  owner,
  data_hash
)
```

`data_hash` is the hash of the full canonical account data bytes:

```text
data_hash = H("magicblock.account_data.v1", account_data)
```

If missing-account finalization is later supported, its representation must be a
separate explicit variant and must not collide with zero-lamport empty data.

### DA Pointer

The protocol stores a hash of the Data Availability pointer, not a variable
length pointer in the fixed commitment account:

```text
da_pointer_hash = H("magicblock.da_pointer.v1", da_pointer_bytes)
```

The raw DA pointer format is a protocol parameter. It must identify the data
needed by verifiers to replay or verify the ER execution that produced the
committed state.

### Commitment Hash

```text
state_commitment_hash = H(
  "magicblock.state_commitment.v1",
  operator_identity,
  account_pubkey,
  commit_id,
  delegation_record,
  da_pointer_hash,
  account_state_hash,
  verifier_snapshot,
  verifier_snapshot_hash,
  challenge_window_id
)
```

The commitment key is `(account_pubkey, commit_id)`.

The ER slot that produced the commitment can be stored as observability metadata
but is not part of the protocol key.

### Challenge Hash

```text
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

The challenger initially submits only `challenge_hash`. The challenger later
reveals `lamports`, `owner`, `data_hash`, full data if required, and `salt`.

### Approval Context

Verifier approvals are normal Solana signer transactions in v1. A verifier does
not submit an off-chain signature blob. Instead, a selected verifier calls
`ApproveCommitment` as the transaction signer.

The approved payload is implicitly:

```text
approval_context = H(
  "magicblock.verifier_approval.v1",
  state_commitment_hash,
  challenge_window_id,
  verifier_identity,
  verifier_snapshot,
  verifier_index
)
```

The program records the approval by selected verifier index. Duplicate approvals
do not increase the approval count.

## Configurable Parameters

These values live in `ProtocolConfig`.

- minimum operator bond
- minimum verifier bond
- minimum challenger stake
- challenge window duration
- approval threshold
- maximum selected verifiers
- maximum verifier snapshot size
- maximum challenge-window extensions
- threshold adjustment schedule
- operator response timeout
- challenger reveal timeout
- council quorum
- council voting timeout
- no-quorum resolution policy
- council supermajority basis points
- valid-match challenger penalty basis points
- correct-challenger payout timelock
- stake withdrawal delay
- maximum state buffer chunk size
- VRF oracle queue
- VRF program id

The initial values are deployment parameters. The flow does not depend on the
specific numeric choices.

## Accounts

Account seeds are written as conceptual PDA seeds. Final seed byte strings should
be frozen before implementation.

### `ProtocolConfig`

Global configuration and upgrade authority.

```text
seeds = ["mimd-protocol-config"]
```

Fields:

```text
ProtocolConfig {
  version: u16,
  authority: Pubkey,
  paused: bool,
  vrf_program: Pubkey,
  vrf_oracle_queue: Pubkey,
  council_config: Pubkey,
  protocol_fee_vault: Pubkey,
  min_operator_bond: u64,
  min_verifier_bond: u64,
  min_challenger_stake: u64,
  challenge_window_slots: u64,
  operator_response_timeout_slots: u64,
  challenger_reveal_timeout_slots: u64,
  council_voting_timeout_slots: u64,
  payout_timelock_slots: u64,
  stake_withdrawal_delay_slots: u64,
  selected_verifier_count: u16,
  approval_threshold: u16,
  max_verifier_snapshot_size: u32,
  max_window_extensions: u16,
  match_penalty_bps: u16,
  council_quorum_bps: u16,
  council_supermajority_bps: u16,
}
```

Notes:

- `paused` blocks new commitments and challenges, but should not block terminal
  challenge resolution or payout claims.
- Any authority-controlled mutation must be explicit and logged through account
  state changes.

### `OperatorBond`

Slashable operator stake and lifecycle state.

```text
seeds = ["mimd-operator-bond", operator_identity]
```

Fields:

```text
OperatorBond {
  operator_identity: Pubkey,
  stake_lamports: u64,
  locked_lamports: u64,
  status: OperatorStatus,
  withdraw_requested_slot: Option<u64>,
  active_commitment_count: u32,
}

OperatorStatus = Active | Exiting | Slashed | Jailed
```

Notes:

- `stake_lamports` is slashable stake and must not be confused with fee vaults.
- `locked_lamports` tracks stake temporarily locked by unresolved commitments or
  disputes if the final economic design requires per-commit locks.

### `VerifierBond`

Verifier stake and eligibility state.

```text
seeds = ["mimd-verifier-bond", verifier_identity]
```

Fields:

```text
VerifierBond {
  verifier_identity: Pubkey,
  stake_lamports: u64,
  status: VerifierStatus,
  registered_slot: u64,
  withdraw_requested_slot: Option<u64>,
}

VerifierStatus = Active | Exiting | Slashed | Jailed
```

### `VerifierSetSnapshot`

A bounded snapshot of active verifiers used for deterministic selection.

```text
seeds = ["mimd-verifier-snapshot", snapshot_id]
```

Fields:

```text
VerifierSetSnapshot {
  snapshot_id: u64,
  created_slot: u64,
  verifier_count: u32,
  snapshot_hash: Hash32,
  entries: Vec<VerifierSnapshotEntry>,
}

VerifierSnapshotEntry {
  verifier_identity: Pubkey,
  verifier_bond: Pubkey,
  weight: u64,
}
```

Notes:

- For v1 this is a bounded on-chain vector.
- A later version can replace `entries` with a Merkle root plus membership
  proofs. The commitment hash already binds `snapshot_hash`, so this migration
  must be versioned.
- Snapshot creation must exclude inactive, exiting, jailed, or underbonded
  verifiers.

### `PendingCommitment`

The central state machine for one account at one commit id.

```text
seeds = ["mimd-pending-commitment", account_pubkey, commit_id]
```

Fields:

```text
PendingCommitment {
  version: u16,
  status: PendingCommitmentStatus,
  operator_identity: Pubkey,
  operator_bond: Pubkey,
  account_pubkey: Pubkey,
  commit_id: u64,
  delegation_record: Pubkey,
  da_pointer_hash: Hash32,
  account_state_hash: Hash32,
  data_hash: Hash32,
  lamports: u64,
  owner: Pubkey,
  state_commitment_hash: Hash32,
  verifier_snapshot: Pubkey,
  verifier_snapshot_id: u64,
  verifier_snapshot_hash: Hash32,
  challenge_window_id: u64,
  posted_slot: u64,
  activation_slot: Option<u64>,
  challenge_window_start_slot: Option<u64>,
  challenge_window_end_slot: Option<u64>,
  window_extension_count: u16,
  selected_verifier_count: u16,
  approval_threshold: u16,
  selected_verifier_indices: Vec<u32>,
  approval_bitmap: Vec<u8>,
  approval_count: u16,
  active_challenge: Option<Pubkey>,
  resolved_state_source: Option<ResolvedStateSource>,
  vrf_request_id: Option<Hash32>,
  vrf_randomness: Option<Hash32>,
  er_slot: Option<u64>,
}

PendingCommitmentStatus =
  AwaitingRandomness |
  Active |
  Challenged |
  AwaitingOperatorResponse |
  AwaitingChallengerReveal |
  AwaitingChallengerRevealAfterOperatorTimeout |
  AwaitingCouncil |
  ResolvedOperator |
  ResolvedChallenger |
  Finalized |
  Expired |
  Cancelled

ResolvedStateSource = OperatorCommitment | ChallengerReveal
```

Notes:

- `selected_verifier_indices` are indices into `VerifierSetSnapshot.entries`.
- `approval_bitmap` has one bit per selected verifier index, not one bit per
  snapshot entry.
- v1 supports one active challenge per pending commitment. Later versions can
  add a challenge list if multi-challenge handling is required.

### `StateBuffer`

Chunked full-state data opened for finalization, operator response, or challenger
reveal.

```text
seeds = [
  "mimd-state-buffer",
  account_pubkey,
  commit_id,
  buffer_role,
  authority
]
```

Fields:

```text
StateBuffer {
  role: StateBufferRole,
  authority: Pubkey,
  account_pubkey: Pubkey,
  commit_id: u64,
  expected_data_hash: Hash32,
  total_len: u32,
  written_len: u32,
  finalized: bool,
  chunks_hash: Hash32,
  data: Vec<u8>,
}

StateBufferRole =
  OperatorFinalize |
  OperatorChallengeResponse |
  ChallengerReveal
```

Notes:

- A buffer can be appended in chunks until `written_len == total_len`.
- `FinalizeStateBuffer` freezes the buffer and verifies `data_hash`.
- The buffer account should be closable after the commitment or challenge is
  terminal.

### `Challenge`

Challenge state for one pending commitment.

```text
seeds = ["mimd-challenge", account_pubkey, commit_id, challenger_identity]
```

Fields:

```text
Challenge {
  status: ChallengeStatus,
  pending_commitment: Pubkey,
  operator_identity: Pubkey,
  challenger_identity: Pubkey,
  challenger_stake_lamports: u64,
  account_pubkey: Pubkey,
  commit_id: u64,
  state_commitment_hash: Hash32,
  challenge_hash: Hash32,
  raised_slot: u64,
  operator_response_deadline_slot: u64,
  challenger_reveal_deadline_slot: Option<u64>,
  operator_state_hash: Option<Hash32>,
  operator_data_hash: Option<Hash32>,
  operator_lamports: Option<u64>,
  operator_owner: Option<Pubkey>,
  operator_state_buffer: Option<Pubkey>,
  challenger_state_hash: Option<Hash32>,
  challenger_data_hash: Option<Hash32>,
  challenger_lamports: Option<u64>,
  challenger_owner: Option<Pubkey>,
  challenger_state_buffer: Option<Pubkey>,
  salt_hash: Option<Hash32>,
  council_case: Option<Pubkey>,
  terminal_outcome: Option<ChallengeOutcome>,
}

ChallengeStatus =
  AwaitingOperatorResponse |
  AwaitingChallengerReveal |
  AwaitingChallengerRevealAfterOperatorTimeout |
  AwaitingCouncil |
  Terminal

ChallengeOutcome =
  InvalidRevealChallengerSlashed |
  MatchingStateChallengerPenalized |
  OperatorCorrectChallengerSlashed |
  ChallengerCorrectOperatorSlashed |
  OperatorTimeout |
  NoQuorum
```

Notes:

- The actual salt is not stored unless needed for audit. Storing `salt_hash` is
  enough after reveal.
- Operator timeout does not directly finalize challenger state. It moves to the
  configured resolution path with non-response as evidence.

### `CouncilConfig`

Council membership and voting parameters.

```text
seeds = ["mimd-council-config"]
```

Fields:

```text
CouncilConfig {
  authority: Pubkey,
  epoch: u64,
  quorum_bps: u16,
  supermajority_bps: u16,
  voting_timeout_slots: u64,
  members: Vec<CouncilMember>,
}

CouncilMember {
  identity: Pubkey,
  weight: u64,
  active: bool,
}
```

Notes:

- Council membership changes should not affect already-open council cases.
- A council case stores the council epoch and member weights used for that case.

### `CouncilCase`

Council vote state for one challenged commitment.

```text
seeds = ["mimd-council-case", challenge]
```

Fields:

```text
CouncilCase {
  challenge: Pubkey,
  pending_commitment: Pubkey,
  council_epoch: u64,
  opened_slot: u64,
  voting_deadline_slot: u64,
  total_weight: u64,
  quorum_weight: u64,
  supermajority_weight: u64,
  operator_votes: u64,
  challenger_votes: u64,
  abstain_votes: u64,
  vote_bitmap: Vec<u8>,
  member_identities: Vec<Pubkey>,
  member_weights: Vec<u64>,
  status: CouncilCaseStatus,
  outcome: Option<CouncilOutcome>,
}

CouncilCaseStatus = Open | Terminal
CouncilOutcome = OperatorCorrect | ChallengerCorrect | NoQuorum
```

### `PayoutTimelock`

Delayed payout for a correct challenger.

```text
seeds = ["mimd-payout-timelock", challenge]
```

Fields:

```text
PayoutTimelock {
  challenge: Pubkey,
  beneficiary: Pubkey,
  amount_lamports: u64,
  unlock_slot: u64,
  claimed: bool,
}
```

## Instructions

### `InitializeProtocolConfig`

Creates `ProtocolConfig`.

Instruction data:

```text
InitializeProtocolConfigArgs {
  authority: Pubkey,
  vrf_program: Pubkey,
  vrf_oracle_queue: Pubkey,
  council_config: Pubkey,
  parameters: ProtocolParameters,
}
```

Accounts:

```text
0. [signer, writable] payer
1. [writable] protocol_config
2. [] system_program
```

Effects:

- Creates and initializes global protocol configuration.

### `UpdateProtocolConfig`

Updates configurable parameters.

Instruction data:

```text
UpdateProtocolConfigArgs {
  fields: ProtocolConfigUpdate,
}
```

Accounts:

```text
0. [signer] authority
1. [writable] protocol_config
```

Effects:

- Applies allowed parameter updates.
- Must not mutate already-open commitment or council case parameters.

### `RegisterOperator`

Creates or tops up an operator bond.

Instruction data:

```text
RegisterOperatorArgs {
  amount_lamports: u64,
}
```

Accounts:

```text
0. [signer, writable] operator_identity
1. [writable] operator_bond
2. [] protocol_config
3. [] system_program
```

Effects:

- Transfers stake into `OperatorBond`.
- Marks the operator active if stake is at least `min_operator_bond`.

### `RegisterVerifier`

Creates or tops up a verifier bond.

Instruction data:

```text
RegisterVerifierArgs {
  amount_lamports: u64,
}
```

Accounts:

```text
0. [signer, writable] verifier_identity
1. [writable] verifier_bond
2. [] protocol_config
3. [] system_program
```

Effects:

- Transfers stake into `VerifierBond`.
- Marks the verifier active if stake is at least `min_verifier_bond`.

### `RequestStakeWithdrawal`

Starts delayed withdrawal for an operator or verifier.

Instruction data:

```text
RequestStakeWithdrawalArgs {
  actor_kind: ActorKind,
}

ActorKind = Operator | Verifier
```

Accounts:

```text
0. [signer] actor_identity
1. [writable] bond_account
2. [] protocol_config
```

Effects:

- Sets status to `Exiting`.
- Sets `withdraw_requested_slot`.
- Exiting actors are excluded from future verifier snapshots and new
  commitments.

### `WithdrawStake`

Completes a delayed withdrawal.

Instruction data:

```text
WithdrawStakeArgs {
  actor_kind: ActorKind,
}
```

Accounts:

```text
0. [signer, writable] actor_identity
1. [writable] bond_account
2. [] protocol_config
```

Effects:

- Transfers unlocked stake back to the actor after the withdrawal delay.
- Fails if the actor is slashed, jailed, under active dispute, or still locked.

### `CreateVerifierSetSnapshot`

Creates a bounded verifier snapshot.

Instruction data:

```text
CreateVerifierSetSnapshotArgs {
  snapshot_id: u64,
}
```

Accounts:

```text
0. [signer, writable] payer_or_cranker
1. [] protocol_config
2. [writable] verifier_snapshot
3..N. [] verifier_bond_accounts
N+1. [] system_program
```

Effects:

- Reads active verifier bonds.
- Stores snapshot entries and `snapshot_hash`.
- Fails if the snapshot exceeds configured maximum size.

Open design point:

- For fully permissionless large sets, this instruction will need a Merkleized
  or paged snapshot design. The bounded vector design is the v1 baseline.

### `PostCommitment`

Posts a pending commitment and requests VRF randomness.

Instruction data:

```text
PostCommitmentArgs {
  commit_id: u64,
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
  da_pointer_hash: Hash32,
  er_slot: Option<u64>,
}
```

Accounts:

```text
0. [signer, writable] operator_identity
1. [writable] operator_bond
2. [writable] pending_commitment
3. [] delegated_account
4. [] delegation_record
5. [] protocol_config
6. [] verifier_snapshot
7. [] dlp_program_identity_pda
8. [writable] vrf_oracle_queue
9. [] vrf_program
10. [] slot_hashes_sysvar
11. [] system_program
```

Effects:

- Verifies the operator is active and sufficiently bonded.
- Verifies the commit id is the next valid commit id for the account.
- Computes `account_state_hash`.
- Computes `state_commitment_hash`.
- Creates `PendingCommitment` in `AwaitingRandomness`.
- Requests VRF randomness using a caller seed bound to:

```text
H(
  "magicblock.commitment_vrf_request.v1",
  pending_commitment,
  operator_identity,
  account_pubkey,
  commit_id,
  state_commitment_hash,
  verifier_snapshot,
  verifier_snapshot_hash
)
```

Notes:

- The challenge window does not start until `ConsumeCommitmentRandomness`.
- The operator cannot select verifier identities. It only supplies the snapshot.
  The program verifies the snapshot account and derives selected indices from
  the VRF output.
- `dlp_program_identity_pda` is not an outer transaction signer. The Delegation
  Program signs for it with `invoke_signed` when making the inner VRF request.

### `ConsumeCommitmentRandomness`

VRF callback that activates a pending commitment.

Instruction data:

The VRF program prepends the random 32-byte output to callback args.

```text
ConsumeCommitmentRandomnessArgs {
  randomness: Hash32,
  pending_commitment: Pubkey,
}
```

Accounts:

```text
0. [signer] vrf_program_identity
1. [writable] pending_commitment
2. [] protocol_config
3. [] verifier_snapshot
```

Effects:

- Verifies `vrf_program_identity` is the expected VRF identity signer.
- Verifies pending commitment status is `AwaitingRandomness`.
- Derives `selected_verifier_indices` from `randomness` and snapshot entries.
- Initializes the approval bitmap.
- Sets `activation_slot`, `challenge_window_start_slot`, and
  `challenge_window_end_slot`.
- Sets status to `Active`.

Selection algorithm:

```text
seed = H(
  "magicblock.verifier_selection.v1",
  randomness,
  state_commitment_hash,
  verifier_snapshot_hash,
  selected_verifier_count
)
```

Then repeatedly derive candidate indices from `H(seed, counter)` until enough
unique verifier indices are selected. Weighted selection can be added by
interpreting snapshot weights as ranges; uniform selection is simpler for v1 if
all active verifiers have equal weight.

### `ApproveCommitment`

Records approval from one selected verifier.

Instruction data:

```text
ApproveCommitmentArgs {
  verifier_snapshot_index: u32,
}
```

Accounts:

```text
0. [signer] verifier_identity
1. [] verifier_bond
2. [writable] pending_commitment
3. [] verifier_snapshot
```

Effects:

- Verifies pending commitment status is `Active`.
- Verifies current slot is within the active challenge window.
- Verifies the verifier bond is active.
- Verifies `verifier_snapshot_index` points to `verifier_identity`.
- Verifies the index is in `selected_verifier_indices`.
- Sets the corresponding approval bit if not already set.
- Increments `approval_count` only for the first approval by that verifier.

Notes:

- v1 uses signer transactions instead of Ed25519 or secp signature blobs.
- A selected verifier approval means "I verified this commitment and do not
  challenge it."

### `RaiseChallenge`

Raises a permissionless challenge against an active pending commitment.

Instruction data:

```text
RaiseChallengeArgs {
  challenge_hash: Hash32,
  stake_lamports: u64,
}
```

Accounts:

```text
0. [signer, writable] challenger_identity
1. [writable] challenge
2. [writable] pending_commitment
3. [] protocol_config
4. [] system_program
```

Effects:

- Verifies pending commitment status is `Active`.
- Verifies current slot is within the challenge window.
- Verifies no active challenge already exists for this pending commitment.
- Transfers `stake_lamports` from challenger into `Challenge`.
- Creates `Challenge` in `AwaitingOperatorResponse`.
- Sets pending commitment status to `Challenged`.
- Sets pending commitment `active_challenge`.

### `WriteStateBuffer`

Writes a chunk into a state buffer.

Instruction data:

```text
WriteStateBufferArgs {
  role: StateBufferRole,
  offset: u32,
  total_len: u32,
  expected_data_hash: Hash32,
  chunk: Vec<u8>,
}
```

Accounts:

```text
0. [signer, writable] authority
1. [writable] state_buffer
2. [] pending_commitment
3. [] system_program
```

Effects:

- Creates or appends to a buffer for the authority, account, commit id, and role.
- Verifies chunks are written at the expected offsets.
- Updates `written_len`.

### `FinalizeStateBuffer`

Freezes a completed state buffer.

Instruction data:

```text
FinalizeStateBufferArgs {
  role: StateBufferRole,
}
```

Accounts:

```text
0. [signer] authority
1. [writable] state_buffer
2. [] pending_commitment
```

Effects:

- Verifies `written_len == total_len`.
- Recomputes `data_hash`.
- Verifies it equals `expected_data_hash`.
- Sets `finalized = true`.

### `OperatorChallengeResponse`

Operator opens or confirms the state it committed to.

Instruction data:

```text
OperatorChallengeResponseArgs {
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
  state_buffer: Option<Pubkey>,
}
```

Accounts:

```text
0. [signer] operator_identity
1. [writable] pending_commitment
2. [writable] challenge
3. [] state_buffer_optional
```

Effects:

- Verifies challenge status is `AwaitingOperatorResponse`.
- Verifies current slot is before `operator_response_deadline_slot`.
- Verifies operator identity matches the pending commitment.
- Recomputes operator account state hash.
- Verifies it equals the pending commitment account state hash.
- Stores operator opened state metadata on `Challenge`.
- Sets challenge status to `AwaitingChallengerReveal`.
- Sets `challenger_reveal_deadline_slot`.

Notes:

- If the pending commitment already contains sufficient full-state evidence for
  the response path, `state_buffer` may be omitted. The default design expects
  full data to be available through a finalized `StateBuffer` when needed.

### `MarkOperatorTimeout`

Records operator non-response and moves the challenge to the reveal-before-
resolution path.

Instruction data:

```text
MarkOperatorTimeoutArgs {}
```

Accounts:

```text
0. [signer] cranker
1. [writable] pending_commitment
2. [writable] challenge
```

Effects:

- Verifies challenge status is `AwaitingOperatorResponse`.
- Verifies current slot is after `operator_response_deadline_slot`.
- Records operator non-response as evidence of operator fault.
- Sets challenge and pending commitment status to
  `AwaitingChallengerRevealAfterOperatorTimeout`.
- Sets `challenger_reveal_deadline_slot` if not already set.

Notes:

- The challenger must still reveal the committed state and salt before any state
  can be finalized in its favor. This prevents a challenger from forcing
  operator slashing or state finalization with an unopened challenge hash.
- After a valid reveal in this state, the challenge enters council resolution
  with operator non-response as additional evidence.

### `ChallengerReveal`

Reveals challenger state and salt.

Instruction data:

```text
ChallengerRevealArgs {
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
  salt: [u8; 32],
  state_buffer: Option<Pubkey>,
}
```

Accounts:

```text
0. [signer] challenger_identity
1. [writable] pending_commitment
2. [writable] challenge
3. [] challenger_state_buffer_optional
4. [writable] protocol_fee_vault
5. [writable] council_case_optional
6. [] council_config_optional
7. [] system_program_optional
```

Effects:

- Verifies challenge status is `AwaitingChallengerReveal` or
  `AwaitingChallengerRevealAfterOperatorTimeout`.
- Verifies current slot is before `challenger_reveal_deadline_slot`.
- Recomputes `challenge_hash`.
- If the hash does not match:
  - fully slashes challenger stake;
  - sends slashed stake to protocol fee vault or configured destination;
  - sets terminal outcome `InvalidRevealChallengerSlashed`;
  - clears pending commitment `active_challenge`;
  - returns pending commitment to `Active` if the challenge window remains valid,
    otherwise leaves it ready for finalization checks.
- If the hash matches and challenger state equals operator state:
  - charges `match_penalty_bps` of challenger stake to protocol;
  - unlocks the remaining challenger stake;
  - sets terminal outcome `MatchingStateChallengerPenalized`;
  - clears pending commitment `active_challenge`;
  - returns pending commitment to normal finalization path.
- If the hash matches, the operator responded, and states differ:
  - stores challenger opened state metadata;
  - opens a council case;
  - sets challenge and pending commitment status to `AwaitingCouncil`.
- If the hash matches and the operator timed out:
  - stores challenger opened state metadata;
  - opens a council case;
  - records operator non-response as additional evidence;
  - sets challenge and pending commitment status to `AwaitingCouncil`.

### `MarkChallengerRevealTimeout`

Penalizes a challenger that does not reveal.

Instruction data:

```text
MarkChallengerRevealTimeoutArgs {}
```

Accounts:

```text
0. [signer] cranker
1. [writable] pending_commitment
2. [writable] challenge
3. [writable] protocol_fee_vault
```

Effects:

- Verifies current slot is after `challenger_reveal_deadline_slot`.
- Fully slashes challenger stake.
- Sets terminal outcome `InvalidRevealChallengerSlashed`.
- Clears pending commitment challenge state.
- If the operator had responded, returns the pending commitment to the normal
  finalization path.
- If the operator had timed out, marks the pending commitment expired or
  cancelled according to protocol policy because neither opened state is
  available as a valid finalization source.

### `CouncilVote`

Records one council member vote.

Instruction data:

```text
CouncilVoteArgs {
  member_index: u32,
  vote: CouncilVoteChoice,
}

CouncilVoteChoice = Operator | Challenger | Abstain
```

Accounts:

```text
0. [signer] council_member
1. [writable] council_case
2. [] challenge
```

Effects:

- Verifies council case is open.
- Verifies current slot is before `voting_deadline_slot`.
- Verifies `member_index` points to `council_member`.
- Verifies member has not already voted.
- Adds member weight to the selected vote bucket.

### `ResolveCouncilCase`

Finalizes a council case when quorum and supermajority conditions are met, or
when the vote timeout has elapsed.

Instruction data:

```text
ResolveCouncilCaseArgs {}
```

Accounts:

```text
0. [signer] cranker
1. [writable] council_case
2. [writable] challenge
3. [writable] pending_commitment
4. [writable] operator_bond
5. [writable] protocol_fee_vault
6. [writable] payout_timelock_optional
7. [] system_program_optional
```

Effects:

- If operator side wins:
  - fully slashes challenger stake;
  - sets challenge outcome `OperatorCorrectChallengerSlashed`;
  - sets pending commitment status `ResolvedOperator`;
  - sets resolved state source `OperatorCommitment`.
- If challenger side wins:
  - slashes operator bond according to configured rules;
  - creates `PayoutTimelock` for challenger payout;
  - sets challenge outcome `ChallengerCorrectOperatorSlashed`;
  - sets pending commitment status `ResolvedChallenger`;
  - sets resolved state source `ChallengerReveal`.
- If timeout occurs without quorum:
  - applies configured no-quorum policy;
  - sets challenge outcome `NoQuorum` if terminal.

Notes:

- The no-quorum policy must be finalized before implementation. Recommended v1
  policy: fail closed by refusing to finalize either state until a terminal
  council outcome is reached or an emergency authority cancels the commitment.

### `FinalizeCommitment`

Applies a commitment or resolved state to the delegated account.

Instruction data:

```text
FinalizeCommitmentArgs {
  state_source: FinalizeStateSource,
}

FinalizeStateSource =
  PendingOperatorState |
  ResolvedOperatorState |
  ResolvedChallengerState
```

Accounts:

```text
0. [signer] finalizer
1. [writable] pending_commitment
2. [writable] delegated_account
3. [writable] delegation_record
4. [writable] delegation_metadata
5. [] state_buffer
6. [] challenge_optional
7. [] protocol_config
8. [] system_program
```

Effects:

- For happy path:
  - verifies pending commitment status is `Active`;
  - verifies challenge window has closed;
  - verifies no unresolved challenge exists;
  - verifies `approval_count >= approval_threshold`;
  - verifies supplied full-state buffer hashes to pending commitment data hash.
- For match path:
  - same checks as happy path after challenge is terminal and cleared.
- For council-resolved operator path:
  - verifies pending commitment status is `ResolvedOperator`.
- For council-resolved challenger path:
  - verifies pending commitment status is `ResolvedChallenger`.
- Applies lamports, owner, and data from the selected opened state.
- Updates delegation metadata latest finalized commit id.
- Marks pending commitment `Finalized`.

Notes:

- A challenge reveal never directly finalizes account state.
- The finalizer may be permissionless. It earns no protocol authority by being
  the transaction signer.

### `ExtendChallengeWindow`

Extends the challenge window when the verifier approval threshold was not met.

Instruction data:

```text
ExtendChallengeWindowArgs {}
```

Accounts:

```text
0. [signer] cranker
1. [writable] pending_commitment
2. [] protocol_config
```

Effects:

- Verifies pending commitment status is `Active`.
- Verifies current slot is after `challenge_window_end_slot`.
- Verifies approval threshold has not been met.
- Verifies extension count is below `max_window_extensions`.
- Extends the window and applies configured threshold adjustment.

If maximum extensions have been reached, the commitment becomes `Expired` and
cannot finalize.

### `ClaimPayout`

Claims a correct-challenger payout after the timelock.

Instruction data:

```text
ClaimPayoutArgs {}
```

Accounts:

```text
0. [signer, writable] beneficiary
1. [writable] payout_timelock
```

Effects:

- Verifies current slot is at or after `unlock_slot`.
- Transfers payout to beneficiary.
- Marks payout claimed.

### `CloseTerminalAccounts`

Closes buffers and terminal protocol accounts when no longer needed.

Instruction data:

```text
CloseTerminalAccountsArgs {
  close_kind: CloseKind,
}
```

Accounts:

```text
0. [signer, writable] recipient
1. [writable] account_to_close
2. [] pending_commitment_or_challenge
```

Effects:

- Closes only accounts proven terminal.
- Returns rent to the chosen recipient according to protocol policy.

## Main Flows

### Happy Path

1. Operator registers and maintains an active bond.
2. Verifiers register and maintain active bonds.
3. A verifier snapshot is created.
4. Operator posts a pending commitment.
5. DLP requests VRF randomness.
6. VRF callback activates the commitment and selects verifiers.
7. Selected verifiers replay or verify off-chain and call `ApproveCommitment`.
8. Challenge window closes.
9. Anyone calls `FinalizeCommitment` with full state buffer.
10. DLP verifies threshold, no challenge, closed window, and state hash.
11. DLP applies the committed state and marks the commitment finalized.

### Insufficient Approvals

1. Commitment is active and no challenge is raised.
2. Challenge window closes below threshold.
3. Anyone calls `ExtendChallengeWindow`.
4. DLP extends the window and adjusts approval criteria.
5. If maximum extensions are reached, the commitment expires.

### Challenge With Invalid Reveal

1. Challenger raises a challenge with locked stake and salted hash.
2. Operator opens the committed state.
3. Challenger reveals state and salt.
4. DLP recomputes `challenge_hash`.
5. If the hash does not match, challenger stake is fully slashed.
6. Pending commitment returns to normal finalization path.

### Challenge With Matching State

1. Challenger raises a challenge.
2. Operator opens committed state.
3. Challenger reveals a valid preimage.
4. Revealed state matches operator state.
5. DLP charges `match_penalty_bps` of challenger stake to protocol.
6. Remaining challenger stake is unlocked.
7. Pending commitment returns to normal finalization path.

### Challenge With Mismatching State

1. Challenger raises a challenge.
2. Operator opens committed state.
3. Challenger reveals a valid but different state.
4. DLP opens a council case.
5. Council members vote on operator state or challenger state.
6. DLP resolves the council case.
7. Finalization applies the council-winning state.

### Operator Timeout

1. Challenger raises a challenge.
2. Operator does not respond before the timeout.
3. Anyone calls `MarkOperatorTimeout`.
4. Challenger must still reveal the challenger state and salt.
5. A valid reveal opens a council case with operator non-response evidence.
6. Council resolves the case.

### Challenger Reveal Timeout

1. Operator responds.
2. Challenger does not reveal before the timeout.
3. Anyone calls `MarkChallengerRevealTimeout`.
4. Challenger stake is fully slashed.
5. Pending commitment returns to normal finalization path.

### Challenger Correct

1. Council resolves in favor of challenger.
2. DLP slashes operator bond according to configured rules.
3. DLP creates a `PayoutTimelock`.
4. DLP marks commitment `ResolvedChallenger`.
5. Anyone finalizes the challenger state.
6. Challenger claims payout after the timelock.

## Off-Chain Responsibilities

### Operator Service

The operator service must:

- produce DA records for ER execution;
- compute canonical `data_hash`, `account_state_hash`, and DA pointer hash;
- post pending commitments;
- ensure VRF activation is cranked if necessary;
- open full state through `StateBuffer` for finalization or challenge response;
- monitor challenges and respond before timeout;
- monitor expired or finalized commitments for cleanup.

### Verifier Service

The verifier service must:

- track active pending commitments;
- detect when it is selected from a snapshot;
- fetch DA records;
- replay or verify ER execution;
- compare replay output to `account_state_hash`;
- submit `ApproveCommitment` if valid;
- optionally raise a challenge if replay output differs and the verifier is
  willing to lock challenger stake.

### Challenger Service

The challenger service must:

- run replica-mode validation or otherwise detect divergence;
- compute the correct challenger account state;
- generate a random salt;
- submit `RaiseChallenge` during the active challenge window;
- open challenger full-state data through `StateBuffer`;
- reveal before the deadline.

### Council Tooling

Council tooling must:

- list open council cases;
- display operator and challenger states;
- display DA/replay evidence and non-response evidence;
- submit `CouncilVote`;
- crank `ResolveCouncilCase` once a terminal condition is reachable.

### Cranker

A permissionless cranker should:

- call `ExtendChallengeWindow`;
- call `MarkOperatorTimeout`;
- call `MarkChallengerRevealTimeout`;
- call `ResolveCouncilCase`;
- call `FinalizeCommitment`;
- close terminal buffers and records.

## Message Summary

### On-Chain Instruction Messages

```text
InitializeProtocolConfig
UpdateProtocolConfig
RegisterOperator
RegisterVerifier
RequestStakeWithdrawal
WithdrawStake
CreateVerifierSetSnapshot
PostCommitment
ConsumeCommitmentRandomness
ApproveCommitment
RaiseChallenge
WriteStateBuffer
FinalizeStateBuffer
OperatorChallengeResponse
MarkOperatorTimeout
ChallengerReveal
MarkChallengerRevealTimeout
CouncilVote
ResolveCouncilCase
FinalizeCommitment
ExtendChallengeWindow
ClaimPayout
CloseTerminalAccounts
```

### Off-Chain Events To Index

The program should emit logs or events for:

- operator registered
- verifier registered
- verifier snapshot created
- commitment posted
- commitment activated
- verifier selected
- verifier approved
- challenge raised
- operator responded
- challenger revealed
- challenge terminal outcome
- council case opened
- council vote recorded
- council case resolved
- commitment finalized
- payout timelock created
- payout claimed

## Suggested Test Scenarios

### Commitment Activation

- Commitment cannot be approved before VRF callback.
- Commitment cannot finalize before VRF callback.
- VRF callback fails unless called with the expected VRF identity signer.
- VRF callback deterministically selects the same verifier indices from the same
  randomness and snapshot.

### Verifier Approval

- Non-selected verifier cannot approve.
- Unbonded verifier cannot approve.
- Exiting or jailed verifier cannot approve.
- Duplicate approval does not increase approval count.
- Approval after challenge window close fails.

### Finalization Guards

- Finalization fails before challenge window close.
- Finalization fails below approval threshold.
- Finalization fails with unresolved challenge.
- Finalization fails if supplied state buffer does not match `data_hash`.
- Finalization succeeds after threshold, closed window, no challenge, and matching
  state buffer.

### Challenge Path

- Challenge can be raised only during active challenge window.
- Challenge locks challenger stake.
- Challenge blocks finalization.
- Second challenge against the same pending commitment fails in v1.
- Operator response with a state hash different from the pending commitment
  fails.
- Operator timeout opens council case.
- Invalid challenger reveal fully slashes challenger.
- Matching reveal charges configured penalty and restores normal finalization.
- Mismatching reveal opens council case.
- Challenger reveal timeout fully slashes challenger.

### Council Resolution

- Non-member cannot vote.
- Duplicate council vote fails.
- Vote after deadline fails unless no-quorum resolution is being cranked.
- Operator supermajority resolves operator correct.
- Challenger supermajority resolves challenger correct.
- Challenger-correct outcome creates payout timelock.
- Finalization applies operator state after operator-correct resolution.
- Finalization applies challenger state after challenger-correct resolution.

### Payouts and Slashing

- Challenger payout cannot be claimed before timelock.
- Challenger payout can be claimed after timelock.
- Slashed operator cannot withdraw slashed stake.
- Exiting actor cannot be selected in new verifier snapshots.
- Stake withdrawal fails before withdrawal delay.

## Open Design Points

- Exact hash function.
- Exact binary serialization.
- Exact DA pointer format.
- Missing-account representation, if supported.
- Whether verifier selection is uniform or stake-weighted in v1.
- Whether verifier snapshots are bounded vectors or Merkleized from the start.
- Exact no-quorum policy.
- Exact operator slashing amount for challenger-correct outcomes.
- Whether a correct challenger receives the entire operator bond or a configured
  slash amount capped by bond.
- Whether verifier slashing for incorrect approvals is in v1 or reserved for v2.
- Whether multiple simultaneous challenges per commitment are needed after v1.
- Whether council is implemented inside DLP or as a separate council program
  that DLP invokes and verifies.

## Recommended Defaults For V1

- Use the Ephemeral VRF callback path for verifier selection.
- Use normal Solana signer transactions for verifier approvals.
- Use bounded on-chain verifier snapshots.
- Use one active challenge per pending commitment.
- Store pending commitment hashes and metadata only.
- Open full account state through chunked `StateBuffer` accounts.
- Implement council voting on-chain.
- Fail closed on no-quorum until the no-quorum policy is explicitly decided.
- Defer verifier slashing for incorrect approvals, but keep approval records so
  v2 can slash against historical evidence.
