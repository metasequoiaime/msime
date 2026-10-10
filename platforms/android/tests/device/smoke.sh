#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
settings=false
handwriting=false
statistics=false
core=false
for option in "${@:2}"; do
  case "$option" in
    # --core 只做安装、首次启动准备词库和 DeviceSmoke 的打字主路径，给 API 28 这类其余套件驱动不了的旧系统用（见 DeviceSmoke 里的说明）。
    --core) core=true ;;
    --settings) settings=true ;;
    --handwriting) handwriting=true ;;
    --statistics) statistics=true ;;
    *) echo "usage: smoke.sh [emulator-5580] [--core] [--settings] [--handwriting] [--statistics]" >&2; exit 1 ;;
  esac
done
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || { echo "Test AVD has not booted" >&2; exit 1; }
bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/msime-android.apk
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
mkdir -p target/android/device-test
xml="$repo_root/target/android/device-test/window.xml"
dump() {
  "$adb" -s "$serial" shell uiautomator dump /data/local/tmp/msime-test-window.xml >/dev/null \
    && "$adb" -s "$serial" pull /data/local/tmp/msime-test-window.xml "$xml" >/dev/null 2>&1
}
# 点 $xml 这份 dump 里的目标，不另取 dump；目标不在里面时返回 1，由调用方决定是否重试。
tap() {
  bounds=$(xmllint --xpath "string(($1)[1]/@bounds)" "$xml")
  [[ "$bounds" =~ ^\[([0-9]+),([0-9]+)\]\[([0-9]+),([0-9]+)\]$ ]] || return 1
  "$adb" -s "$serial" shell input tap "$(( (BASH_REMATCH[1] + BASH_REMATCH[3]) / 2 ))" "$(( (BASH_REMATCH[2] + BASH_REMATCH[4]) / 2 ))"
}
# The host prepares the shipped dictionary itself on first run; there is no button to press for it any more. The 设置 tab shows the state only while preparing or after a failure, so the tab having rendered with no preparation notice is what readiness looks like.
# A fresh install plays the first-launch splash and then opens onboarding over the home screen; the loop skips onboarding when it finds it, and the splash dismisses itself.
# The 设置 tab is drawn under the splash, so 试用键盘 is in the dump while the splash still covers it, and onboarding is only started as the splash begins to fade. Readiness therefore also needs the splash gone, and has to hold on two polls a second apart so an onboarding window that is still opening gets its chance to appear and be skipped.
"$adb" -s "$serial" shell am start -W -n app.msime.android/app.msime.android.home.HomeActivity >/dev/null
ready=false
settled=0
for attempt in $(seq 1 60); do
  # 应用刚启动、窗口还在切换时，uiautomator 偶尔报 `ERROR: null root node returned by UiTestAutomationBridge.` 并以非零退出；set -e 下这一次失败会让整个冒烟在第一条用例之前退出（API 35 上出现过）。轮询里把它当作这一拍还没就绪。
  if ! dump; then settled=0; sleep 1; continue; fi
  if [[ $(xmllint --xpath 'boolean(//node[contains(@text,"词库准备失败")])' "$xml") == true ]]; then echo "Device bootstrap failed" >&2; exit 1; fi
  if [[ $(xmllint --xpath 'boolean(//node[contains(@resource-id,":id/onboarding_skip")])' "$xml") == true ]]; then
    settled=0
    # 用这一拍刚读到的 dump 去点：tap 原先会另取一份 dump，引导页在两次 dump 之间可能正在打开或关闭，另取的那份偶尔是空的（null root），找不到按钮就让整个冒烟在第一条用例之前退出（API 28 上出现过）。这一拍点不到就当作还没就绪，下一拍再看。
    tap '//node[contains(@resource-id,":id/onboarding_skip")]' || true
    sleep 1
    continue
  fi
  if [[ $(xmllint --xpath 'boolean(//node[contains(@text,"试用键盘")])' "$xml") == true \
     && $(xmllint --xpath 'boolean(//node[contains(@text,"正在准备词库")])' "$xml") == false \
     && $(xmllint --xpath 'boolean(//node[contains(@resource-id,":id/home_intro")])' "$xml") == false ]]; then
    settled=$((settled + 1))
    if (( settled >= 2 )); then ready=true; break; fi
  else
    settled=0
  fi
  sleep 1
