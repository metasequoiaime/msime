# Copyright 2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

# 跟踪 develop 的 live ebuild。按版本发布的 ebuild 由 platforms/linux/packaging/gentoo/render.py 从同目录的 msime.ebuild.in 渲染，构建步骤两份一致，改一份时另一份一起改；步骤本身对应 platforms/linux/package-container.sh。

EAPI=8

PYTHON_COMPAT=( python3_{11..14} )
# 仓库 rust-toolchain.toml 钉住的版本。Gentoo 用系统的 Rust，只能要求不低于它，render.py 与 scripts/test-arch-gentoo-packaging.py 让两处保持一致。
RUST_MIN_VER="1.97.1"

inherit cargo cmake git-r3 optfeature python-single-r1 xdg

DESCRIPTION="Metasequoia IME (水杉输入法): Chinese input method for Fcitx5 and IBus"
HOMEPAGE="https://github.com/metasequoiaime/msime"
EGIT_REPO_URI="https://github.com/metasequoiaime/msime.git"
EGIT_BRANCH="develop"

# 本项目 GPL-3；随包的第三方数据与运行库：sherpa-onnx 与 OpenCC（Apache-2.0）、ONNX Runtime 与 nlohmann/json（MIT）、手写模型与 libchewing-data（LGPL-2.1）、rime-stroke（LGPL-3）、rime-cantonese（CC-BY-4.0）、离线释义（CC-BY-SA-4.0）、libhangul 汉字表（BSD）、行政区划（WTFPL-2）。live ebuild 不逐个列出 crate 的许可证，按版本发布的 ebuild 由 pycargoebuild 补上。
LICENSE="GPL-3 Apache-2.0 BSD CC-BY-4.0 CC-BY-SA-4.0 LGPL-2.1 LGPL-3 MIT WTFPL-2"
SLOT="0"
# 顶层 CMake 把 IBus 列为必需：IBus engine、msime-linux-setup 与 Fcitx5 首次配置用到的 msime-linux-prepare 一起构建，所以只有 Fcitx5 插件是可选的。
IUSE="+fcitx5 test"
REQUIRED_USE="${PYTHON_REQUIRED_USE}"
RESTRICT="!test? ( test )"

COMMON_DEPEND="
	${PYTHON_DEPS}
	app-i18n/ibus
	dev-libs/glib:2
	dev-libs/wayland
	media-libs/alsa-lib
	media-libs/harfbuzz:=
	net-libs/libsoup:3.0
	net-libs/webkit-gtk:4.1
	sys-apps/dbus
	x11-libs/cairo
	x11-libs/gdk-pixbuf:2
	x11-libs/gtk+:3
	x11-libs/libX11
	x11-libs/libXext
	x11-libs/libXfixes
	x11-libs/libXrandr
	x11-libs/libxkbcommon
	x11-libs/pango
	fcitx5? ( app-i18n/fcitx:5 )
"
# msime-linux-setup 切换词库前用 pgrep 确认宿主进程。
RDEPEND="
	${COMMON_DEPEND}
	sys-process/procps
"
DEPEND="
	${COMMON_DEPEND}
	dev-cpp/nlohmann_json
	dev-libs/wayland-protocols
	net-misc/curl
"
# 测试里 msime-linux-setup 的用例要 pgrep，IBus 遥测与翻页用例要起 dbus-daemon。
BDEPEND="
	${PYTHON_DEPS}
	dev-util/wayland-scanner
	net-libs/nodejs[npm]
	virtual/pkgconfig
	test? (
		sys-apps/dbus
		sys-process/procps
	)
"

# 锁文件钉住的 sherpa-onnx 预编译运行库，原样装进私有目录。
QA_PREBUILT="usr/lib*/msime-client/libonnxruntime.so usr/lib*/msime-client/libsherpa-onnx-c-api.so"

CMAKE_USE_DIR="${S}/platforms/linux"

pkg_setup() {
	python-single-r1_pkg_setup
	rust_pkg_setup
}

_msime_voice_platform() {
	case ${ARCH} in
		amd64) echo linux-x86_64 ;;
		arm64) echo linux-aarch64 ;;
		*) die "no pinned voice runtime for ${ARCH}" ;;
	esac
}

