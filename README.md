# Delegation program

Delegation module for https://arxiv.org/pdf/2311.02650.pdf

## Api

- [`Consts`](src/consts.rs) – Program constants.
- [`Instruction`](src/instruction/*.rs) – utilities to generate Instructions.
- [`Args`](src/args/*.rs) – Instructions arguments structures.
- [`Errors`](src/error.rs) – Custom program errors.

## Program

- [`Entrypoint`](src/lib.rs) – The program entrypoint.
- [`Processors`](src/processors/) – Instruction implementations.

## Important Instructions

- [`Delegate`](src/processor/delegate.rs) - Delegate an account
- [`CommitState`](src/processor/commit_state.rs) – Commit a new state
- [`Finalize`](src/processor/finalize.rs) – Finalize a new state
- [`Undelegate`](src/processor/undelegate.rs) – Undelegate an account

## Tests

To run the test suite, use the Solana toolchain:

```bash
cargo test-sbf
```

For line coverage, use llvm-cov:

```bash
cargo llvm-cov --test test_commit_state
```

(llvm-cov currently does not work with instructions with CPIs e.g.: delegate, undelegate)

## Integration Tests

The integration tests are located in the `tests/integration` directory.
The tests consist of a Bolt/Anchor program that uses the delegation program to delegate, commit, and undelegate accounts.
This can be also used a reference for how to interact with the program.

To run the integration test, use Bolt or Anchor:

```bash
cd tests/integration && bolt test
```

or:

```bash
cd tests/integration && anchor test
```
