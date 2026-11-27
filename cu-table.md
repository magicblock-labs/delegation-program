# Solana compute-unit reference

This document separates costs defined by the Solana runtime from costs produced
by the Rust compiler. The distinction matters: a syscall has a defined price,
but a Rust expression does not.

For example, `if true { ... }` normally costs nothing because the compiler
removes the condition. A condition read at runtime usually needs a one-CU SBF
branch instruction, but loading the value, selecting a result, and preserving
registers can add more instructions.

## Baseline

The runtime prices below come from Agave 4.1.1. The Rust examples were compiled
with cargo-build-sbf 4.1.0, platform-tools 1.54, and SBF Rust 1.89 using:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
```

The compiler examples were inspected in the generated SBF dump and measured
with `solana-program-test` 3.0.0. They do not invoke syscalls, so the runtime
version difference does not affect their instruction counts.

The main sources for the fixed prices and formulas are Agave's tagged
[`execution_budget.rs`], [`logging.rs`], [`mem_ops.rs`], [`syscalls/src/lib.rs`],
[`cpi.rs`], and [`sysvar.rs`].

## How to read the tables

| Label    | Meaning                                                                                                             |
| -------- | ------------------------------------------------------------------------------------------------------------------- |
| Exact    | The runtime explicitly charges this amount.                                                                         |
| Formula  | The runtime calculates the charge from the input. Integer division rounds down.                                     |
| SBF      | Number of executed SBF instructions in the shown compiled form. Each executed instruction normally consumes one CU. |
| Measured | Difference observed in one particular compiled benchmark. Recompilation can change it.                              |

Runtime charges do not include the SBF instructions needed to prepare and call a
syscall. Likewise, a callee's work is not included in the CPI base charge.

## SBF instructions and Rust constructs

There is no separate price for creating a local variable or reserving a stack
slot. Stack layout is decided at compile time. CU is spent only on instructions
that actually execute, such as moving, loading, storing, or using the value.

### Low-level operations

| Operation                                         | Typical SBF cost | Notes                                                                  |
| ------------------------------------------------- | ---------------: | ---------------------------------------------------------------------- |
| Move a register value                             |             1 CU | Often removed or combined with surrounding code.                       |
| Load 1, 2, 4, or 8 bytes                          |             1 CU | Assumes one SBF load instruction.                                      |
| Store 1, 2, 4, or 8 bytes                         |             1 CU | Assumes one SBF store instruction.                                     |
| Add, subtract, or multiply integers up to 64 bits |             1 CU | One arithmetic instruction after operands are available.               |
| Divide or take remainder up to 64 bits            |             1 CU | A zero check or divisor preparation may add instructions.              |
| Bitwise operation or shift                        |             1 CU | Preparing or masking the shift amount may add instructions.            |
| Compare and branch                                |             1 CU | Producing a Rust `bool` usually needs another one or two instructions. |
| Unconditional branch                              |             1 CU | It may disappear after optimization.                                   |
| Direct function call                              |             1 CU | Excludes argument setup, spills, the callee, and return handling.      |
| Function return                                   |             1 CU | An inlined function has no call or return instruction.                 |
| Borrow or reference                               |   0 CU by itself | It becomes pointer arithmetic, loads, or stores only when used.        |
| Drop a `Copy` value                               |             0 CU | A type with a destructor costs whatever its destructor executes.       |

### Concrete compiler examples

These are small `#[inline(never)]` function bodies from the benchmark. The
counts include the final `exit` instruction but not the caller.

| Rust pattern                                  | Executed SBF instructions | What was generated                                           |
| --------------------------------------------- | ------------------------: | ------------------------------------------------------------ |
| Return one `u64` argument                     |                         2 | Move and return.                                             |
| Runtime `if`, false path                      |                         3 | Default result, conditional branch, return.                  |
| Runtime `if`, true path                       |                         4 | Default result, conditional branch, selected result, return. |
| Volatile 8-byte stack round trip              |                         3 | Store, load, return.                                         |
| `u64` add, subtract, multiply, or bitwise AND |                         3 | Move, operation, return.                                     |
| `u64` division with a known nonzero divisor   |                         3 | Move, divide, return.                                        |
| Convert `u64 == u64` to `u64`                 |                       3–4 | Default result, comparison, optional replacement, return.    |
| Successful `checked_add(...).unwrap_or(0)`    |                         8 | Add, overflow detection, selection, return.                  |
| Non-overflowing `saturating_add`              |                         9 | Add, overflow detection, saturation selection, return.       |

