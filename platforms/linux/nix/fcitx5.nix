# Linux 原生宿主的 CMake 构建，打开 Fcitx5 插件，可选装入随包词库。IBus engine 等其余
# 入口照常一起构建和安装：顶层 CMake 把 IBus 列为必需，而且 msime-linux-setup、
# msime-linux-prepare 是 Fcitx5 首次配置也要用的。
{
  lib,
  stdenv,
  coreutils,
  procps,
  dbus,
  runtimeShell,
  cmake,
  ninja,
  pkg-config,
  python3,
  wayland-scanner,
  wayland-protocols,
  fcitx5,
  ibus,
  libxkbcommon,
  nlohmann_json,
  curl,
  glib,
  cairo,
  pango,
  wayland,
  libx11,
  libxext,
  libxfixes,
  libxrandr,
  msime-host-api,
  # 随包词库。默认不带，与 Linux 安装包一致，由用户首次配置时 `msime-linux-setup --download`
  # 取回；传入 msime-resources 时装进 share/msime-client/resources 并跑带词库的引擎冒烟。
  msime-resources ? null,
}:
let
  root = ../../..;
  xdgShellDir = "${wayland-protocols}/share/wayland-protocols/stable/xdg-shell";
in
stdenv.mkDerivation {
  pname = "msime-fcitx5";
  version = lib.fileContents ../version.txt;

  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      ../../linux
      ../../common
      (root + "/crates/host-api/include")
      # 契约测试拿 Fcitx5 插件与这两处 Rust 定义对照；只列文件，免得 Rust 改动都触发重编。
      (root + "/crates/engine/src/types.rs")
      (root + "/crates/client-core/src/ai.rs")
      (root + "/resources")
      (root + "/shared")
    ];
  };

  cmakeDir = "../platforms/linux";

  strictDeps = true;
  nativeBuildInputs = [
    cmake
    ninja
    pkg-config
    python3
    wayland-scanner
  ];
  # python3 也放在这里，fixup 阶段才会把安装出去的脚本的 `#!/usr/bin/env python3` 改写到它。
  buildInputs = [
    python3
    fcitx5
    ibus
    libxkbcommon
    nlohmann_json
    curl
    glib
    cairo
    pango
    wayland
    libx11
    libxext
    libxfixes
    libxrandr
  ];

  # 构建沙箱里没有 /usr/bin/env。patchShebangs 改写源码树里可执行脚本的首行；测试在运行时
  # 写出的桩脚本和照抄的 .in 模板里，`#!/usr/bin/env` 是字符串字面量，它改不到，这里换成
  # coreutils 的 env，仍按 PATH 找解释器。
  postPatch = ''
    patchShebangs platforms/linux/scripts platforms/linux/tests platforms/linux/data
    grep -rlZ '#!/usr/bin/env' platforms/linux/tests platforms/linux/data \
      | xargs -0 sed -i 's|#!/usr/bin/env|#!${coreutils}/bin/env|g'
  '';

  cmakeFlags = [
    (lib.cmakeBool "MSIME_ENABLE_FCITX5" true)
    (lib.cmakeFeature "MSIME_HOST_LIBRARY" "${msime-host-api}/lib/libmsime_host_api.so")
    # 两处 find_path 只搜 /usr/share 和 /usr/local/share；找不到时 Wayland 的模式徽章和
    # 语音浮层会被静默跳过，构建照样成功，所以这里显式给出。
    (lib.cmakeFeature "MSIME_XDG_SHELL_DIR" xdgShellDir)
    (lib.cmakeFeature "MSIME_BADGE_XDG_SHELL_DIR" xdgShellDir)
  ]
  ++ lib.optional (msime-resources != null) (
    lib.cmakeFeature "MSIME_ENGINE_RESOURCES" "${msime-resources}"
  );

  doCheck = true;
  # msime-linux-setup 切换词库前用 pgrep 确认宿主进程，setup_update 测试会走到这一步。
  # 带词库时 ibus-page-number-visibility 用 GTestDBus 起一个 dbus-daemon，与门禁镜像装 dbus 的理由相同。
  nativeCheckInputs = [
    procps
    dbus
  ];
  # nixpkgs 的 `dbus-daemon --session` 读 /etc/dbus-1/session.conf，构建沙箱里没有 /etc，
  # linux-ibus-startup-telemetry 起不来总线（它把 stderr 丢了，只报没打出地址）。只在测试期间
  # 垫一层，把 --session 换成包里自带的同一份配置；真机上有这个文件，测试本身不用改。
  preCheck = ''
    mkdir -p "$TMPDIR/dbus-shim"
    cat > "$TMPDIR/dbus-shim/dbus-daemon" <<EOF
    #!${runtimeShell}
    args=()
    for arg in "\$@"; do
      [ "\$arg" = --session ] && arg=--config-file=${dbus}/share/dbus-1/session.conf
      args+=("\$arg")
    done
    exec ${dbus}/bin/dbus-daemon "\''${args[@]}"
    EOF
    chmod +x "$TMPDIR/dbus-shim/dbus-daemon"
    export PATH="$TMPDIR/dbus-shim:$PATH"
  '';

  # ctest 跑的是构建目录，看不到装出去的插件能不能加载。fixup 之后再核对一次：Fcitx5 按插件的
  # RUNPATH 找 Host API，它必须落在本包自己的 lib/msime-client 里。
  doInstallCheck = true;
  installCheckPhase = ''
    runHook preInstallCheck
    resolved=$(ldd $out/lib/fcitx5/libmsime-fcitx5.so | awk '$1 == "libmsime_host_api.so" { print $3 }')
    echo "libmsime_host_api.so => $resolved"
    [[ $(realpath -- "$resolved") == "$out/lib/msime-client/libmsime_host_api.so" ]]
    runHook postInstallCheck
  '';

  meta = {
    description = "水杉输入法的 Fcitx5 插件与 Linux 原生宿主";
    homepage = "https://github.com/metasequoiaime/msime";
    license = lib.licenses.gpl3Only;
    platforms = lib.platforms.linux;
  };
}
