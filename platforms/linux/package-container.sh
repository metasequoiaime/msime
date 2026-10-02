#!/usr/bin/env bash
# Build the installable Linux release in a container: a Release host library, the Tauri desktop binary, the native hosts configured for /usr with packaging on, and the CPack .deb and .tar.gz with a SHA256SUMS beside them.
#
# Separate from build-container.sh on purpose. That script is the pre-merge compile-and-unit-test gate and builds Debug against the debug host library; a release must not ship that tree, and the gate must not grow a Tauri build and a packaging step it does not need.
#
# Usage: platforms/linux/package-container.sh [VERSION]
#   VERSION defaults to platforms/linux/version.txt, the version release-linux.yml tags as linux-vVERSION. It becomes both the package version and the version the desktop binary reports, so the in-app update check compares like with like.
#   MSIME_PACKAGE_DESKTOP=0 packages without the Tauri desktop binary (no settings window); the default requires it.
#   CARGO_BUILD_JOBS and CMAKE_BUILD_PARALLEL_LEVEL are passed through when set, to bound memory on a shared Docker VM.
#   MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 fails unless target/language-dictionaries holds both cantonese.db and zhuyin.db with their licences; the default packages whatever is there.
#   MSIME_PACKAGE_FORMAT=rpm builds the RPM in a Fedora container (tests/tools/Dockerfile.package-rpm) instead of the .deb and .tar.gz in the Debian one. It is a separate build, not a conversion: rpmbuild takes Requires from the libraries the binaries link, so they have to be linked against Fedora's (#2095).
#
# The desktop binary embeds the web frontend at compile time, so apps/desktop/dist must be built first (`pnpm install --frozen-lockfile && pnpm --filter @msime/desktop build`). It is built outside the container because the container has no Node toolchain and a bind-mounted node_modules would mix host and container binaries.
#
# Output: target/linux-package/dist/{*.deb,*.tar.gz,SHA256SUMS}, or target/linux-package-rpm/dist/{*.rpm,SHA256SUMS} for the RPM.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

command -v docker >/dev/null 2>&1 || {
  echo "docker is required; run this on a Linux host with docker instead" >&2
  exit 2
}

version="${1:-$(tr -d '[:space:]' < platforms/linux/version.txt)}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "version must be MAJOR.MINOR.PATCH: $version" >&2
  exit 2
}
desktop="${MSIME_PACKAGE_DESKTOP:-1}"
format="${MSIME_PACKAGE_FORMAT:-deb}"
case "$format" in
  deb) build_root="$repo_root/target/linux-package" ;;
  rpm) build_root="$repo_root/target/linux-package-rpm" ;;
  *) echo "MSIME_PACKAGE_FORMAT must be deb or rpm: $format" >&2; exit 2 ;;
esac
if [ "$desktop" = 1 ] && [ ! -f apps/desktop/dist/index.html ]; then
  echo "apps/desktop/dist is missing; run 'pnpm install --frozen-lockfile && pnpm --filter @msime/desktop build' first, or set MSIME_PACKAGE_DESKTOP=0" >&2
  exit 2
fi

mkdir -p "$build_root"
# Third-party notices of the statically linked Rust crates and the bundled npm packages. The npm walk runs here because the container has no node_modules of its own; the crate walk runs in the container after the builds that resolve those crates. Cleared first so a desktop-less run cannot pick up an earlier frontend notice.
rm -rf "$build_root/notices"
mkdir -p "$build_root/notices"
if [ "$desktop" = 1 ]; then
  python3 platforms/linux/collect-notices.py npm "$build_root/notices/frontend-npm-NOTICES.txt" apps/desktop
fi

# Per-checkout tags for the same reason as the gate: parallel worktrees must not run each other's images.
checkout_hash="$(printf %s "$repo_root" | shasum | cut -c1-12)"
if [ "$format" = rpm ]; then
  package_image="msime-linux-package-rpm:$checkout_hash"
  docker build -q -t "$package_image" \
    -f platforms/linux/tests/tools/Dockerfile.package-rpm platforms/linux/tests >/dev/null
else
  gate_image="msime-linux-build-gate:$checkout_hash"
  package_image="msime-linux-package:$checkout_hash"
  docker build -q -t "$gate_image" \
    -f platforms/linux/tests/tools/Dockerfile.build-gate platforms/linux/tests >/dev/null
  docker build -q -t "$package_image" --build-arg MSIME_BUILD_GATE_IMAGE="$gate_image" \
    -f platforms/linux/tests/tools/Dockerfile.package platforms/linux/tests >/dev/null
fi
echo "package image: $package_image" >&2

