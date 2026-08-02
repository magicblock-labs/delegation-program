# MIMD-0025 Implementation Notes

Companion to `mimd-0025.md`. This file only captures low-level implementation
choices and message shapes. Protocol rationale stays in the MIMD.

## Decisions To Review

- Use `ephemeral-vrf`; DLP requests randomness and receives a callback.
- Start the challenge window after VRF activation, not at initial post.
- v1 verifier approvals are normal Solana signer transactions.
- `PendingCommitment` stores hashes/metadata; full data is opened via
  `StateBuffer` for finalize or dispute resolution.
- Council voting is on-chain; off-chain council services are clients.
- v1 supports one active challenge per pending commitment.
- Operator timeout is evidence only. Challenger must still reveal state+salt.

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
  verifier_snapshot,
  verifier_snapshot_hash,
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
| `ProtocolConfig` | `["mimd-protocol-config"]` | Global params, VRF config, council config, protocol fee vault. |
| `OperatorBond` | `["mimd-operator-bond", operator]` | Slashable operator stake and lifecycle. |
| `VerifierBond` | `["mimd-verifier-bond", verifier]` | Slashable verifier stake and eligibility. |
| `VerifierSetSnapshot` | `["mimd-verifier-snapshot", snapshot_id]` | Bounded list of eligible verifiers for a commitment round. |
| `PendingCommitment` | `["mimd-pending-commitment", account, commit_id]` | Main commitment state machine. |
| `StateBuffer` | `["mimd-state-buffer", account, commit_id, role, authority]` | Chunked full account data opened for finalize/reveal. |
| `Challenge` | `["mimd-challenge", account, commit_id, challenger]` | One challenge against one pending commitment. |
| `CouncilConfig` | `["mimd-council-config"]` | Council members, weights, quorum, voting timeout. |
| `CouncilCase` | `["mimd-council-case", challenge]` | Vote state for one challenged commitment. |
| `PayoutTimelock` | `["mimd-payout-timelock", challenge]` | Delayed payout for correct challenger. |

### Essential Fields

```text
ProtocolConfig {
  authority, paused,
  vrf_program, vrf_oracle_queue,
  council_config, protocol_fee_vault,
  min_operator_bond, min_verifier_bond, min_challenger_stake,
  challenge_window_slots,
  operator_response_timeout_slots,
  challenger_reveal_timeout_slots,
  council_voting_timeout_slots,
  payout_timelock_slots,
  selected_verifier_count, approval_threshold, max_window_extensions,
  match_penalty_bps, council_quorum_bps, council_supermajority_bps,
}

OperatorBond {
  operator_identity,
  stake_lamports,
  locked_lamports,
  status: Active | Exiting | Slashed | Jailed,
  withdraw_requested_slot: Option<u64>,
}

VerifierBond {
  verifier_identity,
  stake_lamports,
  status: Active | Exiting | Slashed | Jailed,
  registered_slot,
  withdraw_requested_slot: Option<u64>,
}

VerifierSetSnapshot {
  snapshot_id,
  created_slot,
  snapshot_hash,
  entries: Vec<{ verifier_identity, verifier_bond, weight }>,
}

PendingCommitment {
  status,
  operator_identity, operator_bond,
  account_pubkey, commit_id, delegation_record,
  da_pointer_hash, account_state_hash, data_hash,
  lamports, owner, state_commitment_hash,
  verifier_snapshot, verifier_snapshot_hash,
  challenge_window_id,
  posted_slot, activation_slot: Option<u64>, challenge_window_end_slot: Option<u64>,
  selected_verifier_indices: Vec<u32>,
  approval_bitmap: Vec<u8>, approval_count, approval_threshold,
  active_challenge: Option<Pubkey>,
  vrf_request_id: Option<Hash32>, vrf_randomness: Option<Hash32>,
  resolved_state_source: Option<OperatorCommitment | ChallengerReveal>,
}

PendingCommitmentStatus =
  AwaitingRandomness | Active |
  AwaitingOperatorResponse | AwaitingChallengerReveal |
  AwaitingChallengerRevealAfterOperatorTimeout | AwaitingCouncil |
  ResolvedOperator | ResolvedChallenger |
  Finalized | Expired | Cancelled

StateBuffer {
  role: OperatorFinalize | OperatorChallengeResponse | ChallengerReveal,
  authority, account_pubkey, commit_id,
  expected_data_hash, total_len, written_len, finalized,
  data: Vec<u8>,
}

Challenge {
  status,
  pending_commitment, challenger_identity,
  challenger_stake_lamports, challenge_hash,
  raised_slot,
  operator_response_deadline_slot,
  challenger_reveal_deadline_slot: Option<u64>,
  operator_state: Option<OpenedState>,
  challenger_state: Option<OpenedState>,
  council_case: Option<Pubkey>,
  outcome: Option<ChallengeOutcome>,
}

OpenedState {
  lamports, owner, data_hash, account_state_hash,
  state_buffer: Option<Pubkey>,
}

ChallengeStatus =
  AwaitingOperatorResponse | AwaitingChallengerReveal |
  AwaitingChallengerRevealAfterOperatorTimeout |
  AwaitingCouncil | Terminal

CouncilConfig {
  authority, epoch, quorum_bps, supermajority_bps, voting_timeout_slots,
  members: Vec<{ identity, weight, active }>,
}

CouncilCase {
  challenge, council_epoch, opened_slot, voting_deadline_slot,
  operator_votes, challenger_votes, abstain_votes,
  vote_bitmap: Vec<u8>,
  member_identities: Vec<Pubkey>,
  member_weights: Vec<u64>,
  outcome: Option<OperatorCorrect | ChallengerCorrect | NoQuorum>,
}

PayoutTimelock {
  challenge, beneficiary, amount_lamports, unlock_slot, claimed,
}
```

