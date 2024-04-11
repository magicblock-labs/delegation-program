# Delegation program 

Delegation module for https://arxiv.org/pdf/2311.02650.pdf

## Program
- [`Consts`](src/consts.rs) – Program constants.
- [`Entrypoint`](src/lib.rs) – The program entrypoint.
- [`Errors`](src/error.rs) – Custom program errors.
- [`Idl`](idl/delegator.json) – Interface for clients, explorers, and programs.
- [`Instruction`](src/instruction.rs) – Declared instructions and arguments.
- [`Loaders`](src/loaders.rs) – Validation logic for loading Solana accounts.


## Instructions
- [`Delegate`](src/processor/delegate.rs) - TODO
- [`Update`](src/processor/update.rs) – TODO
- [`Undelegate`](src/processor/undelegate.rs) – TODO


## State


## Tests

To run the test suite, use the Solana toolchain: 

```
cargo test-sbf
```

For line coverage, use llvm-cov:

```
cargo llvm-cov
```
