#!/usr/bin/env bash
# 在专用 AVD 上验收离线语音权限阶段的真实旋转，并检查 instrumentation 退出后的方向。
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || {
  echo "Refusing a non-test AVD: $avd_name" >&2
  exit 1
}

previous_service=$("$adb" -s "$serial" shell settings get secure voice_recognition_service | tr -d '\r')
previous_auto=$("$adb" -s "$serial" shell settings get system accelerometer_rotation | tr -d '\r')
previous_user=$("$adb" -s "$serial" shell settings get system user_rotation | tr -d '\r')
[[ "$previous_auto" =~ ^[0-9]+$ && "$previous_user" =~ ^[0-9]+$ ]] || {
  echo "Rotation settings are not numeric" >&2
  exit 1
}

record_audio_granted() {
  "$adb" -s "$serial" shell dumpsys package "$1" |
    python3 -c 'import sys; print("true" if "android.permission.RECORD_AUDIO: granted=true" in sys.stdin.read() else "false")'
}

host_audio=$(record_audio_granted app.msime.android)
fixture_audio=$(record_audio_granted app.msime.android.test)
fixture_ready=false

restore() {
  local status=$?
  trap - EXIT
  set +e
  if [[ "$fixture_ready" == true ]]; then
    cleanup=$("$adb" -s "$serial" shell am instrument -w -e enabled false \
      app.msime.android.test/app.msime.android.test.SyntheticSpeechSetup)
    printf '%s\n' "$cleanup"
    [[ "$cleanup" == *MSIME_SPEECH_SETUP_PASSED* ]] || status=1
  fi
  if [[ "$previous_service" == null ]]; then
    "$adb" -s "$serial" shell settings delete secure voice_recognition_service >/dev/null || status=1
  else
    "$adb" -s "$serial" shell settings put secure voice_recognition_service "$previous_service" >/dev/null || status=1
  fi
  "$adb" -s "$serial" shell settings put system user_rotation "$previous_user" >/dev/null || status=1
  "$adb" -s "$serial" shell settings put system accelerometer_rotation "$previous_auto" >/dev/null || status=1
  if [[ "$host_audio" == true ]]; then
    "$adb" -s "$serial" shell pm grant app.msime.android android.permission.RECORD_AUDIO >/dev/null || status=1
  else
    "$adb" -s "$serial" shell pm revoke app.msime.android android.permission.RECORD_AUDIO >/dev/null || status=1
  fi
  if [[ "$fixture_audio" == true ]]; then
    "$adb" -s "$serial" shell pm grant app.msime.android.test android.permission.RECORD_AUDIO >/dev/null || status=1
  else
    "$adb" -s "$serial" shell pm revoke app.msime.android.test android.permission.RECORD_AUDIO >/dev/null || status=1
  fi
  exit "$status"
}
trap restore EXIT

[[ -f target/android/msime-android.apk && -f target/android/editor-test.apk ]] || {
  echo "Build target/android/msime-android.apk and editor-test.apk first" >&2
  exit 1
}
"$adb" -s "$serial" install --no-incremental -r target/android/msime-android.apk >/dev/null
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk >/dev/null
setup=$("$adb" -s "$serial" shell am instrument -w -e enabled true \
  app.msime.android.test/app.msime.android.test.SyntheticSpeechSetup)
printf '%s\n' "$setup"
[[ "$setup" == *MSIME_SPEECH_SETUP_PASSED* ]] || exit 1
fixture_ready=true

"$adb" -s "$serial" shell pm revoke app.msime.android android.permission.RECORD_AUDIO >/dev/null || true
"$adb" -s "$serial" shell pm revoke app.msime.android.test android.permission.RECORD_AUDIO >/dev/null || true
api=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
service=app.msime.android.test.SyntheticSpeechService
if (( api < 31 )); then service=app.msime.android.test.LegacySyntheticSpeechService; fi
"$adb" -s "$serial" shell settings put secure voice_recognition_service \
  "app.msime.android.test/$service"

run_rotation_case() {
  # 先让被测进程看到固定横屏，覆盖 UiAutomation 连接时已是固定方向的场景。
  "$adb" -s "$serial" shell settings put system accelerometer_rotation 0
  "$adb" -s "$serial" shell settings put system user_rotation 3
  output=$("$adb" -s "$serial" shell am instrument -w -e scenario offline-rotation \
    app.msime.android.test/app.msime.android.test.VoiceRecognitionLifecycleDeviceSmoke)
  printf '%s\n' "$output"
  [[ "$output" == *"Offline voice boundary passed: offline-rotation"* ]] || return 1
  # am instrument 返回后 UiAutomation 才执行 disconnect；这里的读值是实际进程退出后的值。
  actual_auto=$("$adb" -s "$serial" shell settings get system accelerometer_rotation | tr -d '\r')
  actual_user=$("$adb" -s "$serial" shell settings get system user_rotation | tr -d '\r')
  [[ "$actual_auto" == 0 && "$actual_user" == 3 ]] || {
    echo "Instrumentation changed fixed landscape: auto=$actual_auto user=$actual_user" >&2
    return 1
  }
}

run_rotation_case
