#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
anchor_root="$repo_root/integration-tests/anchor"

mkdir -p "$anchor_root/target/deploy"
cp \
  "$anchor_root/tests/fixtures/test-delegation-keypair.json" \
  "$anchor_root/target/deploy/test_delegation-keypair.json"

cd "$anchor_root"
exec anchor test
