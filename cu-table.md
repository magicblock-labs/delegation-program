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

### Function calls and scalar arguments

The next benchmark used separate `#[inline(never)]` functions. Each callee added
its scalar arguments, and the caller kept the result observable. The identity
loop itself executed six SBF instructions per iteration.

| Compiled pattern           | Total CU per iteration | Extra CU over no-call loop | Notes                                                        |
| -------------------------- | ---------------------: | -------------------------: | ------------------------------------------------------------ |
| No call                    |                      6 |                          0 | Matching volatile-loop baseline.                             |
| Pure call, no arguments    |                      6 |                          0 | LLVM removed the call.                                       |
| Pure identity call         |                      6 |                          0 | LLVM removed this one-argument call too.                     |
| Sum two scalar arguments   |                     12 |                          6 | Includes call, callee addition, return, and caller handling. |
| Sum three scalar arguments |                     14 |                          8 | Includes two additions in the callee.                        |
| Sum four scalar arguments  |                     16 |                         10 | Includes three additions in the callee.                      |
| Sum five scalar arguments  |                     18 |                         12 | Includes four additions in the callee.                       |
| Sum six scalar arguments   |                     23 |                         17 | Additional arguments begin using the stack ABI.              |
| Sum seven scalar arguments |                     26 |                         20 | Includes stack-argument loads and six additions.             |
| Sum eight scalar arguments |                     29 |                         23 | Includes stack-argument loads and seven additions.           |

These rows do **not** say that a two-argument function call costs six CU. A
direct SBF call and return are one instruction each, but the table also includes
argument preparation, the additions, result handling, and any ABI work.

SBF exposes five argument registers. In this build, once the function had more
than five scalar parameters, the compiler used the fifth register as a pointer
to stack arguments. The generated six-argument callee loaded the extra values
from that stack area. This explains the visible increase between five and six
arguments, but it is an ABI observation rather than a stable Rust guarantee.

Also, `#[inline(never)]` prevents inlining; it does not force an otherwise
unnecessary call to remain. LLVM still proved the zero-argument and identity
calls had no relevant side effects and removed them.

### Passing, copying, and returning values

The benchmark built the aggregate values from the changing loop state so LLVM
could not move them outside the loop. The value and reference variants then
summed every `u64` field.

| Compiled pattern                | Total CU per iteration | Extra CU over identity | Observation                                                    |
| ------------------------------- | ---------------------: | ---------------------: | -------------------------------------------------------------- |
| Read a `u64` through `&u64`     |                     11 |                      5 | Includes a call, one dereference, return, and caller handling. |
| Pass 16 bytes by value          |                     16 |                     10 | Same generated callee as the reference form in this build.     |
| Pass 16 bytes by reference      |                     16 |                     10 | Aggregate construction and two field reads dominate.           |
| Pass 32 bytes by value          |                     26 |                     20 | Passed indirectly by this compiler and ABI.                    |
| Pass 32 bytes by reference      |                     26 |                     20 | No measured advantage over by value in this example.           |
| Pass 64 bytes by value          |                     46 |                     40 | Passed indirectly by this compiler and ABI.                    |
| Pass 64 bytes by reference      |                     46 |                     40 | No measured advantage over by value in this example.           |
| Volatile-read 16-byte aggregate |                     13 |                      7 | LLVM read the two fields directly; no destination copy.        |
| Volatile-read 32-byte aggregate |                     17 |                     11 | Four loads and additions, plus call/return handling.           |
| Volatile-read 64-byte aggregate |                     25 |                     19 | Eight loads and additions, plus call/return handling.          |
| Return a 16-byte aggregate      |                     19 |                     13 | Uses a hidden output pointer and stores two fields.            |
| Return a 32-byte aggregate      |                     29 |                     23 | Uses a hidden output pointer and stores four fields.           |
| Return a 64-byte aggregate      |                     49 |                     43 | Uses a hidden output pointer and stores eight fields.          |
| Force a 16-byte stack local     |                     15 |                      9 | Constructs, stores, reloads, and consumes two fields.          |
| Force a 32-byte stack local     |                     25 |                     19 | Constructs, stores, reloads, and consumes four fields.         |
| Force a 64-byte stack local     |                     45 |                     39 | Constructs, stores, reloads, and consumes eight fields.        |