src_unpack() {
	git-r3_src_unpack
	cargo_live_src_unpack

	# 下面几步都要联网，Portage 只在 live ebuild 的 src_unpack 里放行网络。pnpm 取 package.json 的 packageManager 钉住的版本。
	cd "${S}" || die
	local pnpm_spec
	pnpm_spec=$(sed -n 's/.*"packageManager": *"\(pnpm@[^"]*\)".*/\1/p' package.json)
	[[ -n ${pnpm_spec} ]] || die "package.json pins no pnpm version"
	echo "${pnpm_spec}" > "${WORKDIR}/pnpm-spec" || die
	export npm_config_cache="${WORKDIR}/npm-cache"
	npx --yes "${pnpm_spec}" install --frozen-lockfile --store-dir "${WORKDIR}/pnpm-store" || die

	# 随包的锁定资源，脚本按 resources/*.lock.json 的地址下载并核对 SHA-256。
	"${EPYTHON}" scripts/fetch_voice_runtime.py --platform "$(_msime_voice_platform)" --out "${WORKDIR}/voice-runtime" || die
	"${EPYTHON}" scripts/fetch_handwriting_model.py --out "${WORKDIR}/handwriting-model" || die
	"${EPYTHON}" scripts/fetch_offline_glosses.py --out "${WORKDIR}/offline-glosses" || die
	"${EPYTHON}" scripts/fetch_language_dictionaries.py --out "${WORKDIR}/language-dictionaries" || die
}

src_prepare() {
	cmake_src_prepare
}

src_configure() {
	# CMake 配置要求 Host API 已经编好，放到 src_compile 里 Cargo 构建之后。
	:
}

