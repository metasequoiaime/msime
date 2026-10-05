# 水杉输入法 Linux 完整版的 RPM 规格文件，供 Fedora COPR 与 openSUSE OBS 从源码构建。
#
# 内容与 release-linux.yml 用 CPack 打出的 msime-linux RPM 相同（Fcitx5 插件、IBus engine、msime-linux-setup 等入口、MCP 服务和 Tauri 设置窗口），构建步骤照搬 platforms/linux/package-container.sh：Release 的 Host API、MCP 服务和设置窗口，再以 MSIME_ENABLE_PACKAGING=ON 配置 CMake，跑与门禁相同的 ctest。区别只在依赖来源：COPR（默认）和 OBS 构建时没有网络，所以 Cargo 依赖、设置窗口的前端和需要下载的数据都来自 Source1，即 make-source-tarballs.sh 随每个 linux-v 发布生成的 vendor 包。词库照旧不随包，由用户首次配置时 msime-linux-setup --download 取回。
#
# Version 与 %%changelog 由 render-sources.py 按发布版本改写；仓库里的值只是上一次渲染的样子。
#
# 发布（以 0.9.1 为例，均需各平台自己的账号）：
#   COPR：rpmbuild -bs 渲染后的规格文件（_sourcedir 里放好 Source0、Source1 和 msime-rpmlintrc），再 `copr-cli build <owner>/msime msime-0.9.1-1.*.src.rpm`；项目的 chroot 选 fedora-*-x86_64 与 fedora-*-aarch64。
#   OBS：`osc checkout home:<user>/msime`，放入渲染后的 msime.spec、msime-rpmlintrc 和两个 tarball，`osc addremove && osc commit`；仓库选 openSUSE_Tumbleweed、openSUSE_Leap_16.0 与 Fedora_*。
#
# 编译器用发行版自己的 rust/cargo，不用 rust-toolchain.toml 钉住的版本：构建农场取不到 rustup，下限取锁定依赖里最高的 rust-version：Cargo.toml 自己声明 1.89，但 Cargo.lock 锁定的 tauri 2.12、tauri-utils、tauri-runtime-wry、muda、tray-icon 等声明 1.90，cargo 默认拒绝用更低的编译器构建它们。

# 包内私有目录里的 Host API 与 sherpa-onnx 运行库按 RUNPATH 加载，既不能向系统要，也不能当作系统库对外提供；与 packaging.cmake 给 CPack 的设置相同。
%global __requires_exclude ^lib(msime_host_api|sherpa-onnx-c-api|onnxruntime)\\.so.*$
%global __provides_exclude_from ^%{_libdir}/msime-client/.*$
# 不给 C/C++ 代码开 LTO：rusqlite 等 crate 用 cc 把 C 代码编成静态库，optflags 里的 -flto=auto 让库里只有 GCC 的 LTO 中间码，openSUSE 的 Rust 用 clang 加 rust-lld 链接，读不懂这种目标文件，链接时 sqlite3_* 等符号全部未定义（OBS openSUSE_Tumbleweed 上 msime-mcp 就是这样失败的）。
%define _lto_cflags %{nil}

%ifarch x86_64
%global voice_platform linux-x86_64
%endif
%ifarch aarch64
%global voice_platform linux-aarch64
%endif

