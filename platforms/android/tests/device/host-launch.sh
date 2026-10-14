#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
product=${2:-app.msime.android}
case "$product" in
  app.msime.android) preparation=HostIconPreparation ;;
  app.msime.android.pinyin) preparation=PinyinHostIconPreparation ;;
  *) echo "Only full and pinyin test editions are supported" >&2; exit 1 ;;
esac
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || { echo "Test AVD has not booted" >&2; exit 1; }
api=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk >/dev/null
original_ime=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
restore_latin=false
prepared=false
instrument="app.msime.android.test/app.msime.android.test.$preparation"
cleanup() {
  local failed=0 result
  if [[ "$prepared" == true ]]; then
    result=$("$adb" -s "$serial" shell am instrument -w -e restore true "$instrument")
    printf '%s\n' "$result"
    [[ "$result" == *MSIME_ICON_PREPARATION_PASSED* ]] || failed=1
  fi
  if [[ "$restore_latin" == true ]]; then "$adb" -s "$serial" shell pm enable com.android.inputmethod.latin >/dev/null || failed=1; fi
  if [[ "$original_ime" != null && -n "$original_ime" ]]; then
    "$adb" -s "$serial" shell ime enable "$original_ime" >/dev/null || failed=1
    "$adb" -s "$serial" shell ime set "$original_ime" >/dev/null || failed=1
  fi
  return "$failed"
}
trap cleanup EXIT
# API 28 的系统键盘会在进程重绑间隙抢回焦点；只在它原来启用时临时禁用，并在退出时恢复。
if [[ "$api" == 28 ]] && ! "$adb" -s "$serial" shell pm list packages -d | tr -d '\r' | grep -qx 'package:com.android.inputmethod.latin'; then
  "$adb" -s "$serial" shell pm disable-user --user 0 com.android.inputmethod.latin >/dev/null
  restore_latin=true
fi
result=$("$adb" -s "$serial" shell am instrument -w "$instrument")
printf '%s\n' "$result"
[[ "$result" == *MSIME_ICON_PREPARATION_PASSED* ]] || exit 1
prepared=true
for icon in CLASSIC FOREST SKY DUSK VERMILION; do
  result=$("$adb" -s "$serial" shell am instrument -w -e icon "$icon" "$instrument")
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_ICON_PREPARATION_PASSED* ]] || exit 1
  # 宿主 UID 的 instrumentation 必须已经结束；页面验收在外部 UID 运行，不能附着到产品以绕过启动限制。
  "$adb" -s "$serial" shell am force-stop "$product"
  result=$("$adb" -s "$serial" shell am instrument -w -e product "$product" -e style "$icon" \
    app.msime.android.test/app.msime.android.test.HostLaunchDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || exit 1
done
cleanup
trap - EXIT
echo "Dedicated Android AVD: all five icons retain one launcher and keyboard host routes survive delayed component changes"
