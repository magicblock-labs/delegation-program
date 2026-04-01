### Release Process

1. **Create a Release Branch**  
   Create a new branch named `release/v*.*.*` from `main. Use dlp-api version as the branch name, not the dlp version.

2. **Update the Crate Versions**  
   Increment the version in `dlp-api/Cargo.toml` for the release you are preparing.
    - Incrementing the version in dlp's `Cargo.toml` is optional, as it will not be published.

3. **Align the Release Manifests**  
   Run `bash ./.github/version_align.sh`. This will:
   - update the versioned `dlp-api` entries in the root `Cargo.toml` to match `dlp-api/Cargo.toml`
   - keep the `magicblock-delegation-program` self dev-dependency path-only so the root crate can publish cleanly

4. **Commit and Push the Release Branch**  
   Commit the version updates and aligned manifest, then push the `release/v*.*.*` branch.

5. **Continuous Integration (CI) Dry Run**  
   The publish workflow runs on `release/v*` pushes in dry-run mode.
   Ensure the workflow passes before continuing.

6. **Merge and Publish**  
   Merge the release branch into `main`, then create a new GitHub Release.
   This triggers the real publish flow for `magicblock-delegation-program-api`.
   - Note that `magicblock-delegation-program` will not be published.

7. **Post-Deployment**  
   Verify that `magicblock-delegation-program-api` is available as expected after the GitHub Release completes.

### Notes

- `CRATES_TOKEN` must be configured in the repository secrets.
- Manual `workflow_dispatch` runs default to dry-run. Only set `dry_run` to `false` when you intentionally want a real publish.
