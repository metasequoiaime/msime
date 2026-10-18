#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
backend_sources=$(find "$root/shared/backend" -type f -name '*.swift' ! -path '*/Tests/*' ! -name 'Package.swift' -print)
backend_ui_sources=$(find "$root/shared/backend-ui" -type f -name '*.swift' -print)
mac_sources=$(find "$root/platforms/macos/src/backend" -type f -name '*.swift' -print)
[ -n "$mac_sources" ] || { echo 'no macOS backend sources found' >&2; exit 1; }
swift_target=${MSIME_SWIFT_TARGET:-$(uname -m)-apple-macosx${MACOSX_DEPLOYMENT_TARGET:-13.0}}

exec xcrun swiftc -typecheck -parse-as-library -swift-version 5 -module-name MacBackendViews \
  -target "$swift_target" -I "$root/shared/backend/rust" \
  $backend_sources $backend_ui_sources $mac_sources