Name:           msime
Version:        0.9.0
Release:        1%{?dist}
Summary:        Metasequoia IME (水杉输入法): Chinese input method for Fcitx5 and IBus
# 本项目代码为 GPL-3.0-only；其余是随包的第三方代码与数据：Rust crate 与 npm 包（rust-crates-NOTICES.txt、frontend-npm-NOTICES.txt 逐个列出）、sherpa-onnx 与 ONNX Runtime、nlohmann/json、Wayland 协议代码、手写模型、方言词库、离线释义和 resources/licenses 下的各项数据。
License:        GPL-3.0-only AND Apache-2.0 AND MIT AND BSD-3-Clause AND HPND AND LGPL-2.1-or-later AND LGPL-3.0-only AND CC-BY-4.0 AND CC-BY-SA-4.0 AND MPL-2.0 AND WTFPL AND Unicode-3.0 AND ISC AND Zlib
URL:            https://github.com/metasequoiaime/msime
Source0:        %{url}/releases/download/linux-v%{version}/msime-%{version}.tar.xz
Source1:        %{url}/releases/download/linux-v%{version}/msime-%{version}-vendor.tar.xz
# OBS 自动读取与包同名的 rpmlintrc；列为 Source 让它也进 .src.rpm。
Source99:       msime-rpmlintrc

# 语音运行库只为这两个架构钉住了上游构建（resources/voice-runtime.lock.json）。
ExclusiveArch:  x86_64 aarch64

BuildRequires:  cargo >= 1.90
BuildRequires:  rust >= 1.90
BuildRequires:  gcc-c++
BuildRequires:  cmake >= 3.25
BuildRequires:  make
BuildRequires:  python3
BuildRequires:  pkgconfig
BuildRequires:  cmake(Fcitx5Core) >= 5.0.20
BuildRequires:  cmake(nlohmann_json)
BuildRequires:  pkgconfig(ibus-1.0)
BuildRequires:  pkgconfig(xkbcommon)
BuildRequires:  pkgconfig(libcurl)
BuildRequires:  pkgconfig(glib-2.0)
BuildRequires:  pkgconfig(cairo)
BuildRequires:  pkgconfig(pango)
BuildRequires:  pkgconfig(wayland-client)
BuildRequires:  pkgconfig(wayland-protocols)
BuildRequires:  pkgconfig(wayland-scanner)
BuildRequires:  pkgconfig(x11)
BuildRequires:  pkgconfig(xext)
BuildRequires:  pkgconfig(xfixes)
BuildRequires:  pkgconfig(xrandr)
# Host API 经 cpal 用 ALSA 采集麦克风。
BuildRequires:  pkgconfig(alsa)
# Tauri 设置窗口。
BuildRequires:  pkgconfig(webkit2gtk-4.1)
BuildRequires:  pkgconfig(javascriptcoregtk-4.1)
BuildRequires:  pkgconfig(libsoup-3.0)
BuildRequires:  pkgconfig(gtk+-3.0)
BuildRequires:  pkgconfig(openssl)
# %%check：setup_update 测试经 pgrep 确认宿主进程，linux-ibus-startup-telemetry 要起 dbus-daemon，与门禁镜像装 procps 和 dbus 的理由相同。两边包名不同；不写成文件依赖，是因为 zypper 解析不到 /usr/bin/pgrep。
%if 0%{?suse_version}
BuildRequires:  procps
BuildRequires:  dbus-1
%else
BuildRequires:  procps-ng
BuildRequires:  dbus-daemon
%endif
BuildRequires:  desktop-file-utils

Requires:       ibus >= 1.5.20
Requires:       fcitx5 >= 5.0.20
Requires:       python3 >= 3.9
# msime-linux-setup 切换词库前用 pgrep 确认输入法是否在运行。
%if 0%{?suse_version}
Requires:       procps
%else
Requires:       procps-ng
%endif
# openSUSE 把 hicolor-icon-theme 当作 branding 包，rpmlint 不许无版本地依赖它；那里由 filesystem 一类的包提供图标目录。
%if !0%{?suse_version}
Requires:       hicolor-icon-theme
%endif
# 语音：豆包流式识别要 websockets 15 起的同步客户端，录音要 parec、pw-cat 或 arecord 之一。没有它们语音服务照样起来，只是用到的请求失败，所以是 Recommends。
Recommends:     python3-websockets >= 15
%if 0%{?suse_version}
Recommends:     (pulseaudio-utils or pipewire-tools or alsa-utils)
%else
Recommends:     (pulseaudio-utils or pipewire-utils or alsa-utils)
%endif
# GitHub 发布页上 CPack 打的 RPM 叫 msime-linux，文件与本包完全重合；装本包时把它替换掉。
Provides:       msime-linux = %{version}-%{release}
Obsoletes:      msime-linux < %{version}-%{release}

