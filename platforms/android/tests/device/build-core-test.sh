#!/usr/bin/env bash
# 把 msime-client-core 的单测交叉编译成专用 AVD 能直接运行的程序，最后一行打印它的路径，交给 run-core-test.sh。ABI 跟着 start-emulator.sh 选的系统镜像走：arm64 主机（Apple 芯片）是 arm64-v8a，x86_64 主机（CI 的 Linux runner）是 x86_64，也可以用参数指定。
#
# 这一步存在的原因：client-core 的私有文件读写在 Android 上受 /data 只给搜索权限、SELinux 禁止硬链接这些限制，macOS 和 Linux 上的单测一条都碰不到。0.3.0 就因此带着「词库准备失败」和匿名账号写入失败发了出去。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
case "${1:-$(uname -m)}" in
  arm64|aarch64|arm64-v8a) target=aarch64-linux-android ;;
  x86_64|amd64) target=x86_64-linux-android ;;
  *) echo "usage: build-core-test.sh [arm64-v8a|x86_64]" >&2; exit 1 ;;
esac
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
ndk="$android_sdk/ndk/28.2.13676358"
[[ -d "$ndk" ]] || { echo "Install ndk;28.2.13676358 in the selected SDK first" >&2; exit 1; }
# NDK 只发 darwin-x86_64 和 linux-x86_64 两种主机工具链，Apple 芯片上的 darwin-x86_64 也是通用二进制。
case $(uname -s) in
  Darwin) host_tag=darwin-x86_64 ;;
  Linux) host_tag=linux-x86_64 ;;
  *) echo "Unsupported host $(uname -s)" >&2; exit 1 ;;
esac
bin="$ndk/toolchains/llvm/prebuilt/$host_tag/bin"
upper=$(tr '[:lower:]-' '[:upper:]_' <<< "$target")
rustup target add "$target" >/dev/null 2>&1 || true
export "CC_${target//-/_}=$bin/${target}28-clang"
export "AR_${target//-/_}=$bin/llvm-ar"
export "CARGO_TARGET_${upper}_LINKER=$bin/${target}28-clang"
export CARGO_TARGET_DIR="$repo_root/target/android-cargo"
messages=$(cargo test -p msime-client-core --lib --locked --target "$target" --no-run --message-format json)
executable=$(python3 -c '
import json, sys
paths = [m["executable"] for m in map(json.loads, sys.stdin) if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == "msime_client_core"]
print(paths[-1] if paths else "")
' <<< "$messages")
[[ -n "$executable" && -f "$executable" ]] || { echo "cargo did not report the client-core test executable" >&2; exit 1; }
echo "$executable"
