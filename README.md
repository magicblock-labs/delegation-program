# Delegation program

Delegation program for MagicBlock Ephemeral Rollups, based on the
[protocol paper](https://arxiv.org/pdf/2311.02650.pdf).

## Repository layout

This repository is a Cargo workspace with two independently versioned
packages:

- [`program/`](program/) contains the on-chain program.
- [`interface/`](interface/) contains the public wire types, account layouts,
  PDA helpers, CPI helpers, instruction builders, and feature-gated off-chain
  helpers.

Anchor integration tests live in
[`integration-tests/anchor/`](integration-tests/anchor/). Design documents,
release documentation, IDLs, and audit reports live under [`docs/`](docs/),
[`idl/`](idl/), and [`audits/`](audits/).

## Build and test

Build the Rust workspace:

```bash
cargo build --workspace
```

Run the on-chain program tests with the Solana toolchain:

```bash
cargo test-sbf --manifest-path program/Cargo.toml --features unit_test_config
```

Run the Anchor integration tests after building the program:

```bash
cargo build-sbf --manifest-path program/Cargo.toml
npm --prefix integration-tests/anchor install
./scripts/run-anchor-tests.sh
```

For line coverage, use `cargo llvm-cov`, for example:

```bash
cargo llvm-cov --manifest-path program/Cargo.toml --test test_commit_state
```

`llvm-cov` currently does not work with instructions that perform CPIs, such
as delegate and undelegate.

## Upgrading the program

Upgrades go through a Squads multisig whose vault holds the upgrade authority.
CI only proposes them.

1. Run **Actions -> Propose Program Upgrade** with the target `cluster` (start
   with `dry_run`). It builds `delegation_program` verifiably, uploads a buffer,
   and opens a Squads proposal that upgrades the program and records its
   verified build. The job summary shows the proposal number and executable
   hash.
2. Members review and approve the proposal in the Squads app for mainnet. For
   devnet, members can use the shared command-line tools with their own keypair.
3. Once approved, any member with Execute permission executes it.
4. For mainnet, run **Actions -> Verify Program (Remote)**. OtterSec rebuilds
   the recorded commit and verifies it against the deployed binary.

See the [squads-program-upgrade action](https://github.com/magicblock-labs/.github/tree/main/actions/squads-program-upgrade#approving-and-managing-the-multisig-devnet)
for setup, membership changes, and per-cluster GitHub environments. See the
[release process](docs/releases/process.md) for package and tag conventions.
