#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
product=${2:-app.msime.android}
case "$product" in
  app.msime.android) icons=HostIconPreparation; prepare=ClipboardEditPreparation; suite=ClipboardEditLifecycleDeviceSmoke ;;
  app.msime.android.pinyin) icons=PinyinHostIconPreparation; prepare=PinyinClipboardEditPreparation; suite=PinyinClipboardEditLifecycleDeviceSmoke ;;
  *) echo "Only full and pinyin test editions are supported" >&2; exit 1 ;;
esac
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || { echo "Test AVD has not booted" >&2; exit 1; }
active_instrumentation=$("$adb" -s "$serial" shell dumpsys activity processes)
if grep -q '^    Instrumentation #' <<< "$active_instrumentation"; then
  echo "Dedicated AVD is running another instrumentation; wait for it to finish" >&2
  exit 1
fi
bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk >/dev/null
icon_prepared=false
clipboard_prepared=false
check() {
  local marker=$1 result
  shift
  result=$("$adb" -s "$serial" shell am instrument -w "$@")
  printf '%s\n' "$result"
  [[ "$result" == *"$marker"* ]]
}
cleanup() {
  local failed=0
  "$adb" -s "$serial" shell am force-stop "$product" || failed=1
  if [[ "$clipboard_prepared" == true ]]; then
    check MSIME_CLIPBOARD_PREPARATION_PASSED -e restore true "app.msime.android.test/app.msime.android.test.$prepare" || failed=1
  fi
  if [[ "$icon_prepared" == true ]]; then
    check MSIME_ICON_PREPARATION_PASSED -e restore true "app.msime.android.test/app.msime.android.test.$icons" || failed=1
  fi
  return "$failed"
}
trap cleanup EXIT
check MSIME_ICON_PREPARATION_PASSED "app.msime.android.test/app.msime.android.test.$icons"
icon_prepared=true
for scenario in detach activity failure; do
  echo "Clipboard save lifecycle scenario: $scenario"
  clipboard_prepared=true
  check MSIME_CLIPBOARD_PREPARATION_PASSED "app.msime.android.test/app.msime.android.test.$prepare"
  "$adb" -s "$serial" shell am force-stop "$product"
  check MSIME_DEVICE_SMOKE_PASSED -e scenario "$scenario" "app.msime.android.test/app.msime.android.test.$suite"
  "$adb" -s "$serial" shell am force-stop "$product"
  check MSIME_CLIPBOARD_PREPARATION_PASSED -e inspect true "app.msime.android.test/app.msime.android.test.$prepare"
  check MSIME_CLIPBOARD_PREPARATION_PASSED -e restore true "app.msime.android.test/app.msime.android.test.$prepare"
  clipboard_prepared=false
done
cleanup
trap - EXIT
echo "Dedicated Android AVD: clipboard save survives view recreation and preserves entry metadata"
