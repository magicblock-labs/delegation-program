#!/bin/bash

set -euo pipefail

root_manifest="Cargo.toml"
api_manifest="dlp-api/Cargo.toml"
mode="${1:-}"

root_version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root_manifest" | head -n 1)
api_version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$api_manifest" | head -n 1)

if [ -z "$root_version" ] || [ -z "$api_version" ]; then
    echo "Could not read crate versions from $root_manifest and $api_manifest"
    exit 1
fi

echo "Aligning release manifests for root version: $root_version, dlp-api version: $api_version"

tmp="$(mktemp)"

awk -v api_version="$api_version" '
    BEGIN {
        section = ""
    }
    /^\[/ {
        section = $0
    }
    {
        if (section == "[dependencies]" && $0 ~ /^dlp-api = \{ package = "magicblock-delegation-program-api"/) {
            $0 = "dlp-api = { package = \"magicblock-delegation-program-api\", version = \"" api_version "\", path = \"dlp-api\", default-features = false }"
        } else if (section == "[dev-dependencies]" && $0 ~ /^magicblock-delegation-program = \{/) {
            $0 = "magicblock-delegation-program = { path = \".\", features = [\"unit_test_config\"] }"
        } else if (section == "[dev-dependencies]" && $0 ~ /^dlp-api = \{ package = "magicblock-delegation-program-api"/) {
            $0 = "dlp-api = { package = \"magicblock-delegation-program-api\", version = \"" api_version "\", path = \"dlp-api\" }"
        }
        print
    }
' "$root_manifest" > "$tmp"

if cmp -s "$root_manifest" "$tmp"; then
    rm -f "$tmp"
    echo "Release manifests are already aligned"
    exit 0
fi

if [ "$mode" = "--check" ]; then
    rm -f "$tmp"
    echo "Error: release manifests are not aligned. Run ./.github/version_align.sh and commit the result."
    exit 1
fi

mv "$tmp" "$root_manifest"
echo "Aligned $root_manifest"
