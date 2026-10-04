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
  "$image_tag" bash -euo pipefail -c '
    cargo build -p msime-host-api --locked
    cmake -S platforms/linux -B /build/cmake -G Ninja \
      -DMSIME_ENABLE_FCITX5=ON \
      -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so
    cmake --build /build/cmake
    ctest --test-dir /build/cmake --output-on-failure
    # 不是 full 的版本只差 LinuxEdition.h 里的名字和按版本改写的脚本，但那条配置与编译路径（cmake/Edition.cmake、改写规则、系统目录下带版本名的文件）只有打包时才会走到。这里用五笔版按打包的前缀配置并编译一遍，不跑单测；再把它与 full 各自装进暂存目录，两边不能有同一个路径的文件，否则两个包装不到一起。
    cmake -S platforms/linux -B /build/cmake-wubi -G Ninja \
      -DMSIME_EDITION=wubi \
      -DCMAKE_INSTALL_PREFIX=/opt/msime-linux-wubi \
      -DBUILD_TESTING=OFF \
      -DMSIME_ENABLE_FCITX5=ON \
      -DMSIME_HOST_LIBRARY=/build/cargo/debug/libmsime_host_api.so
    cmake --build /build/cmake-wubi
    rm -rf /build/stage-full /build/stage-wubi
    DESTDIR=/build/stage-full cmake --install /build/cmake --prefix /usr >/dev/null
    DESTDIR=/build/stage-wubi cmake --install /build/cmake-wubi >/dev/null
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
