# Release Notes

## Delegation program 2.0.1

`DelegateWithActions` now accepts a cleartext non-signer account meta that refers to a signer entry in the compact pubkey table. A signer meta must still refer to signer storage, and every pubkey in signer storage must have a signed account in the instruction. This fixes mixed signer/non-signer use in post-delegation actions without changing the instruction format.

Clients built with `dlp-api` 4.0.0 that produced valid requests for program 2.0.0 remain compatible. An older client's `cleartext_with_insertable` output still fails when a key inserted as a non-signer is later used as a signer; that output was also rejected by program 2.0.0. Use `dlp-api` 4.0.1 for that case.

Changes since `dlp@v2.0.0`: [compare](https://github.com/magicblock-labs/delegation-program/compare/dlp@v2.0.0...main) · [#187](https://github.com/magicblock-labs/delegation-program/pull/187).

This release records source version 2.0.1. Program deployment and executable verification are separate; cluster, program ID, deployed source commit, and executable hash are not yet available for these notes.
