# Release process

The program and API have independent versions. Use component tags, following
[Anza's convention](https://github.com/anza-xyz/solana-sdk/releases), so each
component has a clear release sequence.

| Component | Version source | Tag | Published artifact |
| --- | --- | --- | --- |
| Delegation program | Root `Cargo.toml` | `dlp@vX.Y.Z` | GitHub Release |
| Public API | `dlp-api/Cargo.toml` | `dlp-api@vX.Y.Z` | GitHub Release and `magicblock-delegation-program-api` on crates.io |

Keep historical tags unchanged. The previous program release is `v1.2.0`;
the subsequent unprefixed releases through `v3.1.0` belong to the API. Use
the preceding release of the same component when choosing a version and
writing release notes. The program crate is no longer published to crates.io.

## Prepare the release commit

1. Start from an up-to-date `main` branch. For an API release, including a paired
   program/API release, create a release branch named `release/v<API version>`,
   such as `release/v4.0.0`. For a program-only release, create a release branch
   named `release/dlp-v<program version>`.
2. Update the manifest version for each component being released and its
   workspace package entry in `Cargo.lock`. Keep unrelated dependency versions
   unchanged. The program and API versions do not need to match.
3. Prepare separate release notes for each component. Describe behavior changes,
   client compatibility requirements, and the comparison with its predecessor.
4. Commit the release preparation and open a PR against `main`.

## Validate before publishing

Require the PR checks to pass. API release branch pushes to `release/v*` also
run the publishing workflow in dry-run mode; that workflow must pass before
publishing the API.

The API publishing workflow handles its triggers as follows:

| Trigger | Behavior |
| --- | --- |
| Publish a `dlp-api@vX.Y.Z` GitHub Release | Require the tag to match the API manifest version, then build, test, and publish the API crate |
| Publish a program or other GitHub Release | Skip the API publishing jobs |
| Push a `release/v*` branch | Run the API publish dry run |
| Manual run | Default to a dry run; choose `dry_run=false` only for an intentional API publication |

`CRATES_TOKEN` must be configured in the repository secrets. The release tag
must point to a commit containing the component-aware publishing workflow.

## Tag and publish

After merging, resolve the exact merged release commit. Create an annotated
tag for each released component at that SHA, using the version in its manifest.
For a paired release, both tags identify the same commit. Push only the intended
tag refs.

Create each GitHub Release with `gh release create --verify-tag --target main`
and its prepared notes. Use the component tag as the release title. For a paired
release, publish the API release with `--latest=false` and mark the program
release Latest with `--latest=true`.

The API GitHub Release triggers crates.io publication. Confirm that the workflow
succeeds and the intended API version is available on crates.io. Publishing a
program GitHub Release records its source version; deployment follows the
separate [program upgrade process](README.md#upgrading-the-program).

## Record deployment provenance

Program release notes should identify the cluster, program ID, deployed source
commit, verified executable hash, and verification evidence when available.
Record the release commit separately if it differs from the deployed source.
A version bump or release tag does not establish that its build is deployed or
that its executable is identical to the deployed binary.
