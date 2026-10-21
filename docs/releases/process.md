# Release process

The program and interface have independent versions and component tags,
following [Anza's convention](https://github.com/anza-xyz/solana-sdk/releases).

| Component | Version source | Initial version | Tag | Published artifact |
| --- | --- | --- | --- | --- |
| Delegation program | `program/Cargo.toml` | `2.0.0` | `delegation-program@vX.Y.Z` | GitHub Release |
| Program interface | `interface/Cargo.toml` | `0.1.0` | `delegation-program-interface@vX.Y.Z` | GitHub Release and `magicblock-delegation-program-interface` on crates.io |

Each package follows its own SemVer history. The program starts at `2.0.0`
because it continues the existing on-chain program. The interface starts at
`0.1.0` because it is a new Rust API that may still change. Keeping their
versions equal would imply a shared release history and require lockstep
releases even when only one component changes.

The old `magicblock-delegation-program-api` crate and its tags remain unchanged
for older clients. Do not publish new versions under that name.

The interface cannot be published until its `wheels = "0.1.0"` dependency is
available on crates.io.

## Prepare a release

1. Start from an up-to-date `main` branch.
2. Create a `release/vX.Y.Z` branch. One branch and pull request may prepare the
   program, the interface, or both. Manifest versions and component tags are
   authoritative when their versions differ.
3. Update each component being released and its package entry in `Cargo.lock`.
   Do not change unrelated package versions.
4. Prepare release notes that describe behavior changes, compatibility
   requirements, and the previous release used for comparison.
5. Open a pull request against `main` and require all checks to pass.

Release branch pushes that change the interface run its publishing workflow in
dry-run mode. Manual workflow runs also default to dry run. `CRATES_TOKEN` is
needed only for a real crates.io publication.

## Tag and publish

After merging an interface release, create an annotated component tag at the
exact merged commit. Create its GitHub Release with
`gh release create --verify-tag --target main`. Publishing that release
publishes the interface crate to crates.io.

The program crate has `publish = false` and is never published to crates.io.
After deployment, tag the exact source commit that produced the deployed
program and create its GitHub Release. Deployment follows the separate program
upgrade process in the [README](../../README.md#upgrading-the-program).

## Record deployment provenance

Program release notes should identify the cluster, program ID, deployed source
commit, verified executable hash, and verification evidence when available. A
release tag records source history; it does not prove that the build is deployed
or that its executable matches the deployed binary.
