# Delegation program

Delegation module for https://arxiv.org/pdf/2311.02650.pdf

## Public Api

- [`Instruction Builders`](dlp-api/src/instruction_builder/) – utilities to generate Instructions.
- [`Args`](dlp-api/src/args/) – Instructions arguments structures.
- [`Consts`](dlp-api/src/consts.rs) – Program constants.
- [`Errors`](dlp-api/src/error.rs) – Custom program errors.

## Program

- [`Entrypoint`](src/lib.rs) – The program entrypoint.
- [`Processors`](src/processor/) – Instruction implementations.

## Important Instructions

- [`Delegate`](src/processor/fast/delegate.rs) - Delegate an account
- [`CommitState`](src/processor/fast/commit_state.rs) – Commit a new state
- [`Finalize`](src/processor/fast/finalize.rs) – Finalize a new state
- [`Undelegate`](src/processor/fast/undelegate.rs) – Undelegate an account

## Tests

To run the test suite, use the Solana toolchain:

```bash
cargo test-sbf --features unit_test_config
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

## Upgrading the Program

Upgrades go through a Squads multisig whose vault holds the upgrade authority. CI only proposes them.

1. Run **Actions → Propose Program Upgrade** with the target `cluster` (start with `dry_run`). It builds `dlp` verifiably, uploads a buffer and opens a Squads proposal that upgrades the program and records its verified build. The job summary shows the proposal number and the executable hash.
2. Members review and approve:
   - **mainnet:** in the [Squads app](https://app.squads.so);
   - **devnet:** with the shared command-line tools, run locally with your own keypair:

   ```bash
   node approve.mjs --multisig <MULTISIG> --keypair <your keypair> <proposal #> --hash <executable hash>
   ```

3. Once approved, any member with Execute executes it.

See the [squads-program-upgrade action](https://github.com/magicblock-labs/.github/tree/main/actions/squads-program-upgrade#approving-and-managing-the-multisig-devnet) for setup, membership changes, and the per-cluster GitHub environments.
