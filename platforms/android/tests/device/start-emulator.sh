#!/usr/bin/env bash
# 启动专用测试 AVD。参数是 API 级别，默认 35（targetSdk）；28 是 minSdk（Android 9）。系统镜像的 ABI 跟主机一致：Apple 芯片用 arm64-v8a，x86_64 主机（CI 的 Linux runner，靠 KVM 加速）用 x86_64。每个级别一台独立 AVD、固定端口：35 是 msime-client-test@emulator-5580（沿用原名），其他级别是 msime-client-test-api<级别>@emulator-<5580 - 2 × (35 - 级别)>，例如 28 在 emulator-5566。打印端口供 smoke.sh 和 run-core-test.sh 使用。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
api=${1:-35}
[[ "$api" =~ ^(28|29|30|31|32|33|34|35)$ ]] || { echo "API level must be 28 to 35, not $api" >&2; exit 1; }
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
case $(uname -m) in
  arm64|aarch64) abi=arm64-v8a ;;
  *) abi=x86_64 ;;
esac
image="system-images;android-$api;default;$abi"
[[ -f "$android_sdk/system-images/android-$api/default/$abi/package.xml" ]] || {
  echo "Install $image in the selected SDK first" >&2; exit 1;
}
if [[ "$api" == 35 ]]; then name=msime-client-test; else name="msime-client-test-api$api"; fi
port=$((5580 - 2 * (35 - api)))
serial="emulator-$port"
existing=$("$android_sdk/platform-tools/adb" -s "$serial" emu avd name 2>/dev/null | tr -d '\r' | head -1 || true)
if [[ -n "$existing" ]]; then
  [[ "$existing" == "$name" ]] || { echo "Port $port belongs to another AVD" >&2; exit 1; }
  echo "Dedicated AVD $name is already live at $serial"; exit 0
fi
avd_home="$repo_root/target/android/avd-home"
mkdir -p "$avd_home"
if [[ ! -f "$avd_home/$name.ini" ]]; then
  # avdmanager canonicalizes toolsdir; a Homebrew symlink otherwise selects the
  # Homebrew prefix instead of the SDK containing the downloaded image.
  tools_marker="$android_sdk/cmdline-tools/msime-avd-test"
  mkdir -p "$tools_marker"
  printf 'no\n' | ANDROID_AVD_HOME="$avd_home" \
    AVDMANAGER_OPTS="-Dcom.android.sdkmanager.toolsdir=\"$tools_marker\"" \
    "$android_sdk/cmdline-tools/latest/bin/avdmanager" create avd --name "$name" \
      --package "$image" --path "$avd_home/$name.avd" --device pixel_6
fi
echo "Starting $name at $serial" >&2
exec env ANDROID_HOME="$android_sdk" ANDROID_AVD_HOME="$avd_home" \
  "$android_sdk/emulator/emulator" -avd "$name" -port "$port" \
  -no-window -no-audio -no-snapshot -no-metrics -gpu swiftshader -memory 2560
