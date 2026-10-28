#!/usr/bin/env bash
# 下载偶发损坏时有限重试；以固定版本和编译器都已落盘为准，不发布残缺的 NDK。
set -euo pipefail
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:?Set ANDROID_SDK_ROOT or ANDROID_HOME}}
version=28.2.13676358
ndk="$android_sdk/ndk/$version"
case $(uname -s) in
  Darwin) host=darwin-x86_64 ;;
  Linux) host=linux-x86_64 ;;
  *) echo "Unsupported NDK host" >&2; exit 1 ;;
esac
ready() {
  grep -qs '^Pkg.Revision = 28\.2\.13676358$' "$ndk/source.properties" \
    && [[ -x "$ndk/toolchains/llvm/prebuilt/$host/bin/clang" ]]
}
if ready; then exit 0; fi
for attempt in 1 2 3; do
  if [[ "$attempt" != 1 ]]; then
    # 走到这里说明上一次没装成（例如 2026-10-27 夜间的「Error on ZipFile unknown archive」）。sdkmanager 的 .temp 里可能留着标记为准备完成的操作，.downloadIntermediates 里可能留着坏掉的压缩包，没装完的 NDK 目录也可能让它以为已经装好而跳过；重来之前全部清掉，迫使它重新下载和解压。
    rm -rf "$android_sdk/.temp" "$android_sdk/.downloadIntermediates" "$ndk"
  fi
  echo "Install Android NDK $version (attempt $attempt/3)"
  # sdkmanager 退出后 yes 会收到 SIGPIPE；只忽略 yes 的退出码，仍检查安装结果。
  if { yes || true; } | "$android_sdk/cmdline-tools/latest/bin/sdkmanager" --install "ndk;$version"; then
    if ready; then exit 0; fi
  fi
  if [[ "$attempt" != 3 ]]; then sleep 5; fi
done
echo "Android NDK $version is incomplete after three attempts" >&2
exit 1