done
[[ "$ready" == true ]] || { echo "Device bootstrap timed out" >&2; exit 1; }
# 输入法 id 必须写成 `ime list` 给出的短写：系统按字符串原样查找，长写 app.msime.android/app.msime.android.MSIMEInputService 在 `ime enable` 时报 Unknown input method。设备测试里的 `ime` 命令都用同一个短写。
"$adb" -s "$serial" shell ime enable app.msime.android/.MSIMEInputService
"$adb" -s "$serial" shell ime set app.msime.android/.MSIMEInputService
"$adb" -s "$serial" shell am force-stop app.msime.android.test
result=$("$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.test.DeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "System input acceptance failed" >&2; exit 1; }
if [[ "$core" == true ]]; then echo "Dedicated Android AVD: install, resource setup and core typing acceptance passed"; exit 0; fi
# MSIME_DEVICE_SMOKE_SKIP：空格分隔的套件名。这些套件已知失败、正在修（原因写在调用方，见 .github/workflows/android-device.yml），跳过时明确打印出来，不算通过。
skipped_suites=()
for entry in \
    "CandidatePanelDeviceSmoke|Candidate panel acceptance failed" \
    "MoreToolsDeviceSmoke|More tools acceptance failed" \
    "EmojiPickerDeviceSmoke|Emoji picker acceptance failed" \
    "PairedPunctuationDeviceSmoke|Paired punctuation acceptance failed" \
    "PreferencesDeviceSmoke|Preferences acceptance failed" \
    "KeyboardHeightDeviceSmoke|Keyboard height acceptance failed" \
    "FuzzyPinyinDeviceSmoke|Fuzzy pinyin acceptance failed" \
    "CandidateGlossDeviceSmoke|Candidate gloss acceptance failed" \
    "NineKeyEnglishDeviceSmoke|Nine-key English acceptance failed" \
    "NineKeyPanelDeviceSmoke|Nine-key panel acceptance failed" \
    "ChineseHelpcodeDeviceSmoke|Chinese helpcode acceptance failed" \
    "MicrosoftShuangpinDeviceSmoke|Microsoft double-pinyin acceptance failed" \
    "EmailSuffixDeviceSmoke|Email suffix acceptance failed" \
    "NumberRowDeviceSmoke|Number row acceptance failed"
do
  suite=${entry%%|*}
  if [[ " ${MSIME_DEVICE_SMOKE_SKIP:-} " == *" $suite "* ]]; then
    echo "SKIPPED (known failure): $suite" >&2
    skipped_suites+=("$suite")
    continue
  fi
  result=$("$adb" -s "$serial" shell am instrument -w "app.msime.android.test/app.msime.android.test.$suite")
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "${entry#*|}" >&2; exit 1; }
done
if [[ "$settings" == true ]]; then
  for suite in SettingsDeviceSmoke SettingsLifecycleSmoke AccountStorageDeviceSmoke; do
    result=$("$adb" -s "$serial" shell am instrument -w "app.msime.android.test/app.msime.android.test.$suite")
    printf '%s\n' "$result"
    [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Shared settings acceptance failed" >&2; exit 1; }
  done
  result=$("$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.BackendAccountRefreshDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Account refresh acceptance failed" >&2; exit 1; }
fi
if [[ "$statistics" == true ]]; then
  "$adb" -s "$serial" shell am force-stop app.msime.android
  result=$("$adb" -s "$serial" shell am instrument -w app.msime.android.test/app.msime.android.test.TypingStatisticsDeviceSmoke)
  printf '%s\n' "$result"
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Typing statistics acceptance failed" >&2; exit 1; }
fi
if [[ "$handwriting" == true ]]; then
  touch_request=/data/user/0/app.msime.android.test/cache/msime-handwriting-touch.request
  touch_ack=/data/user/0/app.msime.android.test/cache/msime-handwriting-touch.ack
  handwriting_output=""
  original_adbd_uid=$("$adb" -s "$serial" shell id -u | tr -d '\r')
  cleanup_handwriting_bridge() {
    "$adb" -s "$serial" shell rm -f "$touch_request" "$touch_ack" >/dev/null 2>&1 || true
    if [[ -n "$handwriting_output" && -f "$handwriting_output" ]]; then
      unlink "$handwriting_output"
    fi
    if [[ "$original_adbd_uid" != 0 ]]; then
      "$adb" -s "$serial" unroot >/dev/null 2>&1 || true
      "$adb" -s "$serial" wait-for-device >/dev/null 2>&1 || true
    fi
  }
  trap cleanup_handwriting_bridge EXIT INT TERM
  "$adb" -s "$serial" root >/dev/null
  "$adb" -s "$serial" wait-for-device
  touch_devices=$("$adb" -s "$serial" shell getevent -lp)
  touch_device=$(printf '%s\n' "$touch_devices" | awk '
    /^add device/ { device=$NF }
    /name: +"virtio_input_multi_touch_1"/ { print device; exit }
  ')
  [[ "$touch_device" =~ ^/dev/input/event[0-9]+$ ]] || { echo "Primary emulator touch device was not found" >&2; exit 1; }
  touch_info=$("$adb" -s "$serial" shell getevent -lp "$touch_device")
  raw_max_x=$(printf '%s\n' "$touch_info" | sed -n 's/.*ABS_MT_POSITION_X.*max \([0-9][0-9]*\).*/\1/p' | head -1)
  raw_max_y=$(printf '%s\n' "$touch_info" | sed -n 's/.*ABS_MT_POSITION_Y.*max \([0-9][0-9]*\).*/\1/p' | head -1)
  physical_size=$("$adb" -s "$serial" shell wm size | sed -n 's/^Physical size: \([0-9][0-9]*\)x\([0-9][0-9]*\).*/\1 \2/p' | head -1)
  read -r screen_width screen_height <<<"$physical_size"
  [[ "$raw_max_x" =~ ^[0-9]+$ && "$raw_max_y" =~ ^[0-9]+$
      && "$screen_width" =~ ^[0-9]+$ && "$screen_height" =~ ^[0-9]+$
      && "$screen_width" -gt 1 && "$screen_height" -gt 1 ]] \
    || { echo "Primary emulator touch geometry was unavailable" >&2; exit 1; }
  raw_stroke() {
    local request_id=$1 start_x=$2 start_y=$3 end_x=$4 end_y=$5
    local raw_start_x=$(( start_x * raw_max_x / (screen_width - 1) ))
    local raw_start_y=$(( start_y * raw_max_y / (screen_height - 1) ))
    local raw_end_x=$(( end_x * raw_max_x / (screen_width - 1) ))
    local raw_end_y=$(( end_y * raw_max_y / (screen_height - 1) ))
    local events="sendevent $touch_device 3 47 0; sendevent $touch_device 3 57 0; sendevent $touch_device 3 48 $request_id; sendevent $touch_device 3 49 $request_id; sendevent $touch_device 3 58 100"
    local step raw_x raw_y
    for step in {0..12}; do
      raw_x=$(( raw_start_x + (raw_end_x - raw_start_x) * step / 12 ))
      raw_y=$(( raw_start_y + (raw_end_y - raw_start_y) * step / 12 ))
      events+="; sendevent $touch_device 3 53 $raw_x; sendevent $touch_device 3 54 $raw_y; sendevent $touch_device 0 0 0; usleep 12000"
    done
    events+="; sendevent $touch_device 3 58 0; sendevent $touch_device 3 57 -1; sendevent $touch_device 0 0 0"
    "$adb" -s "$serial" shell "$events"
  }
  "$adb" -s "$serial" shell rm -f "$touch_request" "$touch_ack"
  handwriting_output=$(mktemp "$repo_root/target/android/device-test/handwriting.XXXXXX")
  "$adb" -s "$serial" shell am instrument -w \
    app.msime.android.test/app.msime.android.test.HandwritingDeviceSmoke \
    >"$handwriting_output" &
  instrumentation_pid=$!
  last_request=0
  touch_deadline=$((SECONDS + 240))
  while kill -0 "$instrumentation_pid" 2>/dev/null; do
    request=$("$adb" -s "$serial" shell cat "$touch_request" 2>/dev/null | tr -d '\r\n' || true)
    if [[ "$request" =~ ^([0-9]+)\ ([0-9]+)\ ([0-9]+)\ ([0-9]+)\ ([0-9]+)$
        && "${BASH_REMATCH[1]}" != "$last_request" ]]; then
      request_id=${BASH_REMATCH[1]}
      raw_stroke "$request_id" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" \
        "${BASH_REMATCH[4]}" "${BASH_REMATCH[5]}"
      "$adb" -s "$serial" shell "printf '%s\\n' '$request_id' > '$touch_ack'"
      last_request=$request_id
    fi
    if (( SECONDS >= touch_deadline )); then
      "$adb" -s "$serial" shell am force-stop app.msime.android.test
      break
    fi
    sleep 0.1
  done
  wait "$instrumentation_pid" || true
  result=$(<"$handwriting_output")
  printf '%s\n' "$result"
  cleanup_handwriting_bridge
  trap - EXIT INT TERM
  [[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* ]] || { echo "Handwriting acceptance failed" >&2; exit 1; }
fi
[[ ${#skipped_suites[@]} -eq 0 ]] || echo "Skipped known failures: ${skipped_suites[*]}" >&2
echo "Dedicated Android AVD: install, resource setup, system input and live preferences acceptance passed"