The direct answer for `if bool` is therefore: the conditional jump itself is
normally **1 CU**, but a complete minimal `if` expression commonly executes a
few SBF instructions. A constant condition can compile to **0 CU**.

### Measured compiled patterns

The benchmark called each non-inlined operation in a loop, forced its result to
remain observable with a volatile store, measured 100 and 1,000 iterations, and
subtracted the matching identity loop. These numbers include caller-side
argument and result handling, so they are examples rather than language prices.

| Compiled pattern                                               | Extra CU per iteration over identity |
| -------------------------------------------------------------- | -----------------------------------: |
| Runtime `if`, false path                                       |                                    6 |
| Runtime `if`, true path                                        |                                    7 |
| Volatile 8-byte stack round trip                               |                                    5 |
| `u64` add, subtract, multiply, or bitwise AND                  |                                    6 |
| `u64` divide, remainder, shift, or equality                    |                                    7 |
| `u64` less-than                                                |                                    6 |
| Successful checked `u64` addition                              |                                   11 |
| Non-overflowing saturating `u64` addition                      |                                   12 |
| `u128` addition with both halves observable                    |                                   11 |
| `u128` multiplication with both halves observable              |                                  102 |
| Four-arm `match` in this input sequence                        |                         9.25 average |
| Four-element checked indexing with the index masked to `0..=3` |                                   19 |
| The same indexing with `get_unchecked`                         |                                   19 |

The checked and unchecked indexing examples compiled to the same function. The
mask proved the index was valid, so LLVM removed the bounds check. This is why
unsafe indexing should not be assumed to save CU without inspecting or measuring
the generated program.

An 8-byte local therefore has no fixed price. If it remains in a register, its
creation may cost zero. If it must be written to and read from the stack, the raw
store and load cost two CU, with possible additional register and pointer work.

## Memory operations

When Rust lowers a copy or comparison to a memory syscall, Agave charges:

```text
max(10, floor(bytes / 250)) CU
```

This is the syscall charge only. Small fixed-size operations may instead compile
to ordinary loads and stores.

|   Bytes | `memcpy`, `memmove`, `memcmp`, or `memset` charge |
| ------: | ------------------------------------------------: |
| 0–2,749 |                                             10 CU |
|   4 KiB |                                             16 CU |
|  10 KiB |                                             40 CU |
|  32 KiB |                                            131 CU |

## Logging and data exchange

| Operation                                      |                Runtime charge | Type    |
| ---------------------------------------------- | ----------------------------: | ------- |
| Log a UTF-8 string of `n` bytes                |                 `max(100, n)` | Formula |
| Log five `u64` values                          |                        100 CU | Exact   |
| Log a public key                               |                        100 CU | Exact   |
| Log remaining compute units                    |                        100 CU | Exact   |
| Read remaining compute units                   |                        100 CU | Exact   |
| Log `k` data fields containing `n` total bytes |              `100 + 100k + n` | Formula |
| Set `n` bytes of return data                   |        `100 + floor(n / 250)` | Formula |
| Get `n` bytes of return data                   | `100 + floor((n + 32) / 250)` | Formula |

The return-data copy charge is added only when nonempty return data is copied.

## Hashing and PDA operations

| Operation                                                           |                  Runtime charge | Type    |
| ------------------------------------------------------------------- | ------------------------------: | ------- |
| Create a PDA with known seeds and bump                              |                        1,500 CU | Exact   |
| Find a PDA bump                                                     |     1,500 CU per attempted bump | Formula |
| SHA-256, Keccak-256, Blake3, or SHA-512 over slices of lengths `nᵢ` | `85 + Σ max(10, floor(nᵢ / 2))` | Formula |
| Poseidon over `n` inputs                                            |                    `61n² + 542` | Formula |

The PDA-search syscall tries bumps internally and charges another 1,500 CU after
each failed attempt. Passing a known bump avoids that search, but the standard
PDA-creation syscall still charges 1,500 CU.

## CPI, sysvars, and heap