docker run --rm --init \
  -v "$repo_root":/source \
  -v "$build_root":/build \
  -w /source \
  -e CARGO_TARGET_DIR=/build/cargo \
  -e MSIME_VERSION="$version" \
  -e MSIME_PACKAGE_DESKTOP="$desktop" \
  -e MSIME_PACKAGE_FORMAT="$format" \
  -e MSIME_REQUIRE_LANGUAGE_DICTIONARIES="${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS"} \
  ${CMAKE_BUILD_PARALLEL_LEVEL:+-e CMAKE_BUILD_PARALLEL_LEVEL="$CMAKE_BUILD_PARALLEL_LEVEL"} \
  "$package_image" bash -euo pipefail -c '
    cargo build --release --locked -p msime-host-api
    cargo build --release --locked -p msime-mcp-server --bin msime-mcp
    desktop_args=()
    crate_roots=(msime-host-api msime-mcp-server)
    if [ "$MSIME_PACKAGE_DESKTOP" = 1 ]; then
      # tauri/custom-protocol is what `tauri build` enables: without it the binary is a dev build that loads devUrl instead of the embedded frontend. TAURI_CONFIG sets the version the app reports, as Build-Client.ps1 does for Windows.
      TAURI_CONFIG="{\"version\":\"$MSIME_VERSION\"}" \
        cargo build --release --locked -p msime-desktop --bin msime-desktop --features tauri/custom-protocol
      desktop_args=(-DMSIME_DESKTOP_BINARY=/build/cargo/release/msime-desktop -DMSIME_FRONTEND_NOTICES=/build/notices/frontend-npm-NOTICES.txt)
      crate_roots+=(msime-desktop:tauri/custom-protocol)
    fi
    python3 platforms/linux/collect-notices.py cargo /build/notices/rust-crates-NOTICES.txt "${crate_roots[@]}"
    # The sherpa-onnx runtime msime-voice-local loads for on-device recognition, pinned by resources/voice-runtime.lock.json for the architecture of this container.
    case "$(uname -m)" in
      x86_64) voice_platform=linux-x86_64 ;;
      aarch64) voice_platform=linux-aarch64 ;;
      *) echo "no pinned voice runtime for $(uname -m)" >&2; exit 2 ;;
    esac
    python3 scripts/fetch_voice_runtime.py --platform "$voice_platform" --out /build/voice-runtime
    # The zinnia model msime-linux-handwriting --local recognises with, pinned by resources/handwriting-model.lock.json.
    python3 scripts/fetch_handwriting_model.py --out /build/handwriting-model
    # Non-English candidate glosses from scripts/fetch_offline_glosses.py, installed only when the databases and their NOTICE are both there; without them the package glosses offline in English only.
    glosses_args=()
    if compgen -G "target/offline-glosses/zh-*.db" >/dev/null && [ -f target/offline-glosses/offline-glosses-NOTICE.txt ]; then
      glosses_args=(-DMSIME_OFFLINE_GLOSSES=/source/target/offline-glosses)
    fi
    # The Cantonese and Zhuyin dictionaries from scripts/fetch_language_dictionaries.py, each installed with its licence text; without them the package offers both schemes as unavailable. MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 fails the package instead.
    languages_args=(-DMSIME_REQUIRE_LANGUAGE_DICTIONARIES="$([ "$MSIME_REQUIRE_LANGUAGE_DICTIONARIES" = 1 ] && echo ON || echo OFF)")
    if [ -d target/language-dictionaries ]; then
      languages_args+=(-DMSIME_LANGUAGE_DICTIONARIES=/source/target/language-dictionaries)
    fi
    # Configure from scratch every time: a cached MSIME_DESKTOP_BINARY or version from an earlier run must not leak into this package.
    rm -rf /build/cmake /build/dist
    cmake -S platforms/linux -B /build/cmake -G Ninja \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_INSTALL_PREFIX=/usr \
      -DMSIME_ENABLE_PACKAGING=ON \
      -DMSIME_ENABLE_FCITX5=ON \
      -DMSIME_HOST_LIBRARY=/build/cargo/release/libmsime_host_api.so \
      -DMSIME_MCP_BINARY=/build/cargo/release/msime-mcp \
      -DMSIME_PACKAGE_VERSION="$MSIME_VERSION" \
      -DMSIME_RUST_NOTICES=/build/notices/rust-crates-NOTICES.txt \
      -DMSIME_VOICE_RUNTIME_DIR=/build/voice-runtime \
      -DMSIME_HANDWRITING_MODEL_DIR=/build/handwriting-model \
      "${desktop_args[@]}" "${glosses_args[@]}" "${languages_args[@]}"
    cmake --build /build/cmake
    ctest --test-dir /build/cmake --output-on-failure
    if [ "$MSIME_PACKAGE_FORMAT" = rpm ]; then
      cpack --config /build/cmake/CPackConfig.cmake -G RPM -B /build/dist
    else
      cpack --config /build/cmake/CPackConfig.cmake -G "TGZ;DEB" -B /build/dist
    fi
    rm -rf /build/dist/_CPack_Packages
    cd /build/dist
    if [ "$MSIME_PACKAGE_FORMAT" = rpm ]; then
      sha256sum -- *.rpm > SHA256SUMS
    else
      sha256sum -- *.deb *.tar.gz > SHA256SUMS
    fi
    cat SHA256SUMS
  '
