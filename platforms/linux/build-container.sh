#!/usr/bin/env bash
# Compile the Linux native host - the IBus engine, the Fcitx5 addon, every
# provider entry point and all the unit tests - and run the tests, in a
# container, so a machine that is not Linux can still hold this gate.
#
# It exists because nothing held it. `verify-local.sh` had no phase for
# platforms/linux at all, and the build had drifted into failing: three tests
# carried relative includes one level short of where the sources moved, and one
# of them could not name its own fixtures. The whole target - the product on this
# platform - would not configure.
#
# This is a compile-and-unit-test gate, not acceptance. It ships no locked
# dictionaries and starts no D-Bus or IBus daemon; tests/tools/check-container.sh
# is the run that does, and it needs a verified resource directory.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

command -v docker >/dev/null 2>&1 || {
  echo "docker is required; run this on a Linux host instead" >&2
  exit 2
}

build_root="$repo_root/target/linux-build-gate"
mkdir -p "$build_root"

# Tag per checkout. Several worktrees build this gate at once and the image is
# built from the Dockerfile in each of them, so a fixed tag means whichever
# finished last decides what everybody runs - a gate silently executing another
# checkout's image is worse than no gate.
image_tag="msime-linux-build-gate:$(printf %s "$repo_root" | shasum | cut -c1-12)"

docker build -q -t "$image_tag" \
  -f platforms/linux/tests/tools/Dockerfile.build-gate platforms/linux/tests >/dev/null
echo "gate image: $image_tag" >&2

docker run --rm --init \
  -v "$repo_root":/source \
  -v "$build_root":/build \
  -w /source \
  -e CARGO_TARGET_DIR=/build/cargo \
  -e CARGO_HOME=/build/cargo-home \
  "$image_tag" bash -euo pipefail -c '
    cargo build -p msime-host-api --locked
    # 读取实际生成的目标图，关闭测试时不得再编译 tests/ 下的源码。
    for configuration in cmake cmake-wubi; do
      mkdir -p "/build/$configuration/.cmake/api/v1/query"
      touch "/build/$configuration/.cmake/api/v1/query/codemodel-v2"
    done
    # scripts/build-official-plugins.sh 构建过官方插件包时，按打包的方式装进 share/<client>/plugin-packs，安装步骤之后核对两个包都到了；没有时显式清空这个选项，免得沿用上一次配置留在缓存里的路径。
    plugin_packs=
    if [ -d target/plugin-packs/script ] && [ -d target/plugin-packs/phrase_table ]; then
      plugin_packs=/source/target/plugin-packs
    fi
    cmake -S platforms/linux -B /build/cmake -G Ninja \
      -DBUILD_TESTING=ON \
      -DMSIME_ENABLE_FCITX5=ON \
      -DMSIME_PLUGIN_PACKS="$plugin_packs" \
      -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so
    cmake --build /build/cmake
    ctest --test-dir /build/cmake --output-on-failure --output-junit /build/native-tests.xml
    # 三项真实 classicui 测试、三项在 Xvfb 里画 IBus 切换徽章的测试，以及悬浮工具栏与双拼键位图的画法、Xvfb 窗口和 IBus 接线测试必须注册并执行，缺依赖或返回 77 都不能让门禁假绿。
    python3 - /build/native-tests.xml <<"PY"
import sys
import xml.etree.ElementTree as ET
report = ET.parse(sys.argv[1]).getroot()
for name in ("fcitx5-candidate-theme-priority", "fcitx5-candidate-theme-hint", "fcitx5-candidate-theme-canvas",
             "ibus-mode-hint-x11-badge", "ibus-mode-hint-x11-badge-no-caret",
             "ibus-mode-hint-x11-badge-point", "ibus-mode-hint-x11-toolbar",
             "linux-floating-panels-model", "linux-floating-panels-render", "linux-floating-panels-x11",
             "linux-python-crash-capture"):
    tests = [test for test in report.findall("testcase") if test.get("name") == name]
    assert len(tests) == 1, f"{name}: missing from Linux gate"
    assert tests[0].get("status") == "run" and tests[0].find("skipped") is None, f"{name}: skipped"
