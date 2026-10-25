#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
[[ "$serial" == emulator-* ]] || { echo "Only a dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || {
  echo "Refusing a non-test AVD" >&2; exit 1;
}
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || {
  echo "Test AVD has not booted" >&2; exit 1;
}
active=$("$adb" -s "$serial" shell dumpsys activity processes)
if grep -q '^    Instrumentation #' <<< "$active"; then
  echo "Test AVD is running another instrumentation" >&2; exit 1
fi
MSIME_ANDROID_CLIPBOARD_CATEGORIES_SMOKE=1 bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk >/dev/null
original_ime=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
cleanup() {
  if [[ "$original_ime" != null && -n "$original_ime" ]]; then
    "$adb" -s "$serial" shell ime enable "$original_ime" >/dev/null
    "$adb" -s "$serial" shell ime set "$original_ime" >/dev/null
  fi
}
trap cleanup EXIT
result=$("$adb" -s "$serial" shell am instrument -w \
  app.msime.android.test/app.msime.android.test.ClipboardCategoriesDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]]
