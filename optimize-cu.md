# CommitFinalize CU optimization analysis

Yes. I reproduced the gap.

The reliable comparison is:

- Current v2 full-data commit with a lamport increase: **6,568 CU**
- Current v2 diff commit with a lamport decrease: **5,618 CU**
- PR #141 ordinary v2 CommitFinalize, copying 4 KiB: **450 CU**
- Your temporary success-path log adds another **250 CU**

I could not reproduce a successful sub-200-CU instruction. The old stack contains a function named `process_commit_finalize_inline_101_cu`, but that experimental path fails with `InvalidArgument` in the old test environment. The 450-CU ordinary path is the reliable baseline.

## Where the CU goes

| Rank | Difference | Approximate current cost | Relative impact |
|---|---|---:|---:|
| 1 | Two `create_program_address` checks | **3,000 CU exact** | 46% of 6,568 |
| 2 | Entrypoint, Wheels argument decoding, live cluster loading and validation | **~1,250 CU** | ~19% |
| 3 | System Program CPI when lamports increase | **1,000+ CU** | 15% |
| 4 | Apply/update tail: resizing, rent check, copying/diff, state updates | **~600 CU** | ~9% |
| 5 | DelegationState decoding and validation | **~500 CU** | ~8% |
| 6 | Remaining control flow and account handling | **~200 CU** | ~3% |

The approximate values come from comparing the CU consumed by the existing failure paths at progressively later validation stages. The PDA and CPI costs are runtime constants.

### 1. PDA checks are the largest problem

The current code calls `Address::create_program_address` for both the cluster and delegation state:

- [cluster validation](program/src/processor/internal/commit_finalize.rs#L139)
- [state validation](program/src/processor/internal/commit_finalize.rs#L175)

Each call costs **1,500 CU**, even though we already have the bump.

The old ordinary path computed the state PDA using the SHA syscall directly. With these seed lengths, that is approximately **136–137 CU per PDA**, rather than 1,500. The old inline path did no PDA computation at all.

This gives us two possible optimizations:

- Conservative: use the stored bump and fast SHA validation. Two PDAs would cost roughly **273 CU**, saving about **2,727 CU**.
- Aggressive: treat a DLP-owned, correctly discriminated account and its stored bindings as authoritative. This was the old `ValidatedDelegationBindings` security model and removes the hashes completely.

I think the aggressive approach is defensible, provided initialization remains the only way to create these account types. We would still verify:

- owner and discriminator;
- `state.delegated_account == delegated_account`;
- `state.delegation_cluster == cluster`;
- live cluster validator and policy.

Rotation means we must continue loading the live cluster; we cannot simply restore the old immutable validator binding.

### 2. The old implementation did much less work

The old code did not have a cluster account. It therefore did not check:

- whether the cluster is paused;
- whether it is Direct or fraud-proof enabled;
- the current validator after rotation;
- the state-to-cluster binding.

Instead, it stored an immutable validator key in `DelegationState`. That was cheaper, but it does not support our current rotation design.

The current generic layout access also does more work:

- `DelegationCluster::decode`;
- `DelegationState::decode`;
- field-by-field access through generated views;
- `DelegationState::decode_mut` again at the end.

Because these account layouts use `buffer_offset = any`, Pubkey accessors also copy 32-byte values rather than exposing aligned references. The old code cast one fixed header and accessed its fields directly.

We should retain Wheels as the account format, but CommitFinalize probably needs an optimized hot-path view or at least one mutable decode rather than decoding the state twice.

### 3. The old `lamports` field was effectively fake

PR #141 accepted `lamports`, but never used it. Its test passed `1_000_000` and never checked the resulting account balance.

The current implementation actually settles the balance:

```rust
settle_lamports(...)?;
```

When the validator must add lamports, that requires a System Program CPI. The runtime charges **1,000 CU before the System Program does any work**.

Therefore:

- A CommitFinalize that performs an inbound System Transfer can never consume less than 500 CU.
- Sub-500 is only possible for the DLP instruction when lamports are unchanged/decreased, or if the increase is funded beforehand.
- Alternatives are a separate preceding System Transfer or a DLP-owned settlement vault. A separate transfer lowers CommitFinalize CU, but not total transaction CU.

This is the largest semantic reason the old number is not apples-to-apples.

### 4. Rent, resizing and state application differ

The current implementation:

- resizes the account;
- settles lamports;
- reads the Rent sysvar;
- verifies rent exemption;
- copies data or applies the diff;
- decodes the state again and updates three fields.

The Rent syscall alone is about **100 CU**.

The old performance test kept the account length unchanged. Even its “inline resize” test used `ACCOUNT_SIZE + 0 * KB`, so it did not benchmark a real resize.

### 5. Copying is not the culprit

The old successful test copied **4 KiB** and still consumed only 450 CU. Under the current cost schedule, a 4-KiB `memcpy` syscall is charged roughly **16 CU**; 10 KiB is roughly **40 CU**, plus small call overhead.

So optimizing the copy first would be looking in the wrong place. The fixed validation and settlement overhead dominates.

## Important incompleteness in the old stack

We should reuse its performance ideas, not transplant the code. The experimental stack also had these issues:

- `lamports` was ignored;
- no rent validation;
- no cluster or validator rotation support;
- no committed-owner validation;
- a hard-coded bump of `255` in one path;
- the “101 CU” path hard-coded the validator;
- extensive unchecked account and slice access;
- the inline diff branch was effectively unfinished;
- its resize benchmark did not resize.

The useful ideas from the Graphite stack are primarily:

- validate immutable bindings during delegation;
- avoid curve-checking PDA syscalls on the hot path;
- use one direct state view;
- keep success paths log-free;
- specialize frequently executed instructions.

The v2-relevant work is mainly in [#140](https://github.com/magicblock-labs/delegation-program/pull/140), [#141](https://github.com/magicblock-labs/delegation-program/pull/141), and [#142](https://github.com/magicblock-labs/delegation-program/pull/142). The lower PRs mostly provide the legacy fast paths, buffer variants, dependency upgrade, and formatting.

## Is less than 500 CU realistic?

For the steady path—same-sized account, unchanged/decreased lamports—I think **yes**. The old separate-state implementation already achieved 450 CU, so embedding state inside the delegated account is not necessary.

My optimization order would be:

1. Replace the two 1,500-CU PDA checks, preferably with authoritative initialized bindings or at least fast SHA validation.
2. Decode each account only once and introduce a fixed-offset hot view that avoids 32-byte copies.
3. Skip rent and resize work when neither account length nor relevant lamports changed.
4. Decide how inbound lamports should be funded; sub-500 is impossible while the System CPI remains inside CommitFinalize.
5. Only then consider custom entrypoint deserialization or more unchecked access.

Before changing code, I would establish separate CU baselines for same-size/full, same-size/diff, resize, lamport decrease, and lamport increase. That will prevent CPI and resizing from hiding progress in the main fast path.
