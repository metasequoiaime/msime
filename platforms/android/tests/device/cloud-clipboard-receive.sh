#!/usr/bin/env bash
# 合成页面只在专用模拟器上进入产品接收方法，不读真实云端。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
[[ "$serial" == emulator-* ]] || { echo 'Only a dedicated emulator is supported' >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -n 1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || exit 1
"$adb" -s "$serial" install --no-incremental -r target/android/msime-android.apk
MSIME_ANDROID_CLOUD_CLIPBOARD_RECEIVE=1 bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
previous=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
restore() {
  local result=$?
  trap - EXIT
  if [[ "$previous" != null && -n "$previous" ]]; then
    "$adb" -s "$serial" shell ime set "$previous" >/dev/null || result=1
  fi
  exit "$result"
}
trap restore EXIT
"$adb" -s "$serial" shell ime enable app.msime.android/.MSIMEInputService >/dev/null
"$adb" -s "$serial" shell ime set app.msime.android/.MSIMEInputService >/dev/null
result=$("$adb" -s "$serial" shell am instrument -w \
  app.msime.android.test/app.msime.android.test.CloudClipboardReceiveDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* && "$result" != *MSIME_DEVICE_SMOKE_FAILED* ]]
