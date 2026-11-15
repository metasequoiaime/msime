#!/usr/bin/env bash
# #10968：在专用模拟器复现报告者的分辨率、密度和大字体；无论成功失败都恢复原设置。
set -euo pipefail
cd "$(dirname "$0")/../../../.."
adb="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}/platform-tools/adb"
serial=${1:-emulator-5580}
[[ "$serial" == emulator-* ]] || exit 1
avd=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd" == msime-client-test || "$avd" == msime-client-test-api* ]] || exit 1
size=$("$adb" -s "$serial" shell wm size | tr -d '\r' | sed -n 's/^Override size: //p')
density=$("$adb" -s "$serial" shell wm density | tr -d '\r' | sed -n 's/^Override density: //p')
font=$("$adb" -s "$serial" shell settings get system font_scale | tr -d '\r')
[[ -z "$size" || "$size" =~ ^[0-9]+x[0-9]+$ ]] || exit 1
[[ -z "$density" || "$density" =~ ^[0-9]+$ ]] || exit 1
[[ "$font" == null || "$font" =~ ^[0-9]+([.][0-9]+)?$ ]] || exit 1
restore() {
  local status=$?
  trap - EXIT
  set +e
  "$adb" -s "$serial" shell wm size "${size:-reset}" || status=1
  "$adb" -s "$serial" shell wm density "${density:-reset}" || status=1
  if [[ "$font" == null ]]; then
    "$adb" -s "$serial" shell settings delete system font_scale || status=1
  else
    "$adb" -s "$serial" shell settings put system font_scale "$font" || status=1
  fi
  exit "$status"
}
trap restore EXIT
"$adb" -s "$serial" shell wm size 1080x2376
"$adb" -s "$serial" shell wm density 480
"$adb" -s "$serial" shell settings put system font_scale 1.25
shot_root=/sdcard/Android/data/app.msime.android/files
for mode in portrait left right floating landscape; do
  "$adb" -s "$serial" shell rm -f "$shot_root/text-trackpad-$mode.png"
done
result=$("$adb" -s "$serial" shell am instrument -w -e layout-only true -e reported-display true \
  app.msime.android.test/app.msime.android.test.TextEditRecentClipboardDeviceSmoke)
printf '%s\n' "$result"
mkdir -p target/android/trackpad-layout
# 失败时也保留已取得的真实界面，未跑到的画面不能伪造为已验收。
for mode in portrait left right floating landscape; do
  "$adb" -s "$serial" pull "$shot_root/text-trackpad-$mode.png" "target/android/trackpad-layout/$mode.png" || true
done
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || exit 1
echo MSIME_TRACKPAD_LAYOUT_PASSED
