#!/usr/bin/env bash
# 只在专用模拟器验收内联填充；中断或测试失败也恢复原填充服务。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || exit 1
previous=$("$adb" -s "$serial" shell settings get secure autofill_service | tr -d '\r')
[[ -z "$previous" || "$previous" =~ ^[A-Za-z0-9_./]+$ ]] || exit 1
restore() {
  local status=$?
  trap - EXIT
  if [[ "$previous" == null ]]; then
    "$adb" -s "$serial" shell settings delete secure autofill_service >/dev/null || status=1
  elif [[ -z "$previous" ]]; then
    "$adb" -s "$serial" shell settings put secure autofill_service "''" >/dev/null || status=1
  else
    "$adb" -s "$serial" shell settings put secure autofill_service "$previous" >/dev/null || status=1
  fi
  exit "$status"
}
trap restore EXIT
"$adb" -s "$serial" install --no-incremental -r target/android/msime-android.apk
MSIME_ANDROID_INLINE_AUTOFILL=1 bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
result=$("$adb" -s "$serial" shell am instrument -w \
  app.msime.android.test/app.msime.android.test.InlineAutofillDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]]