The by-value results do not mean that copying 64 bytes is free. For these
function signatures, the Rust ABI passed the larger values indirectly, so the
by-value and reference callees compiled to the same field loads. A different
type, caller, mutation pattern, or compiler version may require an actual copy.

Likewise, `let copied = value` has no fixed cost even when the type implements
`Copy`. LLVM may keep a scalar in the same register, emit a one-CU move, read the
original fields directly, expand a small copy into loads and stores, or lower a
larger copy to a memory syscall. Copying a reference has the same rule: it copies
one pointer-sized value and often needs no instruction if the existing register
can be reused. Dereferencing the copied reference is a separate load.

Returning aggregates is different in this benchmark. Values larger than one
register used a hidden output pointer supplied by the caller, and the callee
stored each field through it. This made aggregate returns visibly more expensive
than returning a `u64`.

### Local variables and stack storage

A local declaration reserves no CU by itself. Stack-frame size is decided when
the program is compiled, and unused locals disappear. Runtime cost appears only
when generated instructions initialize, spill, load, store, copy, or drop the
value.

| Source-level action                     | Possible generated work                                      |
| --------------------------------------- | ------------------------------------------------------------ |
| Bind a value already in a register      | Nothing, or a one-CU register move.                          |
| Copy an immutable reference             | Nothing, or a one-CU pointer move.                           |
| Dereference a reference                 | Usually one load after its address is available.             |
| Spill one scalar to the stack           | One store; loading it again costs one more instruction.      |
| Initialize a fixed-size stack aggregate | Commonly one store per scalar-sized field, plus setup.       |
| Read the aggregate fields               | Commonly one load per field, unless values remain available. |
| Drop a type without `Drop`              | No runtime work solely for the drop.                         |
| Drop a type implementing `Drop`         | Whatever the generated destructor and its callees execute.   |

The forced-local rows above deliberately used volatile access. Normal Rust code
often costs less because LLVM can keep fields in registers, forward values from
stores to later uses, or remove the local entirely.

### Reading integers from byte buffers

For `u32` and `u64`, `read_unaligned()` was **not cheaper** than copying the
bytes into a temporary array and calling `from_le_bytes()`. The optimized SBF
contained no temporary-array copy. LLVM reduced both forms to one integer load.

The benchmark compared these forms:

```rust
// The caller must guarantee that at least size_of::<T>() bytes are valid.
let value = unsafe { pointer.cast::<u64>().read_unaligned() };

// Bounds-checked slice followed by a temporary array.
let value = u64::from_le_bytes(data[..8].try_into().unwrap());
```

| Compiled form                                             | Successful-path SBF instructions | Generated work                |
| --------------------------------------------------------- | -------------------------------: | ----------------------------- |
| Raw `read_unaligned::<u32>()`                             |                                2 | One 32-bit load, then `exit`. |
| Raw array copy followed by `u32::from_le_bytes()`         |                                2 | The same load and `exit`.     |
| Raw `read_unaligned::<u64>()`                             |                                2 | One 64-bit load, then `exit`. |
| Raw array copy followed by `u64::from_le_bytes()`         |                                2 | The same load and `exit`.     |
| Length check, then `read_unaligned::<u32>()`              |                                3 | Branch, 32-bit load, `exit`.  |
| `data[..4].try_into()` followed by `u32::from_le_bytes()` |                                3 | Branch, 32-bit load, `exit`.  |
| Length check, then `read_unaligned::<u64>()`              |                                3 | Branch, 64-bit load, `exit`.  |
| `data[..8].try_into()` followed by `u64::from_le_bytes()` |                                3 | Branch, 64-bit load, `exit`.  |

The raw copy and `read_unaligned()` functions even shared the same symbol in
the generated binary because their machine code was identical. The checked
versions also had identical successful paths. These counts include the callee's
final `exit`, but not a caller-side call instruction.

Therefore, prefer the safe temporary-array form when starting from a slice. It
retains bounds checking and explicitly states the byte order without costing
more CU in this build. `read_unaligned()` is useful only when the surrounding
decoder has already proved the byte range valid and the unsafe access makes that
contract clear; skipping the check is otherwise unsound.

`read_unaligned::<u64>()` reads native-endian data. SBF is little-endian, but
portable code should still use `u64::from_le(read_value)` when the serialized
format is little-endian. The conversion compiles away on SBF.

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
