#!/usr/bin/env bash
# 验证默认 Download 与真实文件夹授权，重启后再导出，并覆盖存储失败。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:?usage: clipboard-export.sh <dedicated-emulator>}
[[ "$serial" == emulator-* ]] || { echo "Dedicated emulator required" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] \
  || { echo "Refusing a non-test AVD" >&2; exit 1; }
original_ime=$("$adb" -s "$serial" shell settings get secure default_input_method | tr -d '\r')
# 套件在中途设置合成锁屏 PIN 走系统认证（DeviceCredentialFixture.PIN），失败退出时也要清掉，否则后面的套件都会遇到锁屏。没有凭据时 `verify` 不经 Gatekeeper 直接成功；只在它失败、也就是确有凭据时才用合成 PIN 清除。无凭据时也去 `clear --old` 会被 Android 9 记作一次输错，累积几次后连 `set-pin` 都被限速拒绝。
clear_credential() {
  if ! "$adb" -s "$serial" shell locksettings verify 2>/dev/null | grep -q "verified successfully"; then
    "$adb" -s "$serial" shell locksettings clear --old 2580 >/dev/null 2>&1 || true
  fi
}
restore_ime() {
  clear_credential
  # 选择器是独立任务，结束产品 Activity 后仍可能露出上一轮失败时留下的页面。
  "$adb" -s "$serial" shell am force-stop com.android.documentsui >/dev/null 2>&1 || true
  # Android 9、10 的锁屏凭据确认是设置应用里的独立 Activity；套件中途失败时它会留在前台挡住后面的套件。
  "$adb" -s "$serial" shell am force-stop com.android.settings >/dev/null 2>&1 || true
  if [[ "$original_ime" =~ ^[a-zA-Z0-9_.]+/[a-zA-Z0-9_.]+$ ]]; then
    "$adb" -s "$serial" shell ime enable "$original_ime" >/dev/null 2>&1 || true
    "$adb" -s "$serial" shell ime set "$original_ime" >/dev/null 2>&1 || true
  fi
}
trap restore_ime EXIT
# 上一轮被强行中断时可能留下合成 PIN；无凭据路径要从真正没有锁屏的状态开始。
clear_credential
# 清理本套件失败时独立留在前台的系统选择器，避免挡住下次原生页启动。
"$adb" -s "$serial" shell am force-stop com.android.documentsui
# 此包仅含合成编辑器与文档提供方，清掉上一轮留下的合成文件，不清产品数据。
"$adb" -s "$serial" shell pm clear app.msime.android.test >/dev/null
for phase in select verify; do
  "$adb" -s "$serial" shell am force-stop app.msime.android
  result=$("$adb" -s "$serial" shell am instrument -w -r -e phase "$phase" \
    app.msime.android.test/app.msime.android.test.ClipboardExportDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] \
    || { echo "Clipboard export $phase acceptance failed" >&2; exit 1; }
done
