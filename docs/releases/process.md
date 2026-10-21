# Release process

The program, interface, and client have independent versions and component
tags, following [Anza's convention](https://github.com/anza-xyz/solana-sdk/releases).

| Component | Version source | Initial version | Tag | Published artifact |
| --- | --- | --- | --- | --- |
| Delegation program | `program/Cargo.toml` | `2.0.0` | `delegation-program@vX.Y.Z` | GitHub Release |
| Program interface | `interface/Cargo.toml` | `0.1.0` | `delegation-program-interface@vX.Y.Z` | GitHub Release and `magicblock-delegation-program-interface` on crates.io |
| Client helpers | `client/Cargo.toml` | `0.1.0` | `delegation-program-client@vX.Y.Z` | GitHub Release and `magicblock-delegation-program-client` on crates.io |

Each package follows its own SemVer history. The program starts at `2.0.0`
because it continues the existing on-chain program. The interface and client
start at `0.1.0` because they are new Rust APIs that may still change. Keeping
their versions equal would imply a shared release history and require lockstep
releases even when only one component changes.

The old `magicblock-delegation-program-api` crate and its tags remain unchanged
for older clients. Do not publish new versions under that name.

The interface cannot be published until its `wheels = "0.1.0"` dependency is
available on crates.io. The client must be published after the interface version
declared in `client/Cargo.toml` is available.

## Prepare a release

1. Start from an up-to-date `main` branch.
2. Create `release/program/vX.Y.Z`, `release/interface/vX.Y.Z`, or
   `release/client/vX.Y.Z` for the component being released.
3. Update that component's manifest version and its package entry in
   `Cargo.lock`. Do not change unrelated package versions.
4. Prepare release notes that describe behavior changes, compatibility
   requirements, and the previous release used for comparison.
5. Open a pull request against `main` and require all checks to pass.

Release branch pushes run the corresponding crate publishing workflow in dry
run mode. Manual workflow runs also default to dry run. `CRATES_TOKEN` is needed
only for a real crates.io publication.

## Tag and publish

After merging, create an annotated component tag at the exact merged commit.
Create its GitHub Release with `gh release create --verify-tag --target main`.

The interface and client GitHub Releases publish their crates to crates.io. If
both are released together, publish the interface first because the client
depends on its declared interface version. Confirm the interface is available
on crates.io before publishing the client.

The program crate has `publish = false`. Its GitHub Release records the source
version; deployment follows the separate program upgrade process in the
[README](../../README.md#upgrading-the-program).

## Record deployment provenance

Program release notes should identify the cluster, program ID, deployed source
commit, verified executable hash, and verification evidence when available. A
release tag records source history; it does not prove that the build is deployed
or that its executable matches the deployed binary.
