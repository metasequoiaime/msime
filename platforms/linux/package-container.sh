#!/usr/bin/env bash
# Build the installable Linux release in a container: a Release host library, the Tauri desktop binary, the native hosts configured for /usr with packaging on, and the CPack .deb and .tar.gz with a SHA256SUMS beside them.
#
# Separate from build-container.sh on purpose. That script is the pre-merge compile-and-unit-test gate and builds Debug against the debug host library; a release must not ship that tree, and the gate must not grow a Tauri build and a packaging step it does not need.
#
# Usage: platforms/linux/package-container.sh [VERSION]
#   VERSION defaults to platforms/linux/version.txt, the version release-linux.yml tags as linux-vVERSION. It becomes both the package version and the version the desktop binary reports, so the in-app update check compares like with like.
#   MSIME_PACKAGE_DESKTOP=0 packages without the Tauri desktop binary (no settings window); the default requires it.
#   MSIME_RELEASE_DATE=YYYY-MM-DD 是设置应用关于页显示的发布日期（apps/desktop/src-tauri/build.rs），缺省取 HEAD 的提交日期。
#   CARGO_BUILD_JOBS and CMAKE_BUILD_PARALLEL_LEVEL are passed through when set, to bound memory on a shared Docker VM.
#   MSIME_BUILD_RUN 设置时原样传进容器：release-linux.yml 把它设成 github.run_id，编进 msime-client-services 作为本构建的运行 ID（update_check::BUILD_RUN），应用内检查更新据此比较同一版本号的构建；本机打包不设，构建没有运行 ID，只比版本号。
#   MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 fails unless target/language-dictionaries holds every dictionary resources/language-dictionaries.lock.json pins, each with its licence; the default packages whatever is there.
#   MSIME_REQUIRE_WORDBOOKS=1 fails unless target/wordbooks holds every 背单词 wordbook resources/wordbook.lock.json lists, with wordbooks-NOTICE.txt (python3 scripts/fetch_wordbooks.py builds them; release-linux.yml sets it); the default installs them into share/<client>/wordbooks when they are there
#   MSIME_REQUIRE_OFFICIAL_PLUGINS=1 fails unless target/plugin-packs holds the official plugin packs scripts/build-official-plugins.sh builds (release-linux.yml sets it); the default installs them into share/<client>/plugin-packs when they are there and packages without them otherwise.
#   MSIME_PACKAGE_EDITIONS 是要打包的版本 id，逗号分隔，缺省是版本表里每个有 Linux 段的版本（platforms/linux/scripts/edition_linux.py editions）。各版本共用同一次构建出的宿主库、msime-mcp 和桌面二进制，原生宿主按版本各配置、各打一个包：full 是今天的 msime-linux，装在 /usr；其他版本是 msime-linux-<id>，装在 /opt/msime-linux-<id>。单测只在 full 的构建上跑，其他版本的原生代码与它只差 LinuxEdition.h 里的名字。
#   MSIME_PACKAGE_FORMAT=rpm builds the RPM in a Fedora container (tests/tools/Dockerfile.package-rpm) instead of the .deb and .tar.gz in the Debian one. It is a separate build, not a conversion: rpmbuild takes Requires from the libraries the binaries link, so they have to be linked against Fedora's (#2095).
#   MSIME_PACKAGE_TESTS_ONLY=1 只走到 ctest 为止：同一个镜像、同样的 Release 构建和 CMake 配置，不跑 cpack、不出任何包，版本缺省只有 full（单测只在 full 上跑），也缺省不编桌面二进制（MSIME_PACKAGE_DESKTOP 缺省变为 0）。ci-platforms.yml 的「Linux native host (Fedora)」在 PR 上用它配 MSIME_PACKAGE_FORMAT=rpm，把只在 Fedora 打包容器里失败的 ctest 提前到合并前，见 .agents/notes/implemented/process/2026-10-28-ci-red-tracking.md。
#
# The desktop binary embeds the web frontend at compile time, so apps/desktop/dist must be built first (`pnpm install --frozen-lockfile && pnpm --filter @msime/desktop build`). It is built outside the container because the container has no Node toolchain and a bind-mounted node_modules would mix host and container binaries.
#
# Output: target/linux-package/dist/{*.deb,*.tar.gz,SHA256SUMS}, or target/linux-package-rpm/dist/{*.rpm,SHA256SUMS} for the RPM; each edition contributes its own package (msime-linux_*, msime-linux-<id>_* and so on).
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo_root"

