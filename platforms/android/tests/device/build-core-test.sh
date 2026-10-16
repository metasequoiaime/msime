#!/usr/bin/env bash
# 把 msime-client-base、msime-client-core、msime-client-providers 和 msime-client-services 的单测交叉编译后在专用 AVD 上整套运行：run-core-test.sh 是这个目标的 cargo runner，cargo 把每个测试程序分别交给它。设备用 MSIME_ANDROID_TEST_SERIAL 选，默认是 API 35 的 emulator-5580（见 run-core-test.sh）。ABI 跟着 start-emulator.sh 选的系统镜像走：arm64 主机（Apple 芯片）是 arm64-v8a，x86_64 主机（CI 的 Linux runner）是 x86_64，也可以用参数指定。
#
# 这一步存在的原因：client-base、client-core、client-providers 和 client-services 的私有文件读写在 Android 上受 /data 只给搜索权限、SELinux 禁止硬链接这些限制，macOS 和 Linux 上的单测一条都碰不到。0.3.0 就因此带着「词库准备失败」和匿名账号写入失败发了出去。
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
# runner 用 `--config` 的 TOML 数组传入：`CARGO_TARGET_<triple>_RUNNER` 环境变量会被 cargo 按空白切开，仓库路径一带空格（例如外接盘 `/Volumes/My Disk/...`）脚本就找不到了。数组元素写成 TOML 字面字符串，路径里不能有单引号。
runner="$repo_root/platforms/android/tests/device/run-core-test.sh"
[[ "$runner" != *"'"* ]] || { echo "repository path must not contain a single quote: $repo_root" >&2; exit 1; }
export CARGO_TARGET_DIR="$repo_root/target/android-cargo"
cargo --config "target.$target.runner=['bash', '$runner']" test -p msime-client-base -p msime-client-core -p msime-client-providers -p msime-client-services --lib --locked --no-fail-fast --target "$target"