PY
    # 不是 full 的版本只差 LinuxEdition.h 里的名字和按版本改写的脚本，但那条配置与编译路径（cmake/Edition.cmake、改写规则、系统目录下带版本名的文件）只有打包时才会走到。这里用五笔版按打包的前缀配置并编译一遍，不跑单测；再把它与 full 各自装进暂存目录，两边不能有同一个路径的文件，否则两个包装不到一起。
    cmake -S platforms/linux -B /build/cmake-wubi -G Ninja \
      -DMSIME_EDITION=wubi \
      -DCMAKE_INSTALL_PREFIX=/opt/msime-linux-wubi \
      -DBUILD_TESTING=OFF \
      -DMSIME_ENABLE_FCITX5=ON \
      -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so
    python3 platforms/linux/tests/core/build_testing.py /build/cmake /build/cmake-wubi
    cmake --build /build/cmake-wubi
    rm -rf /build/stage-full /build/stage-wubi
    DESTDIR=/build/stage-full cmake --install /build/cmake --prefix /usr >/dev/null
    DESTDIR=/build/stage-wubi cmake --install /build/cmake-wubi >/dev/null
    # 从实际安装目录再跑 Python 入口：同时检查模块分发、相对库路径和非 full 的构建归属。
    for edition in full wubi; do
      prefix=$(python3 platforms/linux/scripts/edition_linux.py field --edition "$edition" install_prefix)
      client=$(python3 platforms/linux/scripts/edition_linux.py field --edition "$edition" client_directory)
      telemetry=$(python3 platforms/linux/scripts/edition_linux.py field --edition "$edition" fcitx5_addon)
      installed="/build/stage-$edition$prefix"
      python3 platforms/linux/tests/core/python_crash_capture.py \
        --controller /build/cmake/common-capture-only-test \
        --bridge "$installed/lib/$client/libmsime-python-crash.so" \
        --paths "$installed/bin/msime_voice_paths.py" --scripts "$installed/bin" \
        --version "$(cat platforms/linux/version.txt)" --client-directory "$client" --telemetry-directory "$telemetry"
    done
    if [ -n "$plugin_packs" ]; then
      for pack in script/uuid phrase_table/latex-symbols; do
        installed=/build/stage-full/usr/share/msime-client/plugin-packs/$pack
        if [ ! -f "$installed/plugin.toml" ] || ! diff -r "$plugin_packs/$pack" "$installed" >/dev/null; then
          echo "the official plugin pack $pack was not installed into share/msime-client/plugin-packs as built" >&2
          exit 1
        fi
      done
      echo "official plugin packs installed: $(cd /build/stage-full/usr/share/msime-client/plugin-packs && find . -name plugin.toml | sort | tr "\n" " ")" >&2
    fi
    # 不内置词库的 full/wubi 安装也必须带齐辅助码，否则 setup 下载词库后仍无法筛选。
    for packaged in /build/stage-full/usr/share/msime-client /build/stage-wubi/opt/msime-linux-wubi/share/msime-client-wubi; do
      for table in helpcode.txt zrm_helpcode_big_unique.txt shouyou2_0_helpcode.txt shouyouplus_helpcode.txt xiaohe_helpcode.txt jiajia_helpcode.txt wubi86_helpcode.txt; do
        cmp "resources/helpcodes/$table" "$packaged/resources/helpcodes/$table"
      done
    done
    shared=$(comm -12 <(cd /build/stage-full && find . ! -type d | sort) <(cd /build/stage-wubi && find . ! -type d | sort))
    if [ -n "$shared" ]; then
      echo "full and wubi install the same paths:" >&2
      echo "$shared" >&2
      exit 1
    fi
    # 五笔版的 Fcitx5 插件要按本版本的文件名找宿主库，不能是 full 的 libmsime_host_api.so：同一个 fcitx5 进程按这个名字复用已经加载的库，两个版本会共用先加载的那一份。
    plugin=$(find /build/stage-wubi -name libmsime-wubi-fcitx5.so -print -quit)
    needed=$(readelf -d "$plugin" | grep NEEDED)
    if ! grep -q "\[libmsime_host_api_wubi\.so\]" <<<"$needed" || grep -q "\[libmsime_host_api\.so\]" <<<"$needed"; then
      echo "the wubi Fcitx5 plugin does not link its own host library:" >&2
      echo "$needed" >&2
      exit 1
    fi
    # 日文版只配置不编译：它的原生代码与五笔版只差 LinuxEdition.h 里的名字，但它是不带中文主词库、登记在另一个语言下的版本，cmake/Edition.cmake 和改写规则在这里走另一条分支。配置出的 IBus 组件和 Fcitx5 输入法条目要登记在日语（ja）下。
    cmake -S platforms/linux -B /build/cmake-japanese -G Ninja \
      -DMSIME_EDITION=japanese \
      -DCMAKE_INSTALL_PREFIX=/opt/msime-linux-japanese \
      -DBUILD_TESTING=OFF \
      -DMSIME_ENABLE_FCITX5=ON \
      -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so >/dev/null
    if ! grep -q "<language>ja</language>" /build/cmake-japanese/msime-linux-japanese.xml ||
       ! grep -qx "LangCode=ja" /build/cmake-japanese/edition/fcitx5/msime-inputmethod.conf; then
      echo "the japanese edition does not register under Japanese:" >&2
      grep -h -e "<language>" -e "^LangCode=" /build/cmake-japanese/msime-linux-japanese.xml /build/cmake-japanese/edition/fcitx5/msime-inputmethod.conf >&2
      exit 1
    fi
  '