%description
Metasequoia IME (水杉输入法) is a Chinese input method. This package
contains the native Linux hosts built from source: the Fcitx5 addon, the
IBus engine, the first-run setup program msime-linux-setup, the provider
services, the MCP server and the settings window. The dictionaries are not
included; msime-linux-setup --download fetches them on first use.

%prep
%autosetup -n msime-%{version} -a 1
mv msime-%{version}-vendor vendor
# 有些 crate 的 .rs 带可执行位，首行又是 `#![...]`；它们随 debugsource 打包时 brp-mangle-shebangs 当成坏的 shebang 报错。Cargo 的校验只看内容，不看权限。
find vendor/cargo -type f -name '*.rs' -perm /111 -exec chmod a-x {} +

%build
# openSUSE 的 rpm 不一定定义 set_build_flags，这时退回 optflags。
%{?set_build_flags}
export CFLAGS="${CFLAGS:-%{optflags}}" CXXFLAGS="${CXXFLAGS:-%{optflags}}"
export CARGO_HOME="$PWD/.cargo-home" CARGO_NET_OFFLINE=true PYTHONDONTWRITEBYTECODE=1
%if 0%{?suse_version}
# Fedora 的 set_build_flags 会给 RUSTFLAGS 带上调试信息，openSUSE 不会；Release 配置默认又不带，find-debuginfo 就既不拆出调试信息也不剥离 Rust 编译的三个程序。只带行号表，与 debian/rules 相同。
export CARGO_PROFILE_RELEASE_DEBUG=line-tables-only
%endif
%{?_smp_build_ncpus:export CARGO_BUILD_JOBS=%{_smp_build_ncpus}}
mkdir -p "$CARGO_HOME"
sed 's|^directory = .*|directory = "'"$PWD"'/vendor/cargo"|' vendor/cargo-config.toml > "$CARGO_HOME/config.toml"

# 对着源码树里的锁文件核对 vendor 包里的数据。文件已在锁定的摘要上时这些脚本不联网；对不上就会去下载，在没有网络的构建农场里直接失败。语音运行库从归档解到构建目录，不改动 vendor/。
mkdir -p build/voice-runtime
cp -a vendor/voice-runtime/%{voice_platform}/.archive build/voice-runtime/
python3 scripts/fetch_voice_runtime.py --platform %{voice_platform} --out build/voice-runtime
python3 scripts/fetch_handwriting_model.py --out vendor/handwriting-model
python3 scripts/fetch_offline_glosses.py --out vendor/offline-glosses
python3 scripts/fetch_language_dictionaries.py --out vendor/language-dictionaries

# msime-desktop 编译时把前端嵌进去。tauri/custom-protocol 是 `tauri build` 打开的特性，没有它就是加载 devUrl 的开发构建；TAURI_CONFIG 设定应用报告的版本。
rm -rf apps/desktop/dist
cp -a vendor/frontend/dist apps/desktop/dist
cargo build --release --locked --offline -p msime-host-api
cargo build --release --locked --offline -p msime-mcp-server --bin msime-mcp
TAURI_CONFIG='{"version":"%{version}"}' \
  cargo build --release --locked --offline -p msime-desktop --bin msime-desktop --features tauri/custom-protocol
python3 platforms/linux/collect-notices.py cargo build/rust-crates-NOTICES.txt \
  msime-host-api msime-mcp-server msime-desktop:tauri/custom-protocol

