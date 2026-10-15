#!/usr/bin/env bash
# 真实系统选择器授权一次，强停产品后再次导出，证明授权跨进程保留。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:?usage: diagnostics-export-directory.sh <dedicated-emulator>}
[[ "$serial" == emulator-* ]] || { echo "Dedicated emulator required" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] \
  || { echo "Refusing a non-test AVD" >&2; exit 1; }
original_ime=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
restore_ime() {
  # 选择器是独立任务，结束产品 Activity 后仍可能露出上一轮失败时留下的页面。
  "$adb" -s "$serial" shell am force-stop com.android.documentsui >/dev/null 2>&1 || true
  if [[ "$original_ime" =~ ^[a-zA-Z0-9_.]+/[a-zA-Z0-9_.]+$ ]]; then
    "$adb" -s "$serial" shell ime enable "$original_ime" >/dev/null 2>&1 || true
    "$adb" -s "$serial" shell ime set "$original_ime" >/dev/null 2>&1 || true
  fi
}
trap restore_ime EXIT
# 清理本套件失败时独立留在前台的系统选择器，避免挡住下次原生页启动。
"$adb" -s "$serial" shell am force-stop com.android.documentsui
# 此包仅含合成编辑器与文档提供方，清掉上一轮留下的合成文件，不清产品数据。
"$adb" -s "$serial" shell pm clear app.msime.android.test >/dev/null
for phase in select verify; do
  "$adb" -s "$serial" shell am force-stop app.msime.android
  result=$("$adb" -s "$serial" shell am instrument -w -r -e phase "$phase" \
    app.msime.android.test/app.msime.android.test.DiagnosticsExportDirectoryDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] \
    || { echo "Diagnostic directory $phase acceptance failed" >&2; exit 1; }
done