src_compile() {
	cargo_src_compile -p msime-host-api
	cargo_src_compile -p msime-mcp-server --bin msime-mcp

	# 桌面二进制在编译时嵌入前端。tauri/custom-protocol 是 `tauri build` 打开的特性，缺了它会得到加载 devUrl 的开发版；TAURI_CONFIG 设定应用报告的版本。
	local version
	version=$(<platforms/linux/version.txt)
	version=${version//[[:space:]]/}
	export npm_config_cache="${WORKDIR}/npm-cache" npm_config_offline=true
	npx --offline "$(<"${WORKDIR}/pnpm-spec")" --filter @msime/desktop build || die
	TAURI_CONFIG="{\"version\":\"${version}\"}" \
		cargo_src_compile -p msime-desktop --bin msime-desktop --features tauri/custom-protocol

	# 静态链接进二进制的 crate 与打进前端的 npm 包的许可证，打包配置要求两者都在。
	mkdir -p "${WORKDIR}/notices" || die
	"${EPYTHON}" platforms/linux/collect-notices.py npm "${WORKDIR}/notices/frontend-npm-NOTICES.txt" apps/desktop || die
	cargo_env "${EPYTHON}" platforms/linux/collect-notices.py cargo "${WORKDIR}/notices/rust-crates-NOTICES.txt" \
		msime-host-api msime-mcp-server msime-desktop:tauri/custom-protocol || die

	local target="${S}/$(cargo_target_dir)"
	# MSIME_ENABLE_PACKAGING 打开与发布包相同的校验（前缀必须是 /usr、第三方声明齐全），不跑 CPack。
	local mycmakeargs=(
		-DBUILD_TESTING=$(usex test)
		-DMSIME_ENABLE_PACKAGING=ON
		-DMSIME_ENABLE_FCITX5=$(usex fcitx5)
		-DMSIME_HOST_LIBRARY="${target}/libmsime_host_api.so"
		-DMSIME_MCP_BINARY="${target}/msime-mcp"
		-DMSIME_DESKTOP_BINARY="${target}/msime-desktop"
		-DMSIME_RUST_NOTICES="${WORKDIR}/notices/rust-crates-NOTICES.txt"
		-DMSIME_FRONTEND_NOTICES="${WORKDIR}/notices/frontend-npm-NOTICES.txt"
		-DMSIME_VOICE_RUNTIME_DIR="${WORKDIR}/voice-runtime"
		-DMSIME_HANDWRITING_MODEL_DIR="${WORKDIR}/handwriting-model"
		-DMSIME_OFFLINE_GLOSSES="${WORKDIR}/offline-glosses"
		-DMSIME_LANGUAGE_DICTIONARIES="${WORKDIR}/language-dictionaries"
		-DMSIME_REQUIRE_LANGUAGE_DICTIONARIES=ON
	)
	cmake_src_configure
	cmake_src_compile
}

src_test() {
	# 与拉取请求门禁和发布构建跑的是同一套 ctest。
	cmake_src_test
}

src_install() {
	cmake_src_install
	python_fix_shebang "${ED}"/usr/bin

	# ctest 跑的是构建目录，看不到装出去的插件能不能加载。Fcitx5 按插件的 RUNPATH 找 Host API，它必须落在本包自己的 msime-client 目录里。
	if use fcitx5; then
		local libdir="${ED}/usr/$(get_libdir)"
		local resolved
		resolved=$(ldd "${libdir}/fcitx5/libmsime-fcitx5.so" | awk '$1 == "libmsime_host_api.so" { print $3 }')
		einfo "libmsime_host_api.so => ${resolved}"
		[[ $(realpath -- "${resolved}") == "${libdir}/msime-client/libmsime_host_api.so" ]] ||
			die "the Fcitx5 addon does not resolve the Host API in its own package"
	fi
}

# 以下两段对应 .deb 的 postinst/prerm（platforms/linux/cmake/deb-postinst.in、deb-prerm.in）。单元列表与 platforms/linux/CMakeLists.txt 的 MSIME_USER_UNITS 一致；没有 systemd（OpenRC）或装到别的 ROOT 时什么也不做，任何失败都不影响安装。
_msime_units="msime-linux-online.socket msime-linux-online.service msime-linux-voice.socket msime-linux-voice.service msime-linux-clipboard.service"
_msime_services="msime-linux-online.service msime-linux-voice.service msime-linux-clipboard.service"

_msime_users() {
	[[ -z ${ROOT} ]] || return 1
	command -v systemctl >/dev/null 2>&1 || return 1
	command -v loginctl >/dev/null 2>&1 || return 1
	loginctl list-users --no-legend 2>/dev/null
}

pkg_postinst() {
	xdg_pkg_postinst

	# 已在运行的用户服务继续执行旧程序：重新加载每个用户的 systemd 用户管理器、补上匿名翻译账号，并重启正在运行的 MSIME 服务。
	local users uid name unit
	if users=$(_msime_users); then
		while read -r uid name _; do
			[[ ${uid} =~ ^[0-9]+$ ]] || continue
			if systemctl --user -M "${uid}@" daemon-reload </dev/null >/dev/null 2>&1; then
				if command -v systemd-run >/dev/null 2>&1; then
					systemd-run --user -M "${uid}@" --wait --collect --quiet "${EPREFIX}"/usr/bin/msime-linux-online-provider --ensure-anonymous-account </dev/null >/dev/null 2>&1
				fi
				for unit in ${_msime_services}; do
					systemctl --user -M "${uid}@" try-restart "${unit}" </dev/null >/dev/null 2>&1
				done
			else
				ewarn "could not reach the systemd user manager of ${name:-uid ${uid}}; that user should restart its services: systemctl --user daemon-reload && systemctl --user try-restart ${_msime_services}"
			fi
		done <<< "${users}"
	fi

	if [[ -z ${REPLACING_VERSIONS} ]]; then
		elog "每个用户首次使用前运行一次 msime-linux-setup --download 取回词库（约 170 MB），"
		elog "再在 Fcitx5 或 IBus 的输入法列表里添加水杉输入法。"
	fi
	optfeature "Doubao streaming speech recognition" dev-python/websockets
	optfeature "voice recording" media-libs/libpulse media-video/pipewire media-sound/alsa-utils
}

pkg_prerm() {
	# 升级时旧版本的 pkg_prerm 也会跑，那时单元保持运行，由新版本的 pkg_postinst 重启。
	[[ -n ${REPLACED_BY_VERSION} ]] && return
	local users uid name unit
	users=$(_msime_users) || return
	while read -r uid name _; do
		[[ ${uid} =~ ^[0-9]+$ ]] || continue
		if systemctl --user -M "${uid}@" show --property=Version </dev/null >/dev/null 2>&1; then
			for unit in ${_msime_units}; do
				systemctl --user -M "${uid}@" disable --now "${unit}" </dev/null >/dev/null 2>&1
			done
			if command -v systemd-run >/dev/null 2>&1; then
				systemd-run --user -M "${uid}@" --wait --collect --quiet "${EPREFIX}"/usr/bin/msime-linux-setup --unregister </dev/null >/dev/null 2>&1
			fi
		else
			ewarn "could not reach the systemd user manager of ${name:-uid ${uid}}; that user should stop its services: systemctl --user disable --now ${_msime_units}; and remove the input method from its lists"
		fi
	done <<< "${users}"
}