# 不用发行版的 %%cmake：Fedora 与 openSUSE 的宏传的参数不同，openSUSE 默认还会 CMAKE_SKIP_RPATH，而插件和各入口正是按 $ORIGIN 的 RUNPATH 找包内的 Host API。
cmake -S platforms/linux -B build/cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=%{_prefix} \
  -DCMAKE_INSTALL_LIBDIR=%{_lib} \
  -DCMAKE_INSTALL_SYSCONFDIR=%{_sysconfdir} \
  -DCMAKE_SKIP_RPATH=OFF \
  -DCMAKE_SKIP_INSTALL_RPATH=OFF \
  -DMSIME_EDITION=full \
  -DMSIME_ENABLE_PACKAGING=ON \
  -DMSIME_ENABLE_FCITX5=ON \
  -DMSIME_PACKAGE_VERSION=%{version} \
  -DMSIME_HOST_LIBRARY="$PWD/target/release/libmsime_host_api.so" \
  -DMSIME_MCP_BINARY="$PWD/target/release/msime-mcp" \
  -DMSIME_DESKTOP_BINARY="$PWD/target/release/msime-desktop" \
  -DMSIME_RUST_NOTICES="$PWD/build/rust-crates-NOTICES.txt" \
  -DMSIME_FRONTEND_NOTICES="$PWD/vendor/frontend/frontend-npm-NOTICES.txt" \
  -DMSIME_VOICE_RUNTIME_DIR="$PWD/build/voice-runtime" \
  -DMSIME_HANDWRITING_MODEL_DIR="$PWD/vendor/handwriting-model" \
  -DMSIME_OFFLINE_GLOSSES="$PWD/vendor/offline-glosses" \
  -DMSIME_LANGUAGE_DICTIONARIES="$PWD/vendor/language-dictionaries" \
  -DMSIME_REQUIRE_LANGUAGE_DICTIONARIES=ON
cmake --build build/cmake %{?_smp_mflags}