| Operation                              |                  Runtime charge | Notes                                                                                                |
| -------------------------------------- | ------------------------------: | ---------------------------------------------------------------------------------------------------- |
| CPI invocation base                    |                          946 CU | Excludes translation, account data, and the callee.                                                  |
| CPI data translation                   |      About one CU per 250 bytes | Several instruction, account-meta, account-info, and account-data components are charged separately. |
| Dedicated sysvar getter for type `T`   |          `100 + size_of::<T>()` | Applies to the older dedicated getter syscalls.                                                      |
| Generic `get_sysvar` copying `n` bytes | `100 + max(10, floor(n / 250))` | The 32-byte sysvar ID adds zero at the current divisor.                                              |
| Additional 32-KiB heap page            |                            8 CU | Charged for requested heap above the default.                                                        |

A CPI that invokes the System Program can never fit below the 946-CU invocation
base, even before the System Program and account translation do any work.

## Cryptographic syscalls

### Curve25519

| Operation                                |              Edwards |            Ristretto |
| ---------------------------------------- | -------------------: | -------------------: |
| Validate point                           |               159 CU |               169 CU |
| Add                                      |               473 CU |               521 CU |
| Subtract                                 |               475 CU |               519 CU |
| Multiply                                 |             2,177 CU |             2,208 CU |
| Multiscalar multiplication of `n` points | `2,273 + 758(n - 1)` | `2,303 + 788(n - 1)` |

### Other cryptography

| Operation                                                |                                   Runtime charge |
| -------------------------------------------------------- | -----------------------------------------------: |
| secp256k1 public-key recovery                            |                                        25,000 CU |
| alt_bn128 G1 addition                                    |                                           334 CU |
| alt_bn128 G2 addition                                    |                                           535 CU |
| alt_bn128 G1 multiplication                              |                                         3,840 CU |
| alt_bn128 G2 multiplication                              |                                        15,670 CU |
| alt_bn128 pairing of `n` pairs                           | `36,364 + 12,121(n - 1) + 85 + input_bytes + 32` |
| alt_bn128 G1 compression                                 |                                           130 CU |
| alt_bn128 G1 decompression                               |                                           498 CU |
| alt_bn128 G2 compression                                 |                                           186 CU |
| alt_bn128 G2 decompression                               |                                        13,710 CU |
| Big modular exponentiation with maximum input length `n` |                      `100 + floor(n² / 2) + 190` |
| BLS12-381 G1 addition / subtraction                      |                                     128 / 129 CU |
| BLS12-381 G2 addition / subtraction                      |                                     203 / 204 CU |
| BLS12-381 G1 / G2 multiplication                         |                                 4,627 / 8,255 CU |
| BLS12-381 G1 / G2 decompression                          |                                 2,100 / 3,050 CU |
| BLS12-381 G1 / G2 validation                             |                                 1,565 / 1,968 CU |
| BLS12-381 pairing of `n` pairs                           |                         `25,445 + 13,023(n - 1)` |

## Practical rules

- Measure the optimized SBF binary, not the Rust source in isolation.
- Compare complete alternatives under the same inputs. A tiny helper can change
  inlining, register allocation, and spills in its caller.
- Treat a local variable, reference, `Option`, `Result`, or enum as having no
  intrinsic CU price. Its generated instructions are what matter.
- Prefer runtime-defined formulas for syscalls. Do not infer syscall prices from
  one transaction when the input size changes the charge.
- Use repeated operations and a matched control when measuring single-digit CU.
  Calling the remaining-CU syscall itself costs 100 CU.
- Re-run measurements after changing compiler, SBF architecture, optimization
  profile, Agave version, or feature set.

[`execution_budget.rs`]: https://github.com/anza-xyz/agave/blob/v4.1.1/program-runtime/src/execution_budget.rs
[`logging.rs`]: https://github.com/anza-xyz/agave/blob/v4.1.1/syscalls/src/logging.rs
[`mem_ops.rs`]: https://github.com/anza-xyz/agave/blob/v4.1.1/syscalls/src/mem_ops.rs
[`syscalls/src/lib.rs`]: https://github.com/anza-xyz/agave/blob/v4.1.1/syscalls/src/lib.rs
[`cpi.rs`]: https://github.com/anza-xyz/agave/blob/v4.1.1/program-runtime/src/cpi.rs
[`sysvar.rs`]: https://github.com/anza-xyz/agave/blob/v4.1.1/syscalls/src/sysvar.rs
