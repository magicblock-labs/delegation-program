# Delegation program 

Delegation module for https://arxiv.org/pdf/2311.02650.pdf

## Program

- [`Consts`](src/consts.rs) – Program constants.
- [`Entrypoint`](src/lib.rs) – The program entrypoint.
- [`Errors`](src/error.rs) – Custom program errors.
- [`Idl`](idl/delegator.json) – Interface for clients, explorers, and programs.
- [`Instruction`](src/instruction.rs) – Declared instructions and arguments.
- [`Loaders`](src/loaders.rs) – Validation logic for loading Solana accounts.


## Instructions

- [`Delegate`](src/processor/delegate.rs) - Delegate an account
- [`CommitState`](src/processor/update.rs) – Commit a new state
- [`Undelegate`](src/processor/undelegate.rs) – Undelegate an account 


## State

- [`CommitState`](src/state/commit_state.rs) – Commit state account state.
- [`Delegator`](src/state/delegator.rs) – Delegator account state.

## Tests

To run the test suite, use the Solana toolchain: 

```
cargo test-sbf
```

For line coverage, use llvm-cov:

```
cargo llvm-cov --test test_commit_state
```

(llvm-cov currently does not work with instructions with CPIs e.g.: delegate, undelegate)

## Integration Tests

The integration tests are located in the `tests/integration` directory.
The tests consist of a Bolt/Anchor program that uses the delegation program to delegate, commit, and undelegate accounts.
This can be also used a reference for how to interact with the program.

To run the integration test, use Bolt or Anchor:

```
cd tests/integration && bolt test
```

or:

```
cd tests/integration && anchor test
```