%install
DESTDIR=%{buildroot} cmake --install build/cmake
# CPack 的 RPM 把 Debian 维护脚本翻译后内联；这里把 CMake 由同一份模板配置出的脚本装进包里，%%post/%%preun 再按 dpkg 的参数调用，单元列表和清理逻辑只维护在 platforms/linux/cmake/deb-*.in 一处。
install -Dm755 build/cmake/debian/postinst %{buildroot}%{_libexecdir}/msime-client/postinst
install -Dm755 build/cmake/debian/prerm %{buildroot}%{_libexecdir}/msime-client/prerm
# CMake 用 install(FILES) 装包内私有库，权限是 0644；find-debuginfo 只处理带可执行位的 ELF，不改就既不剥离也不拆出调试信息。只改本包编译的 Host API：sherpa-onnx 与 ONNX Runtime 是上游预构建库，没有 build-id，Fedora 的 find-debuginfo --strict-build-id 会因此失败，它们按原样安装。
chmod 0755 %{buildroot}%{_libdir}/msime-client/libmsime_host_api.so
%if 0%{?suse_version}
# Fedora 的 brp-mangle-shebangs 会把 `#!/usr/bin/env` 改成解释器的绝对路径，openSUSE 没有这一步，rpmlint 按 env-script-interpreter 记错。只改装出去的副本，源码树里的模板保持原样（settings_launcher_contract 测试核对它）。
for script in $(grep -lIE '^#!/usr/bin/env (python3|sh)$' %{buildroot}%{_bindir}/*); do
  sed -i -e '1s|^#!/usr/bin/env python3$|#!/usr/bin/python3|' -e '1s|^#!/usr/bin/env sh$|#!/bin/sh|' "$script"
done
%endif

%check
export PYTHONDONTWRITEBYTECODE=1
ctest --test-dir build/cmake --output-on-failure %{?_smp_mflags}
desktop-file-validate %{buildroot}%{_datadir}/applications/msime-linux.desktop
# ctest 跑的是构建目录，看不到装出去的程序能不能加载。Fcitx5 插件、IBus engine 和本地语音助手都按 RUNPATH 找包内私有目录里的库，它们必须解析到 buildroot 里的同一个目录。
private=%{buildroot}%{_libdir}/msime-client
for binary in %{buildroot}%{_libdir}/fcitx5/libmsime-fcitx5.so %{buildroot}%{_bindir}/msime-linux-ibus; do
  resolved=$(ldd "$binary" | awk '$1 == "libmsime_host_api.so" { print $3 }')
  echo "$binary: libmsime_host_api.so => $resolved"
  test "$(realpath -- "$resolved")" = "$(realpath -- "$private/libmsime_host_api.so")"
done
resolved=$(ldd "$private/libsherpa-onnx-c-api.so" | awk '$1 == "libonnxruntime.so" { print $3 }')
echo "libsherpa-onnx-c-api.so: libonnxruntime.so => $resolved"
test "$(realpath -- "$resolved")" = "$(realpath -- "$private/libonnxruntime.so")"

# rpm 给 %%post 的是装上后的实例数（1 为新装，2 及以上为升级），%%preun 是卸下后剩余的实例数（0 为卸载）；换成 dpkg 对应的调用参数。脚本自己保证不让事务失败。
%post
if [ "$1" -ge 2 ]; then
  sh %{_libexecdir}/msime-client/postinst configure upgrade || :
else
  sh %{_libexecdir}/msime-client/postinst configure || :
fi

%preun
if [ "$1" = 0 ]; then
  sh %{_libexecdir}/msime-client/prerm remove || :
fi

# 以 Obsoletes 替换发布页的 msime-linux 时，rpm 先装本包、跑完上面的 %%post，再按卸载移除 msime-linux：它的 %%preun 对每个已登录用户停用 MSIME 的用户单元并运行 msime-linux-setup --unregister（程序这时由本包提供），输入法从 IBus 与 Fcitx5 的列表里消失。移除完成后（$2 是 msime-linux 剩下的实例数）替每个用户以临时单元运行 msime-linux-setup --register，把单元和列表恢复；没配置过的用户它什么也不做。与 %%preun 一样不能让事务失败。
%triggerpostun -- msime-linux
if [ "$2" = 0 ] && command -v loginctl >/dev/null 2>&1 && command -v systemd-run >/dev/null 2>&1; then
  loginctl list-users --no-legend 2>/dev/null | while read -r uid name _; do
    case "$uid" in
      ''|*[!0-9]*) continue ;;
    esac
    systemd-run --user -M "$uid@" --wait --collect --quiet %{_bindir}/msime-linux-setup --register </dev/null >/dev/null 2>&1 ||
      echo "msime: replacing msime-linux took MSIME out of the input method lists of ${name:-uid $uid}; that user should run: msime-linux-setup --register" >&2
  done
fi
:

%files
# CMake 把许可证（copyright）与全部第三方声明装在 doc/msime-client，与 .deb 相同；%%license 另放一份项目许可证到发行版的标准位置。
%license LICENSE
%{_datadir}/doc/msime-client/
%{_bindir}/msime
%{_bindir}/msime-*
%{_bindir}/msime_*.py
%{_libdir}/msime-client/
%{_libdir}/fcitx5/libmsime-fcitx5.so
%{_libexecdir}/msime-client/
%{_datadir}/msime-client/
%{_datadir}/fcitx5/addon/msime.conf
%{_datadir}/fcitx5/inputmethod/msime.conf
%{_datadir}/ibus/component/msime-linux.xml
%{_datadir}/applications/msime-linux.desktop
%{_datadir}/icons/hicolor/*/apps/msime-linux.*
%{_datadir}/systemd/user/msime-linux-*
%config(noreplace) %{_sysconfdir}/xdg/autostart/msime-linux-clipboard.desktop

%changelog
* Mon Oct 05 2026 Metasequoia IME <metasequoiaime@gmail.com> - 0.9.0-1
- Release 0.9.0