## Instructions

Account lists include only protocol-relevant accounts. Add payer/system/rent
accounts where account creation or lamport movement requires them.

| Instruction | Data | Key accounts | Effect |
| --- | --- | --- | --- |
| `InitializeProtocolConfig` | initial params | authority, config | Creates global config. |
| `UpdateProtocolConfig` | changed params | authority, config | Updates future-facing params only. |
| `RegisterOperator` | `amount_lamports` | operator signer, operator bond, config | Deposits slashable operator stake. |
| `RegisterVerifier` | `amount_lamports` | verifier signer, verifier bond, config | Deposits slashable verifier stake. |
| `RequestStakeWithdrawal` | actor kind | actor signer, bond, config | Marks actor exiting after delay. |
| `WithdrawStake` | actor kind | actor signer, bond, config | Withdraws unlocked unslashed stake. |
| `CreateVerifierSetSnapshot` | `snapshot_id` | cranker, config, snapshot, verifier bonds | Stores bounded active verifier list and hash. |
| `PostCommitment` | commit data below | operator, operator bond, pending commitment, delegated account, delegation record, config, verifier snapshot, DLP identity PDA, VRF queue/program | Creates `AwaitingRandomness` commitment and CPIs to VRF. |
| `ConsumeCommitmentRandomness` | randomness, pending commitment | VRF identity signer, pending commitment, config, snapshot | Selects verifiers and starts challenge window. |
| `ApproveCommitment` | `verifier_snapshot_index` | verifier signer, verifier bond, pending commitment, snapshot | Records selected verifier approval bit. |
| `WriteStateBuffer` | role, offset, total len, expected hash, chunk | authority signer, state buffer, pending commitment | Writes opened full-state data. |
| `FinalizeStateBuffer` | role | authority signer, state buffer, pending commitment | Freezes buffer after hash check. |
| `RaiseChallenge` | challenge hash, stake | challenger signer, challenge, pending commitment, config | Locks stake and blocks finalization. |
| `OperatorChallengeResponse` | opened state metadata | operator signer, pending commitment, challenge, optional state buffer | Opens operator state and starts challenger reveal timeout. |
| `MarkOperatorTimeout` | none | cranker, pending commitment, challenge | Records non-response and waits for challenger reveal. |
| `ChallengerReveal` | opened state metadata, salt | challenger signer, pending commitment, challenge, optional buffer, fee vault, optional council case/config | Validates challenge preimage and either penalizes, dismisses, or opens council case. |
| `MarkChallengerRevealTimeout` | none | cranker, pending commitment, challenge, fee vault | Slashes challenger for no reveal. |
| `CouncilVote` | member index, vote | council member signer, council case, challenge | Adds one weighted vote. |
| `ResolveCouncilCase` | none | cranker, council case, challenge, pending commitment, operator bond, fee vault, optional payout timelock | Resolves operator-correct, challenger-correct, or no-quorum. |
| `FinalizeCommitment` | state source | finalizer, pending commitment, delegated account, delegation record/metadata, state buffer, optional challenge, config | Applies happy-path or resolved state. |
| `ExtendChallengeWindow` | none | cranker, pending commitment, config | Extends or expires under-approved commitment. |
| `ClaimPayout` | none | beneficiary signer, payout timelock | Pays correct challenger after timelock. |
| `CloseTerminalAccounts` | close kind | recipient, account to close, terminal parent account | Closes terminal buffers/records. |