command -v docker >/dev/null 2>&1 || {
  echo "docker is required; run this on a Linux host with docker instead" >&2
  exit 2
}

version="${1:-$(tr -d '[:space:]' < platforms/linux/version.txt)}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "version must be MAJOR.MINOR.PATCH: $version" >&2
  exit 2
}
tests_only="${MSIME_PACKAGE_TESTS_ONLY:-0}"
[[ "$tests_only" == 0 || "$tests_only" == 1 ]] || { echo "MSIME_PACKAGE_TESTS_ONLY must be 0 or 1: $tests_only" >&2; exit 2; }
# 只测试时缺省不编 Tauri 设置应用：ctest 不用它，它却是整个构建里最慢的一块。
if [ "$tests_only" = 1 ]; then
  desktop="${MSIME_PACKAGE_DESKTOP:-0}"
else
  desktop="${MSIME_PACKAGE_DESKTOP:-1}"
fi
# 容器里的 git 读不了挂进去的检出（worktree 的 .git 指向宿主机路径，目录属主也与容器里的 root 不同），所以发布日期在宿主机上取好传进去。源码包解出的树没有 .git、自带 platforms/linux/release-date.txt，这时传空值，交给 build.rs 读那个文件。
release_date="${MSIME_RELEASE_DATE:-}"
if [ -z "$release_date" ] && [ ! -f platforms/linux/release-date.txt ]; then
  release_date=$(git log -1 --format=%cs HEAD 2>/dev/null || true)
fi
case "$tests_only" in
  0) editions="${MSIME_PACKAGE_EDITIONS:-$(python3 platforms/linux/scripts/edition_linux.py editions)}" ;;
  1)
    editions="${MSIME_PACKAGE_EDITIONS:-full}"
    [[ ",$editions," == *",full,"* ]] || { echo "MSIME_PACKAGE_TESTS_ONLY=1 runs the unit tests, which only the full edition builds; MSIME_PACKAGE_EDITIONS must include full" >&2; exit 2; }
    ;;
esac
known_editions=",$(python3 platforms/linux/scripts/edition_linux.py editions),"
IFS=, read -r -a edition_list <<<"$editions"
[ "${#edition_list[@]}" -gt 0 ] || { echo "MSIME_PACKAGE_EDITIONS names no edition" >&2; exit 2; }
for edition in "${edition_list[@]}"; do
  [[ "$known_editions" == *",$edition,"* ]] || {
    echo "MSIME_PACKAGE_EDITIONS names $edition, which has no Linux identifiers in shared/contracts/editions.json" >&2
    exit 2
  }
done
format="${MSIME_PACKAGE_FORMAT:-deb}"
case "$format" in
  deb) build_root="$repo_root/target/linux-package" ;;
  rpm) build_root="$repo_root/target/linux-package-rpm" ;;
  *) echo "MSIME_PACKAGE_FORMAT must be deb or rpm: $format" >&2; exit 2 ;;
esac
if [ "$desktop" = 1 ] && [ ! -f apps/desktop/dist/index.html ]; then
  echo "apps/desktop/dist is missing; run 'pnpm install --frozen-lockfile && pnpm --filter @msime/desktop build' first, or set MSIME_PACKAGE_DESKTOP=0" >&2
  exit 2
fi

mkdir -p "$build_root"
# Third-party notices of the statically linked Rust crates and the bundled npm packages. The npm walk runs here because the container has no node_modules of its own; the crate walk runs in the container after the builds that resolve those crates. Cleared first so a desktop-less run cannot pick up an earlier frontend notice.
rm -rf "$build_root/notices"
mkdir -p "$build_root/notices"
if [ "$desktop" = 1 ]; then
  python3 platforms/linux/collect-notices.py npm "$build_root/notices/frontend-npm-NOTICES.txt" apps/desktop
fi

# 镜像只含工具链；相同配方跨 worktree 复用，使用镜像 ID 避免并发标签覆盖。
if [ "$format" = rpm ]; then
  package_image=$(python3 platforms/linux/scripts/package-image.py msime-linux-package-rpm \
    platforms/linux/tests/tools/Dockerfile.package-rpm)
