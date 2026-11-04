#!/usr/bin/env bash
# 安装设备验收所需的固定 SDK 包；下载或解压损坏时有限重试并清掉残片。
set -euo pipefail

android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:?Set ANDROID_SDK_ROOT or ANDROID_HOME}}
api=${1:?Usage: install-sdk-packages.sh <api-level>}
case "$api" in
  28|35) ;;
  *) echo "Unsupported Android API level: $api" >&2; exit 2 ;;
esac

manager="$android_sdk/cmdline-tools/latest/bin/sdkmanager"
packages=(
  'build-tools;35.0.0'
  'platforms;android-35'
  'platforms;android-36'
  emulator
  platform-tools
  "system-images;android-${api};default;x86_64"
)

ready() {
  [[ -x "$android_sdk/emulator/emulator" ]] \
    && [[ -x "$android_sdk/platform-tools/adb" ]] \
    && [[ -f "$android_sdk/build-tools/35.0.0/package.xml" ]] \
    && [[ -f "$android_sdk/platforms/android-35/package.xml" ]] \
    && [[ -f "$android_sdk/platforms/android-36/package.xml" ]] \
    && [[ -f "$android_sdk/system-images/android-${api}/default/x86_64/package.xml" ]]
}

if ready; then exit 0; fi

for attempt in 1 2 3; do
  if [[ "$attempt" != 1 ]]; then
    # sdkmanager 可能留下损坏归档、完成标记和半截目标目录；不清掉这些状态时，
    # 下一次会复用坏文件并再次报 Error on ZipFile unknown archive。
    rm -rf \
      "$android_sdk/.temp" \
      "$android_sdk/.downloadIntermediates" \
      "$android_sdk/emulator" \
      "$android_sdk/platform-tools" \
      "$android_sdk/build-tools/35.0.0" \
      "$android_sdk/platforms/android-35" \
      "$android_sdk/platforms/android-36" \
      "$android_sdk/system-images/android-${api}/default/x86_64"
  fi
  echo "Install Android SDK packages for API $api (attempt $attempt/3)"
  # sdkmanager 退出时 yes 会收到 SIGPIPE；只忽略 yes 的退出码，保留 sdkmanager 的结果。
  if { yes || true; } | "$manager" --install "${packages[@]}" >/dev/null; then
    if ready; then exit 0; fi
  fi
  if [[ "$attempt" != 3 ]]; then sleep 5; fi
done

echo "Android SDK packages for API $api are incomplete after three attempts" >&2
exit 1
