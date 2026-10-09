# Linux 原生宿主的 CMake 构建，默认打开 Fcitx5 插件，可选装入随包词库与设置窗口。IBus engine 等其余
# 入口照常一起构建和安装：顶层 CMake 把 IBus 列为必需，而且 msime-linux-setup、
# msime-linux-prepare 是 Fcitx5 首次配置也要用的。
{
  lib,
  root,
  version,
  stdenv,
  runCommand,
  procps,
  dbus,
  cmake,
  ninja,
  pkg-config,
  python3,
  bashNonInteractive,
  makeWrapper,
  wrapGAppsHook3,
  glib-networking,
  wl-clipboard,
  xclip,
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
  # 装进 bin 的 msime-mcp，与 deb、rpm 相同；设置窗口的 MCP 页在自己旁边找它。
  msime-mcp,
  # 随包的几个 Rust 产物静态链接的 crate 的许可证声明（rust-notices.nix），装成
  # share/doc/msime-client/rust-crates-NOTICES.txt。default.nix 默认传入。
  rustNotices ? null,
  # 随包词库。默认不带，与 Linux 安装包一致，由用户首次配置时 `msime-linux-setup --download`
  # 取回；传入 msime-resources 时装进 share/msime-client/resources 并跑带词库的引擎冒烟。
  # 不叫 msime-resources：经 overlay 时 pkgs 里有同名的包，callPackage 会自动填上它。
  bundledResources ? null,
  # 离线手写模型（msime-handwriting-model）。default.nix 默认传入，与各发行版的包一致；
  # 传 null 时不装模型，`msime-linux-handwriting --local` 报告没有安装模型。
  handwritingModel ? null,
  # 本地语音识别用的 sherpa-onnx 运行库（msime-voice-runtime）。default.nix 默认传入，与各发行版的包
  # 一致；传 null 时 msime-voice-local 报告运行库缺失，本地识别不可用，云端识别不受影响。
  voiceRuntime ? null,
  # 设置窗口的 Tauri 二进制（msime-desktop）。default.nix 默认传入；传 null 时没有
  # msime-linux-settings 与桌面入口，插件菜单里打开设置、手写、语音等面板的项都不起作用。
  # 它嵌着的前端的 npm 依赖的许可证声明取自它的 frontendNotices。
  settingsWindow ? null,
  # 构建 Fcitx5 插件。只用 IBus 时关掉（default.nix 的 msime-ibus），包里没有插件，也不依赖 fcitx5；
  # IBus 一侧去不掉：provider 程序都链接 IBus 宿主库。
  enableFcitx5 ? true,
}:
let
  # 安装出去的 provider 脚本用的解释器。豆包流式识别要 websockets 的同步客户端
  # （scripts/msime_voice_doubao.py 按特性检查，不限主版本上限）；其余脚本只用标准库。
  python = python3.withPackages (ps: [ ps.websockets ]);
  # msime-linux-setup 的包装器追加到 XDG_DATA_DIRS 的 IBus schema 数据目录（其下是 glib-2.0/schemas）。
  ibusSchemas = glib.getSchemaDataDirPath ibus;
  glibPath = lib.makeBinPath [ glib ];
  clipboardPath = lib.makeBinPath [
    wl-clipboard
    xclip
  ];
