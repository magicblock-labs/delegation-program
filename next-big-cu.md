# Next major CommitFinalize CU optimization

The current steady-state `CommitFinalize` path consumes **2,083 CU** with the
temporary success log enabled.

Temporary probes produced this initial breakdown:

| Section                                           |      CU |
|---------------------------------------------------|--------:|
| Instruction-argument decode                       |     121 |
| DelegationState::decode-mut                       |     303 |
| State validation after decoding                   |     446 |
| Cluster borrow, decode, and live-policy checks    |     164 |
| Same-size resize call                             |      43 |
| No-op lamport settlement                          |      21 |
| Rent check                                        |     161 |
| Data borrow and copy                              |      74 |
| State-field writes                                |     144 |
| Everything else, including the temporary log      |    ~606 |
| **Total**                                         | **2,083** |

The two account-decoding sections therefore cost at most **467 CU** in this
initial measurement. The cluster figure also includes borrowing the account and
checking its live policy, so cluster decoding itself costs less than 164 CU.

A later, more focused measurement using `benchmark::start()` and dropping the
benchmark immediately after `DelegationState::decode_mut` measured that operation
at **411 CU**. We should use the focused result when prioritizing that specific
operation; the initial table remains useful as a rough breakdown of the complete
instruction.

## The larger change

Partial decoding alone cannot reduce the instruction below 500 CU. The next major
change should be a dedicated steady-state fast path inside `CommitFinalize`.

The common case has:

- the same account-data length;
- the same committed lamports;
- no undelegation request;
- a Direct cluster;
- either complete account data or an in-place diff.

This case does not need:

- a no-op resize;
- no-op lamport settlement;
- a Rent syscall when the operation cannot make rent status worse;
- the original-owner instruction argument or comparison;
- complete checked state decoding;
- complete checked cluster decoding;
- repeated checks for immutable facts already proved by `Delegate`.

The specialized path would do approximately this:

```text
parse compact arguments
verify the validator signer
open an unchecked fixed state header
verify the state PDA using fast SHA
verify the commit sequence
verify state.cluster == the passed cluster
open an unchecked cluster runtime header
verify the cluster is not paused
verify the current validator
copy complete data or apply the diff
update lamports, sequence, and requester directly
```

Any resize, lamport change, or unusual undelegation condition would use the
existing checked path.

## State layout

The frequently accessed fields should be in a small fixed header, with cold
delegation metadata after it:

```text
DelegationStateHot
- bump
- delegation cluster
- delegation mode
- lamports
- commit sequence
- undelegation requester

Cold state
- original owner
- rent payer
- delegation slot
- delegated-account bump
- encoded seeds
```

For the fast path:

- `DelegationMode::Direct` is stored after `Delegate` fully validates the cluster.
- The state PDA binds the state to its delegated account, making a stored
  `delegated_account` field unnecessary.
- `owner` is removed from the Direct `CommitFinalize` arguments because the
  original owner was established during delegation and cannot change.
- The state discriminator does not need to be checked after validating the unique
  state PDA.
- The live cluster still provides `paused` and `current_validator`, preserving
  pause and validator-rotation behavior.

## Unchecked Wheels views

Wheels should provide an unsafe mutable decode API for trusted layouts, for
example:

```rust
unsafe fn decode_mut_unchecked(
    storage: &impl LayoutStorageMut,
) -> Result<ViewMut<'_>, ProgramError>
```

It should still acquire the account borrow, but skip complete layout validation
and construct the view directly. Its safety contract must require that the bytes
were initialized with the exact layout and can only be structurally modified by
trusted program code.

This contract is suitable for the cluster address stored in `DelegationState`, as
long as delegation clusters remain non-closeable and every cluster update
preserves the layout. It is also suitable for `DelegationState` after its owner
and PDA identity have been established.

## Expected route toward 500 CU

Approximate opportunities are:

- remove the temporary success log: **250 CU**;
- use unchecked fixed state and cluster views: **300–400 CU**;
- move immutable checks to `Delegate`: **150–250 CU**;
- bypass rent, resize, and settlement in the safe steady case: **about 225 CU**;
- update scalar state fields directly: **more than 100 CU**;
- use a compact specialized processor and argument parser: **100–200 CU**.

These estimates overlap in places and must be measured one change at a time. The
first goal should be to reach the 600–900 CU range with the specialized path.
Unchecked borrows and a tightly inlined processor may then make sub-500 CU
possible.
