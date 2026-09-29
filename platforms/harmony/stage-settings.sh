#!/usr/bin/env bash
# Builds the HarmonyOS settings page from the shared UI (packages/ui) into entry/src/main/resources/rawfile/settings/index.html. Run it before hvigorw assembleHap, alongside stage-resources.sh, build-native.sh and stage-voice-runtime.sh; entry/hvigorfile.ts refuses to package a HAP without it.
#
# The page is one self-contained HTML file because a resource:// document has a null origin and the WebView will not fetch a module script or a stylesheet across it. It is built here rather than committed: a committed 1 MB one-line artefact that every shared-UI change had to regenerate conflicted between any two such pull requests, and building it from the same checkout as the HAP means it can never be older than the UI it ships with.
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"
command -v pnpm >/dev/null 2>&1 || { echo "pnpm is required to build the settings page (corepack enable)" >&2; exit 1; }
[ -d node_modules ] || { echo "dependencies are not installed: run pnpm install --frozen-lockfile at the repository root" >&2; exit 1; }
pnpm --filter @msime/harmony build >/dev/null
page="$repo_root/platforms/harmony/entry/src/main/resources/rawfile/settings/index.html"
[ -s "$page" ] || { echo "the settings build produced no $page" >&2; exit 1; }
# The WebView reads this one file and nothing beside it.
extra=$(find "$(dirname "$page")" -mindepth 1 ! -name index.html | head -1)
[ -z "$extra" ] || { echo "the settings build left files beside index.html that the WebView cannot load: $extra" >&2; exit 1; }
echo "Staged for the HAP: $page"
