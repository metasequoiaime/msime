#!/usr/bin/env bash
# 仅在专用 AVD 验收拼音包的系统键盘入口；--fresh 显式清除该测试包数据，整个流程不打开设置应用。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
[[ $# -ge 2 ]] || {
  echo "usage: pinyin-startup.sh <emulator-serial> <pinyin-apk> [--fresh] [--hide-resume]" >&2; exit 1;
}
serial=$1
apk=$2
fresh=false
hide_resume=false
for option in "${@:3}"; do
  case "$option" in
    --fresh) fresh=true ;;
    --hide-resume) hide_resume=true ;;
    *) echo "Unknown pinyin startup option: $option" >&2; exit 1 ;;
  esac
done
[[ "$hide_resume" != true || "$fresh" == true ]] || {
  echo "--hide-resume requires --fresh" >&2; exit 1;
}
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
tools="$android_sdk/build-tools/35.0.0"
[[ "$serial" == emulator-* && -f "$apk" ]] || { echo "Expected an emulator and an APK" >&2; exit 1; }
avd=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd" == msime-client-test || "$avd" == msime-client-test-api* ]] || {
  echo "Refusing a non-test AVD" >&2; exit 1;
}
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || {
  echo "Test AVD has not booted" >&2; exit 1;
}
badging=$("$tools/aapt" dump badging "$apk")
[[ "$badging" == "package: name='app.msime.android.pinyin' "* ]] || {
  echo "Expected the native pinyin edition APK" >&2; exit 1;
}
bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r "$apk"
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
previous=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
restore() {
  # 只把之前读到的系统组件标识传回设备，不接受任意 shell 文本。
  if [[ "$previous" =~ ^[A-Za-z0-9_.]+/[A-Za-z0-9_.$]+$ ]]; then
    "$adb" -s "$serial" shell ime set "$previous" >/dev/null || true
  fi
}
trap restore EXIT
if [[ "$fresh" == true ]]; then
  result=$("$adb" -s "$serial" shell pm clear app.msime.android.pinyin | tr -d '\r')
  [[ "$result" == Success ]] || { echo "Could not reset the test pinyin package" >&2; exit 1; }
fi
methods=$("$adb" -s "$serial" shell ime list -a -s | tr -d '\r')
ime=$(printf '%s\n' "$methods" | awk '/^app\.msime\.android\.pinyin\// {print}')
[[ "$ime" == app.msime.android.pinyin/app.msime.android.MSIMEInputService ]] || {
  echo "Pinyin input method component was unavailable" >&2; exit 1;
}
"$adb" -s "$serial" shell ime enable "$ime"
"$adb" -s "$serial" shell ime set "$ime"
# 清数据时系统可能异步切回原键盘；启动编辑器之前确认选择连续稳定。
settled=0
for attempt in $(seq 1 60); do
  selected=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
  if [[ "$selected" == "$ime" ]]; then
    settled=$((settled + 1))
    (( settled < 4 )) || break
  else
    settled=0
    "$adb" -s "$serial" shell ime set "$ime" >/dev/null
  fi
  sleep 0.25
done
(( settled >= 4 )) || { echo "Pinyin input selection did not settle" >&2; exit 1; }
result=$("$adb" -s "$serial" shell timeout 90 am instrument -w -e pinyin-startup true -e pinyin-hide-resume "$hide_resume" \
  app.msime.android.test/app.msime.android.test.DeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Pinyin startup acceptance failed" >&2; exit 1; }