else
  gate_image=$(python3 platforms/linux/scripts/package-image.py msime-linux-build-gate \
    platforms/linux/tests/tools/Dockerfile.build-gate --tag)
  package_image=$(python3 platforms/linux/scripts/package-image.py msime-linux-package \
    platforms/linux/tests/tools/Dockerfile.package --build-arg MSIME_BUILD_GATE_IMAGE="$gate_image")
fi
echo "package image: $package_image" >&2

docker run --rm --init \
  -v "$repo_root":/source \
  -v "$build_root":/build \
  -w /source \
  -e CARGO_TARGET_DIR=/build/cargo \
  -e CARGO_HOME=/build/cargo-home \
  -e MSIME_BUILD_REVISION="${MSIME_BUILD_REVISION:-$(git rev-parse HEAD)}" \
  -e MSIME_BUILD_RUN="${MSIME_BUILD_RUN:-${GITHUB_RUN_ID:-}}" \
  -e GITHUB_RUN_ATTEMPT="${GITHUB_RUN_ATTEMPT:-}" \
  -e MSIME_PACKAGE_IMAGE="$package_image" \
  -e MSIME_VERSION="$version" \
  -e MSIME_RELEASE_DATE="$release_date" \
  -e MSIME_PACKAGE_DESKTOP="$desktop" \
  -e MSIME_PACKAGE_FORMAT="$format" \
  -e MSIME_PACKAGE_EDITIONS="$editions" \
  -e MSIME_PACKAGE_TESTS_ONLY="$tests_only" \
  -e MSIME_REQUIRE_LANGUAGE_DICTIONARIES="${MSIME_REQUIRE_LANGUAGE_DICTIONARIES:-0}" \
  -e MSIME_REQUIRE_OFFICIAL_PLUGINS="${MSIME_REQUIRE_OFFICIAL_PLUGINS:-0}" \
  -e MSIME_REQUIRE_WORDBOOKS="${MSIME_REQUIRE_WORDBOOKS:-0}" \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS"} \
  ${CMAKE_BUILD_PARALLEL_LEVEL:+-e CMAKE_BUILD_PARALLEL_LEVEL="$CMAKE_BUILD_PARALLEL_LEVEL"} \
  "$package_image" bash -euo pipefail -c '
    bash platforms/linux/scripts/prepare-package-cache.sh
    # 先保留行号用于符号归档，CPack 安装步骤只剥离安装副本；原件继续供其余版本使用。
    export CARGO_PROFILE_RELEASE_STRIP=none
    export CARGO_PROFILE_RELEASE_DEBUG=line-tables-only
    cargo build --release --locked -p msime-host-api
    cargo build --release --locked -p msime-mcp-server --bin msime-mcp
    desktop_args=()
    crate_roots=(msime-host-api msime-mcp-server)
    if [ "$MSIME_PACKAGE_DESKTOP" = 1 ]; then
      # tauri/custom-protocol is what `tauri build` enables: without it the binary is a dev build that loads devUrl instead of the embedded frontend. TAURI_CONFIG sets the version the app reports, as Build-Client.ps1 does for Windows.
      TAURI_CONFIG="{\"version\":\"$MSIME_VERSION\"}" \
        cargo build --release --locked -p msime-desktop --bin msime-desktop --features tauri/custom-protocol
      desktop_args=(-DMSIME_DESKTOP_BINARY=/build/cargo/release/msime-desktop -DMSIME_FRONTEND_NOTICES=/build/notices/frontend-npm-NOTICES.txt)
      crate_roots+=(msime-desktop:tauri/custom-protocol)
    fi
    python3 platforms/linux/collect-notices.py cargo /build/notices/rust-crates-NOTICES.txt "${crate_roots[@]}"
    # The sherpa-onnx runtime msime-voice-local loads for on-device recognition, pinned by resources/voice-runtime.lock.json for the architecture of this container.
    case "$(uname -m)" in
      x86_64) voice_platform=linux-x86_64 ;;
      aarch64) voice_platform=linux-aarch64 ;;
      *) echo "no pinned voice runtime for $(uname -m)" >&2; exit 2 ;;
    esac
    python3 scripts/fetch_voice_runtime.py --platform "$voice_platform" --out /build/voice-runtime
    # 发布页的 deb/rpm 不带手写用的 Zinnia 模型（26.8 MB）：每个版本都以 -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF 配置，设置应用在用户需要手写时按 resources/handwriting-model.lock.json 下载模型和它的许可证。发行版仓库的包（packaging/ 下各定义和 nix）不传这个选项，照旧随包。下面的离线释义交给每个版本的 configure；不提供中文方案的版本（版本表 features.offline_glosses 为 false）由 CMakeLists.txt 去掉，所以日文、越南文和藏文版的安装包里没有。
    # Non-English candidate glosses from scripts/fetch_offline_glosses.py, installed only when the databases and their NOTICE are both there; without them the package glosses offline in English only.
    glosses_args=()
    if compgen -G "target/offline-glosses/zh-*.db" >/dev/null && [ -f target/offline-glosses/offline-glosses-NOTICE.txt ]; then
      glosses_args=(-DMSIME_OFFLINE_GLOSSES=/source/target/offline-glosses)
    fi
    # 随桌面安装包分发的官方插件包（scripts/build-official-plugins.sh 的产物）。发版工作流在打包前把它放进 target/plugin-packs 并设 MSIME_REQUIRE_OFFICIAL_PLUGINS=1，没有时失败；本机打包没有它时照旧打包，只是不带官方包。
    plugin_packs_args=()
    if [ -d target/plugin-packs/script ] && [ -d target/plugin-packs/phrase_table ]; then
      plugin_packs_args=(-DMSIME_PLUGIN_PACKS=/source/target/plugin-packs)
    elif [ "$MSIME_REQUIRE_OFFICIAL_PLUGINS" = 1 ]; then
      echo "MSIME_REQUIRE_OFFICIAL_PLUGINS=1 but target/plugin-packs holds no official plugin packs; run scripts/build-official-plugins.sh first" >&2
      exit 1
    fi
    # 背单词的内置词书（scripts/fetch_wordbooks.py），每个版本都装，装在 share/<client>/wordbooks；MSIME_REQUIRE_WORDBOOKS=1 时 CMake 在缺了锁文件列出的任何一本时失败。
    wordbooks_args=(-DMSIME_REQUIRE_WORDBOOKS="$([ "$MSIME_REQUIRE_WORDBOOKS" = 1 ] && echo ON || echo OFF)")
    if [ -d target/wordbooks ]; then
      wordbooks_args+=(-DMSIME_WORDBOOKS=/source/target/wordbooks)
    fi
    # The Cantonese, Zhuyin and Stroke dictionaries from scripts/fetch_language_dictionaries.py, each installed with its licence text; without them the package offers those schemes as unavailable. MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 fails the package instead.
    languages_args=(-DMSIME_REQUIRE_LANGUAGE_DICTIONARIES="$([ "$MSIME_REQUIRE_LANGUAGE_DICTIONARIES" = 1 ] && echo ON || echo OFF)")
    if [ -d target/language-dictionaries ]; then
      languages_args+=(-DMSIME_LANGUAGE_DICTIONARIES=/source/target/language-dictionaries)
    fi
    # 每轮清空发行附件，保留对象文件；配置缓存单独重建，旧版本号和可选资源不能进入本次包。
    rm -rf /build/dist
    IFS=, read -r -a editions <<<"$MSIME_PACKAGE_EDITIONS"
    for edition in "${editions[@]}"; do
      # 每个版本一个构建目录：full 仍是 /build/cmake，装在 /usr；其他版本装在版本表给的前缀下。粤语、注音和笔画词库只有用到它们的版本要（cmake/Edition.cmake 对其他版本忽略），所以「必须带齐」也只对 full 生效。单测只跑 full 的。
      build=/build/cmake
      testing=ON
      edition_languages_args=("${languages_args[@]}")
      if [ "$edition" != full ]; then
        build=/build/cmake-$edition
        testing=OFF
        edition_languages_args=(-DMSIME_REQUIRE_LANGUAGE_DICTIONARIES=OFF)
      fi
      prefix=$(python3 platforms/linux/scripts/edition_linux.py field --edition "$edition" install_prefix)
      rm -f "$build/CMakeCache.txt"
      cmake -S platforms/linux -B "$build" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DMSIME_RELEASE_CRASH_SYMBOLS=ON \
        -DCMAKE_INSTALL_PREFIX="$prefix" \
        -DMSIME_EDITION="$edition" \
        -DBUILD_TESTING="$testing" \
        -DMSIME_ENABLE_PACKAGING=ON \
        -DMSIME_ENABLE_FCITX5=ON \
        -DMSIME_HOST_LIBRARY=/build/cargo/release/libmsime_host_api.so \
        -DMSIME_MCP_BINARY=/build/cargo/release/msime-mcp \
        -DMSIME_PACKAGE_VERSION="$MSIME_VERSION" \
        -DMSIME_RUST_NOTICES=/build/notices/rust-crates-NOTICES.txt \
        -DMSIME_VOICE_RUNTIME_DIR=/build/voice-runtime \
        -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF \
        "${desktop_args[@]}" "${glosses_args[@]}" "${plugin_packs_args[@]}" "${edition_languages_args[@]}" "${wordbooks_args[@]}"
      cmake --build "$build"
      if [ "$testing" = ON ]; then
        ctest --test-dir "$build" --output-on-failure
      fi
      [ "$MSIME_PACKAGE_TESTS_ONLY" = 0 ] || continue
      python3 scripts/crash-symbols.py collect --platform linux --version "$MSIME_VERSION" \
        --root "$build" --root /build/cargo/release \
        --output "/build/symbols/linux-$MSIME_VERSION-$MSIME_PACKAGE_FORMAT-$edition-$(uname -m)-${MSIME_BUILD_RUN:-local}-${GITHUB_RUN_ATTEMPT:-1}.zip"
      # Rust 安装副本由 install(CODE) 剥离；原件保留给后续版本归档。

      if [ "$MSIME_PACKAGE_FORMAT" = rpm ]; then
        cpack --config "$build/CPackConfig.cmake" -G RPM -B /build/dist
      else
        cpack --config "$build/CPackConfig.cmake" -G "TGZ;DEB" -B /build/dist
      fi
      rm -rf /build/dist/_CPack_Packages
    done
    if [ "$MSIME_PACKAGE_TESTS_ONLY" = 1 ]; then
      echo "MSIME_PACKAGE_TESTS_ONLY=1: built and tested $MSIME_PACKAGE_EDITIONS, no packages produced" >&2
      exit 0
    fi
    cd /build/dist
    if [ "$MSIME_PACKAGE_FORMAT" = rpm ]; then
      sha256sum -- *.rpm > SHA256SUMS
    else
      # 这个 bookworm 镜像里的 CPack（CMake 3.25）只用 xz 预设 6 压缩 data.tar.xz，也没有 CPACK_DEBIAN_COMPRESSION_LEVEL，所以每个 .deb 都以 -9 重新打包，大约还能再省五分之一。单个压缩线程把内存控制在 xz -9 每线程所需的 674 MiB，单个块的压缩率也最好。重新打包还会把 control.tar.gz 变成 control.tar.xz，release-linux.yml 据此确认这一步执行过。
      # 同一次重新打包里把 dpkg-shlibdeps 从 Fcitx5 插件推出的 libfcitx5* 从 Depends 里去掉：Fcitx5 只作为 IBus 的备选出现（`ibus | fcitx5`，见 cmake/packaging.cmake），否则 IBus 系统会被拉进 Fcitx5 的库，Ubuntu 22.04 这类 Fcitx5 低于 5.0.20 的系统更是整包装不上，连 IBus 也用不了（#6305、#6401）。CPack 没有按文件跳过 dpkg-shlibdeps 的办法，只能在这里改。
      for deb in *.deb; do
        repack=$(mktemp -d)
        dpkg-deb --raw-extract "$deb" "$repack/root"
        sed -E -i "/^Depends:/s/, libfcitx5[a-z0-9]+( \([^)]*\))?//g" "$repack/root/DEBIAN/control"
        if grep -E "^Depends:.*libfcitx5" "$repack/root/DEBIAN/control"; then
          echo "$deb: Depends still names a Fcitx5 library after the rewrite" >&2
          exit 1
        fi
        dpkg-deb --root-owner-group --threads-max=1 -Zxz -z9 --build "$repack/root" "$deb" >/dev/null
        rm -rf "$repack"
      done
      sha256sum -- *.deb *.tar.gz > SHA256SUMS
    fi
    cat SHA256SUMS
  '
