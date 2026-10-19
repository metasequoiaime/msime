#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:?usage: upstream-skins.sh emulator-serial upstream-checkout}
upstream=${2:?usage: upstream-skins.sh emulator-serial upstream-checkout}
[[ "$serial" == emulator-* ]] || { echo 'Only the dedicated emulator is supported' >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || {
  echo 'Refusing a non-test AVD' >&2; exit 1;
}
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || {
  echo 'Test AVD has not booted' >&2; exit 1;
}
[[ -n $("$adb" -s "$serial" shell pm path app.msime.android) ]] || {
  echo 'Install and prepare the native host APK first' >&2; exit 1;
}
MSIME_ANDROID_UPSTREAM_SKINS="$upstream" bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
run_id="upstream-$(date +%Y%m%d-%H%M%S)-$$"
output="target/android/device-test/$serial/$run_id"
mkdir -p "$output"
device_output="/sdcard/Android/data/app.msime.android/files/$run_id"
# 只清理本次创建的导出目录；偏好和临时安装的皮肤由 instrumentation 的 finally 恢复。
trap '"$adb" -s "$serial" shell rm -rf "$device_output" >/dev/null 2>&1 || true' EXIT
"$adb" -s "$serial" shell am instrument -w -e capture_run "$run_id" \
  app.msime.android.test/app.msime.android.test.CandidateUpstreamSkinDeviceSmoke | tee "$output/instrumentation.log"
"$adb" -s "$serial" pull "$device_output/." "$output/" >/dev/null
if ! grep -q 'MSIME_DEVICE_SMOKE_PASSED' "$output/instrumentation.log"; then
  echo 'Upstream skin acceptance failed; inspect the fixed-stage log and captured rasters' >&2
  exit 1
fi
count=$(find "$output" -maxdepth 1 -name '*.png' -type f | wc -l | tr -d ' ')
[[ "$count" == 8 ]] || { echo "Expected eight upstream rasters, got $count" >&2; exit 1; }
echo "Fixed upstream dd018f79858314ed542a47c528608752b13bd261: eight native IME captures in $output; inspect them visually"