in
stdenv.mkDerivation (finalAttrs: {
  pname = if enableFcitx5 then "msime-fcitx5" else "msime-ibus";
  inherit version;

  # 只放 CMake 构建和测试读到的部分：说明文档和 nix 目录本身的改动不触发重编，
  # resources 与 shared 里与 Linux 宿主无关的目录也去掉。
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (lib.fileset.difference ../../linux (
        lib.fileset.unions [
          ../README.md
          ./.
        ]
      ))
      ../../common
      (root + "/crates/host-api/include")
      # 契约测试拿 Fcitx5 插件与这两处 Rust 定义对照；只列文件，免得 Rust 改动都触发重编。
      (root + "/crates/engine/src/types.rs")
      (root + "/crates/client-core/src/ai.rs")
      (lib.fileset.difference (root + "/resources") (
        lib.fileset.unions [
          (root + "/resources/eval")
          (root + "/resources/dictionary-sources")
          (root + "/resources/voice-models")
        ]
      ))
      (lib.fileset.difference (root + "/shared") (
        lib.fileset.unions [
          (root + "/shared/apple")
          (root + "/shared/apple-bridge")
          (root + "/shared/backend")
          (root + "/shared/backend-ui")
          (root + "/shared/snapshot")
        ]
      ))
    ];
  };

  cmakeDir = "../platforms/linux";

  strictDeps = true;
  nativeBuildInputs = [
    cmake
    ninja
    pkg-config
    python3
    makeWrapper
    wayland-scanner
  ]
  ++ lib.optional (settingsWindow != null) wrapGAppsHook3;
  # python 与 bash 放在这里，patchShebangs --host（postInstall 的与 fixup 自动跑的）才会把装出去的
  # 脚本改写到它们：provider 脚本要带 websockets 的 python，sh、bash 脚本（msime-linux-settings、
  # Omarchy 的钩子等）在 NixOS 上没有 /bin/bash 可用。
  # wayland-protocols 只提供 .pc 和协议 XML，CMake 经 pkg-config 找到其中的 xdg-shell.xml。
  buildInputs = [
    python
    bashNonInteractive
    wayland-protocols
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
  ]
  ++ lib.optional enableFcitx5 fcitx5
  # 设置页发出的 https 请求（tauri.conf.json 的 connect-src）由 WebKit 经 GIO 的 TLS 模块完成。
  ++ lib.optional (settingsWindow != null) glib-networking;

  # 测试会直接执行源码树里的脚本，构建沙箱里没有 /usr/bin/env。
  postPatch = ''
    patchShebangs platforms/linux/scripts platforms/linux/tests platforms/linux/data
  '';
  # 上面改写的是源码树，CMake 从那里装进 bin 的脚本随之指向构建用的 python3 与 bash；fixup 的
  # patchShebangs 不动已经指向 store 的 shebang，所以在这里按宿主的 PATH 重新改写，python 换成带
  # websockets 的那份。由 .in 模板生成的脚本（msime-linux-settings、Omarchy 的钩子）不在源码树里被
  # 改写过，留给 fixup。
  postInstall = ''
    patchShebangs --update --host $out/bin
  '';

  cmakeFlags = [
    (lib.cmakeBool "MSIME_ENABLE_FCITX5" enableFcitx5)
    (lib.cmakeFeature "MSIME_HOST_LIBRARY" "${msime-host-api}/lib/libmsime_host_api.so")
    # NixOS 的 systemd.packages 只从包里的 lib/systemd/user 与 etc/systemd/user 取用户单元，
    # 默认的 share/systemd/user 会被忽略。
    (lib.cmakeFeature "MSIME_SYSTEMD_USER_UNIT_DIR" "lib/systemd/user")
    (lib.cmakeFeature "MSIME_MCP_BINARY" (lib.getExe msime-mcp))
  ]
  # THIRD_PARTY_NOTICES.txt 指向的两份声明。CMake 只在打开 MSIME_ENABLE_PACKAGING 时强制要求它们，
  # 这里没开，漏传只会得到一条警告，所以装后检查核对它们在。
  ++ lib.optional (rustNotices != null) (lib.cmakeFeature "MSIME_RUST_NOTICES" "${rustNotices}")
  ++ lib.optional (bundledResources != null) (
    lib.cmakeFeature "MSIME_ENGINE_RESOURCES" "${bundledResources}/${bundledResources.directory}"
  )
  ++ lib.optional (handwritingModel != null) (
    lib.cmakeFeature "MSIME_HANDWRITING_MODEL_DIR" "${handwritingModel}/${handwritingModel.directory}"
  )
  ++ lib.optional (voiceRuntime != null) (
    lib.cmakeFeature "MSIME_VOICE_RUNTIME_DIR" "${voiceRuntime}"
  )
  # CMake 把它装成 msime-linux-desktop，与 msime-linux-setup、手写模型在同一个前缀下：设置窗口按
  # 自己所在的前缀找这些。
  ++ lib.optionals (settingsWindow != null) [
    (lib.cmakeFeature "MSIME_DESKTOP_BINARY" (lib.getExe settingsWindow))
    (lib.cmakeFeature "MSIME_FRONTEND_NOTICES" settingsWindow.frontendNotices)
  ];

  # 关掉插件只少了 fcitx5 子目录，其余 ctest 与打开时完全相同，已在 msime-fcitx5 里跑过；msime-ibus 特有
  # 的安装布局由下面的装后检查核对。
  doCheck = enableFcitx5;
  # msime-linux-setup 切换词库前用 pgrep 确认宿主进程，setup_update 测试会走到这一步。
  # linux-ibus-startup-telemetry 和带词库时的 ibus-page-number-visibility 要起 dbus-daemon，
  # 与门禁镜像装 dbus 的理由相同。
  nativeCheckInputs = [
    procps
    dbus
  ];

  # 剪贴板监视器在 Wayland 上只靠 wl-paste 取剪贴板（--watch 与读取都是），在 X11 上靠 xclip 或
  # xsel，找不到时一直空转，剪贴板历史什么也记不下。录音、提示音和静音用的音频工具不随包：provider 取 PATH 上找到的第一个
  # （parec、pw-cat、arecord），带上 PulseAudio 的工具会让只有 PipeWire、没开 pipewire-pulse 的
  # 系统选到连不上的 parec，所以交给系统的音频栈。设置窗口的剪贴板面板读写剪贴板也一样：Wayland 上
  # 只经 wl-copy、wl-paste，X11 上只经 xclip、xsel。两者都带上 xclip，xsel 不必再带。
  #
  # wrapGAppsHook3 默认把 bin 下每个可执行文件都包一层，这里只包设置窗口，其余的不用 GTK。包装后真正
  # 的二进制是同目录下的 .msime-linux-desktop-wrapped，它按 current_exe 找前缀，不受影响。
  #
  # msime-linux-setup 经 gsettings 把引擎写进 IBus 的输入源列表（org.freedesktop.ibus.general），但 IBus 的
  # schema 只在 ibus 自己的包装器和 GNOME 会话的 XDG_DATA_DIRS 里，Hyprland 这类会话的 gsettings 找不到它，
  # 注册就退回手动步骤。追加在后面，会话里已有的 schema 优先；gsettings 与 Fcitx5 注册用的 gdbus 同理，
  # PATH 上没有时用 glib 的。
  dontWrapGApps = true;
  postFixup = ''
    wrapProgram $out/bin/msime-linux-clipboard-monitor --prefix PATH : ${clipboardPath}
    wrapProgram $out/bin/msime-linux-setup \
      --suffix XDG_DATA_DIRS : ${ibusSchemas} \
      --suffix PATH : ${glibPath}
  ''
  + lib.optionalString (settingsWindow != null) ''
    wrapGApp $out/bin/msime-linux-desktop --prefix PATH : ${clipboardPath}
  '';

  # ctest 跑的是构建目录，看不到装出去的插件能不能加载。fixup 之后再核对一次：Fcitx5 按插件的
  # RUNPATH 找 Host API，IBus engine 也一样，它必须落在本包自己的 lib/msime-client 里；组件文件的
  # <exec> 要指向本包里能执行的启动脚本。语音运行库同理，另外它的依赖都要
  # 能单独解析：msime-voice-local 自己已经载入了 libstdc++，只看它能否打开运行库发现不了缺依赖。
  # 设置窗口经 fixup 收缩过 RUNPATH，也核对一遍它的 GTK 与 WebKit 依赖都还解析得到，以及包装器给了
  # TLS 模块：缺了它 GIO 只记一条警告，设置页的 https 请求失败。THIRD_PARTY_NOTICES.txt 指向的
  # 几份声明也要真的装进来。msime-linux-setup 包装后还要能运行、带着 IBus 的 schema。
  doInstallCheck = true;
  installCheckPhase = ''
    runHook preInstallCheck
    resolves() {
      local resolved
      resolved=$(ldd "$1" | awk -v name="$2" '$1 == name { print $3 }')
      echo "$2 => $resolved"
      [[ $(realpath -- "$resolved") == "$out/lib/msime-client/$2" ]]
    }
    ${lib.optionalString enableFcitx5 ''
      resolves $out/lib/fcitx5/libmsime-fcitx5.so libmsime_host_api.so
    ''}
    resolves $out/bin/msime-linux-ibus libmsime_host_api.so
    $out/bin/msime-linux-setup --help > /dev/null
    grep -qF ${ibusSchemas} $out/bin/msime-linux-setup
    XDG_DATA_DIRS=${ibusSchemas} ${glibPath}/gsettings list-schemas | grep -qx org.freedesktop.ibus.general
    for component in $out/share/ibus/component/*.xml; do
      [[ -x $(sed -n 's|.*<exec>&quot;\([^&]*\)&quot;.*|\1|p' "$component") ]]
    done
    $out/bin/msime-mcp --version
    ${lib.optionalString (rustNotices != null) ''
      [[ -s $out/share/doc/msime-client/rust-crates-NOTICES.txt ]]
    ''}
    ${lib.optionalString (voiceRuntime != null) ''
      resolves $out/lib/msime-client/libsherpa-onnx-c-api.so libonnxruntime.so
      [[ $(ldd $out/lib/msime-client/libsherpa-onnx-c-api.so $out/lib/msime-client/libonnxruntime.so) != *"not found"* ]]
      python3 ../platforms/linux/tests/voice/local_runtime.py $out/lib/msime-client/msime-voice-local
    ''}
    ${lib.optionalString (settingsWindow != null) ''
      [[ $(ldd $out/bin/.msime-linux-desktop-wrapped) != *"not found"* ]]
      grep -qF ${glib-networking}/lib/gio/modules $out/bin/msime-linux-desktop
      grep -qF ${xclip}/bin $out/bin/msime-linux-desktop
      [[ -s $out/share/doc/msime-client/frontend-npm-NOTICES.txt ]]
    ''}
    runHook postInstallCheck
  '';

  passthru.enableFcitx5 = enableFcitx5;
  # 交给 i18n.inputMethod.ibus.engines 的 IBus engine：只放组件文件，<exec> 的绝对路径指回本包。为什么
  # 不能直接给整个包，见 nix/README.md 的 msime-fcitx5 一节。
  passthru.ibusEngine =
    runCommand "msime-ibus-engine-${finalAttrs.version}"
      {
        meta = {
          inherit (finalAttrs.meta) description homepage platforms;
          isIbusEngine = true;
        };
      }
      ''
        mkdir -p $out/share/ibus/component
        ln -s ${finalAttrs.finalPackage}/share/ibus/component/*.xml $out/share/ibus/component/
      '';

  meta = {
    description =
      if enableFcitx5 then "水杉输入法的 Fcitx5 插件与 Linux 原生宿主" else "水杉输入法的 IBus engine 与 Linux 原生宿主";
    homepage = "https://github.com/metasequoiaime/msime";
    license = [
      lib.licenses.gpl3Only
    ]
    ++ lib.optional (handwritingModel != null) handwritingModel.meta.license
    ++ lib.optionals (voiceRuntime != null) voiceRuntime.meta.license;
    platforms = lib.platforms.linux;
  };
})
