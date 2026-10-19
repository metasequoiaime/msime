#!/usr/bin/env bash
set -euo pipefail
[[ ${MSIME_ISOLATED_LINUX_TEST:-} == 1 && -d /resources && -d /build ]] || exit 2

cmake -S platforms/linux -B /build/fcitx5 -G Ninja \
  -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so \
  -DMSIME_ENABLE_FCITX5=ON -DMSIME_ENABLE_PACKAGING=OFF \
  -DMSIME_FCITX5_TEST_RESOURCES=/resources
cmake --build /build/fcitx5 --target msime-fcitx5 fcitx5-native-test
ctest --test-dir /build/fcitx5 -R '^fcitx5-(native-context|native-ai|native-smart-abc|daemon-frontend|gtk-editor|gtk-canvas-horizontal|gtk-canvas-vertical|ibus-gtk-editor)$' \
  --output-on-failure
echo "Fcitx5 native addon, daemon, and GTK editor acceptance passed"
