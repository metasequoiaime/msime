#!/usr/bin/env bash
# 安装当前工作树的宿主包，仅在专用模拟器上替换识别服务，结束时恢复。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
mode=${2:-lifecycle}
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || exit 1
previous=$("$adb" -s "$serial" shell settings get secure voice_recognition_service | tr -d '\r')
# 只保留应用包中这一项权限的布尔状态，不打印或保存 dumpsys 的其他内容。
record_audio_granted() {
  "$adb" -s "$serial" shell dumpsys package "$1" | \
    python3 -c 'import sys; print("true" if "android.permission.RECORD_AUDIO: granted=true" in sys.stdin.read() else "false")'
}
host_audio=$(record_audio_granted app.msime.android)
fixture_audio=$(record_audio_granted app.msime.android.test)
fixture_ready=false
restore() {
  local status=$?
  trap - EXIT
  set +e
  # 清理失败也继续恢复系统设置和授权；清理本身失败不能把本次测试报告成通过。
  if [[ "$fixture_ready" == true ]]; then
    cleanup=$("$adb" -s "$serial" shell am instrument -w -e enabled false \
      app.msime.android.test/app.msime.android.test.SyntheticSpeechSetup)
    printf '%s\n' "$cleanup"
    [[ "$cleanup" == *MSIME_SPEECH_SETUP_PASSED* ]] || status=1
  fi
  if [[ "$previous" == null ]]; then
    "$adb" -s "$serial" shell settings delete secure voice_recognition_service >/dev/null || status=1
  else
    "$adb" -s "$serial" shell settings put secure voice_recognition_service "'$previous'" || status=1
  fi
  if [[ "$host_audio" != true ]]; then
    "$adb" -s "$serial" shell pm revoke app.msime.android android.permission.RECORD_AUDIO || status=1
  fi
  if [[ "$fixture_audio" != true && "$fixture_ready" == true ]]; then
    "$adb" -s "$serial" shell pm revoke app.msime.android.test android.permission.RECORD_AUDIO || status=1
  fi
  exit "$status"
}
trap restore EXIT
"$adb" -s "$serial" install --no-incremental -r target/android/msime-android.apk
bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
fixture_ready=true
setup=$("$adb" -s "$serial" shell am instrument -w -e enabled true -e hold "$([[ "$mode" == hold ]] && echo true || echo false)" \
  app.msime.android.test/app.msime.android.test.SyntheticSpeechSetup)
[[ "$setup" == *MSIME_SPEECH_SETUP_PASSED* ]]
"$adb" -s "$serial" shell pm grant app.msime.android android.permission.RECORD_AUDIO
"$adb" -s "$serial" shell pm grant app.msime.android.test android.permission.RECORD_AUDIO
service=app.msime.android.test.SyntheticSpeechService
api=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
if (( api < 31 )); then service=app.msime.android.test.LegacySyntheticSpeechService; fi
"$adb" -s "$serial" shell settings put secure voice_recognition_service "app.msime.android.test/$service"
run_case() {
  local scenario=$1
  local result
  result=$("$adb" -s "$serial" shell am instrument -w -e scenario "$scenario" \
    app.msime.android.test/app.msime.android.test.VoiceRecognitionLifecycleDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *"MSIME_VOICE_LIFECYCLE_PASSED: $scenario"* ]]
}
if [[ "$mode" == hold ]]; then
  # smoke.sh 只走 hold 模式；人物检查不需要合成语音或麦克风，放在这里才会真的执行。
  result=$("$adb" -s "$serial" shell am instrument -w -e scenario voice-mascot \
    app.msime.android.test/app.msime.android.test.VoiceRecognitionLifecycleDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]]
  result=$("$adb" -s "$serial" shell am instrument -w -e voice-hold true \
    app.msime.android.test/app.msime.android.test.VoiceProviderCatalogDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]]
  exit
fi
run_case result
run_case ready-error
run_case stop-before-resume
if (( api < 31 )); then
  # 旧系统由调用应用直接绑定；选择只允许系统绑定的入口，制造真正的同步 SecurityException。
  "$adb" -s "$serial" shell settings put secure voice_recognition_service \
    app.msime.android.test/app.msime.android.test.SyntheticSpeechService
  run_case refused-start
fi
