#!/usr/bin/env bash
# 复用原生手写设备用例的真实触摸桥；仅操作专用合成 AVD。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:?usage: handwriting-smoke.sh <dedicated-emulator-serial> [--ocr-coexistence]}
coexistence=false
if [[ "${2:-}" == --ocr-coexistence ]]; then coexistence=true
elif [[ -n "${2:-}" ]]; then echo 'Unknown handwriting option' >&2; exit 1
fi
[[ "$serial" == emulator-* ]] || { echo 'Dedicated AVD required' >&2; exit 1; }
avd=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd" == msime-client-test || "$avd" == msime-client-test-api* ]] || { echo 'Unexpected AVD' >&2; exit 1; }
mkdir -p target/android/device-test
touch_request=/data/user/0/app.msime.android.test/cache/msime-handwriting-touch.request
touch_ack=/data/user/0/app.msime.android.test/cache/msime-handwriting-touch.ack
handwriting_output=""
instrumentation_pid=""
original_adbd_uid=$("$adb" -s "$serial" shell id -u | tr -d '\r')
cleanup_handwriting_bridge() {
  if [[ -n "$instrumentation_pid" ]] && kill -0 "$instrumentation_pid" 2>/dev/null; then
    "$adb" -s "$serial" shell am force-stop app.msime.android.test >/dev/null 2>&1 || true
    kill "$instrumentation_pid" 2>/dev/null || true
    wait "$instrumentation_pid" 2>/dev/null || true
  fi
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
if [[ "$coexistence" == true ]]; then
  for checkpoint in before result after failure; do
    "$adb" -s "$serial" shell rm -f "/data/local/tmp/msime-ocr-ime-handwriting-$checkpoint.png"
  done
fi
handwriting_output=$(mktemp "$repo_root/target/android/device-test/handwriting.XXXXXX")
"$adb" -s "$serial" shell am instrument -w -e ocr-coexistence "$coexistence" \
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
instrumentation_pid=""
result=$(<"$handwriting_output")
printf '%s\n' "$result"
cleanup_handwriting_bridge
trap - EXIT INT TERM
[[ "$result" == *MSIME_DEVICE_SMOKE_PASSED* && "$result" != *MSIME_DEVICE_SMOKE_FAILED* ]] || { echo "Handwriting acceptance failed" >&2; exit 1; }