### Key Instruction Data

```text
PostCommitment {
  commit_id: u64,
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
  da_pointer_hash: Hash32,
  er_slot: Option<u64>,
}

ConsumeCommitmentRandomness {
  randomness: Hash32,
  pending_commitment: Pubkey,
}

ApproveCommitment {
  verifier_snapshot_index: u32,
}

RaiseChallenge {
  challenge_hash: Hash32,
  stake_lamports: u64,
}

OperatorChallengeResponse {
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
  state_buffer: Option<Pubkey>,
}

ChallengerReveal {
  lamports: u64,
  owner: Pubkey,
  data_hash: Hash32,
  salt: [u8; 32],
  state_buffer: Option<Pubkey>,
}

CouncilVote {
  member_index: u32,
  vote: Operator | Challenger | Abstain,
}

FinalizeCommitment {
  state_source:
    PendingOperatorState |
    ResolvedOperatorState |
    ResolvedChallengerState,
}
```

### Important Instruction Rules

- `PostCommitment` computes `state_commitment_hash`, stores the pending record,
  and requests VRF. `dlp_program_identity_pda` is signed by DLP with
  `invoke_signed`.
- `ConsumeCommitmentRandomness` verifies the VRF identity signer, derives unique
  selected verifier indices from randomness and `verifier_snapshot_hash`, and
  starts the challenge window.
- `ApproveCommitment` requires the verifier to be bonded, in the snapshot,
  selected, and still inside the challenge window. Duplicate approvals do not
  increment `approval_count`.
- `ChallengerReveal` has four terminal branches:
  invalid hash, matching state, mismatch after operator response, valid reveal
  after operator timeout.
- `FinalizeCommitment` on the happy path requires closed window, approval
  threshold, no unresolved challenge, and full-state hash match.
- `ResolvedOperator` finalizes operator-opened state.
- `ResolvedChallenger` finalizes challenger-opened state.

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
| Mismatch | Challenger raises, operator responds, challenger reveals different valid state, council resolves, winning state finalizes. |
| Operator timeout | Challenger raises, operator misses deadline, timeout recorded, challenger reveals, council resolves with non-response evidence. |
| Challenger timeout | Challenger raises, operator responds, challenger misses reveal deadline, challenger stake slashed, commitment returns to normal finalization. |

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
- Council tooling: present opened states/evidence and submit votes.
- Cranker: call timeout, extension, resolution, finalization, payout, and close
  instructions.

## Open Design Points

- Hash function and byte serialization.
- DA pointer wire format.
- Uniform vs stake-weighted verifier selection.
- Bounded vector vs Merkleized verifier snapshots.
- No-quorum council policy.
- Operator slash amount and challenger payout amount.
- Whether verifier slashing for bad approvals is v1 or v2.
- Whether council lives inside DLP or a separate council program.
