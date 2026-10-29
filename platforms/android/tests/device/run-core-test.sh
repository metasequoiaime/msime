#!/usr/bin/env bash
# 专用 AVD 上的 cargo runner：build-core-test.sh 跑 cargo test 时，cargo 把 client-base、client-core、client-providers、client-services 的每个单测程序分别交给它，在设备上整套运行；msime-engine 的单测程序带着测试名过滤参数交过来，只跑过滤出的那几组。
set -euo pipefail
[[ $# -ge 1 && -f "$1" ]] || { echo "Expected a Cargo test binary and optional test filters" >&2; exit 1; }
binary=$1
shift
# 过滤参数原样拼进设备上的 shell 命令，只收测试名里会出现的字符。
for filter in "$@"; do
  [[ "$filter" =~ ^[A-Za-z0-9_:]+$ ]] || { echo "Unsupported test filter: $filter" >&2; exit 1; }
done
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
# MSIME_ANDROID_TEST_SERIAL 选另一个级别的专用 AVD（start-emulator.sh 打印的端口），默认是 API 35 的 emulator-5580。
serial=${MSIME_ANDROID_TEST_SERIAL:-emulator-5580}
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || { echo "Refusing a non-test AVD" >&2; exit 1; }
[[ $("$adb" -s "$serial" shell getprop sys.boot_completed | tr -d '\r') == 1 ]] || { echo "Test AVD has not booted" >&2; exit 1; }
"$adb" -s "$serial" shell mkdir -p /data/local/tmp/msime-rust-tests
"$adb" -s "$serial" push "$binary" /data/local/tmp/msime-rust-tests/core-tests >/dev/null
"$adb" -s "$serial" shell chmod 700 /data/local/tmp/msime-rust-tests/core-tests
# Android 的 SELinux 不允许建硬链接（应用和 adb shell 都是），名字里带 hard_link 的测试先建硬链接再断言它被拒绝，在这里准备不出被测对象，按名字跳过；应用在设备上也建不出硬链接，这类防护在那里没有对象。逐条加 cfg_attr 追不上新加的同类测试。
"$adb" -s "$serial" shell "TMPDIR=/data/local/tmp/msime-rust-tests /data/local/tmp/msime-rust-tests/core-tests $* --skip hard_link"
