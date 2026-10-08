#!/usr/bin/env bash
# Cargo runner for the complete client-core unit suite on the dedicated AVD.
set -euo pipefail
[[ $# == 1 && -f "$1" ]] || { echo "Expected one Cargo test binary; filters are not supported" >&2; exit 1; }
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
# MSIME_ANDROID_TEST_SERIAL 选另一个级别的专用 AVD（start-emulator.sh 打印的端口），默认是 API 35 的 emulator-5580。
serial=${MSIME_ANDROID_TEST_SERIAL:-emulator-5580}
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || { echo "Test AVD has not booted" >&2; exit 1; }
"$adb" -s "$serial" shell mkdir -p /data/local/tmp/msime-rust-tests
"$adb" -s "$serial" push "$1" /data/local/tmp/msime-rust-tests/core-tests >/dev/null
"$adb" -s "$serial" shell chmod 700 /data/local/tmp/msime-rust-tests/core-tests
"$adb" -s "$serial" shell 'TMPDIR=/data/local/tmp/msime-rust-tests /data/local/tmp/msime-rust-tests/core-tests'
