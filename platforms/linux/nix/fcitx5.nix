# Linux 原生宿主的 CMake 构建，打开 Fcitx5 插件，可选装入随包词库。IBus engine 等其余
# 入口照常一起构建和安装：顶层 CMake 把 IBus 列为必需，而且 msime-linux-setup、
# msime-linux-prepare 是 Fcitx5 首次配置也要用的。
{
  lib,
  root,
  version,
  stdenv,
  procps,
  dbus,
  cmake,
  ninja,
  pkg-config,
  python3,
  makeWrapper,
  wl-clipboard,
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
  # 不叫 msime-resources：经 overlay 时 pkgs 里有同名的包，callPackage 会自动填上它。
  bundledResources ? null,
  # 离线手写模型（msime-handwriting-model）。default.nix 默认传入，与各发行版的包一致；
  # 传 null 时不装模型，`msime-linux-handwriting --local` 报告没有安装模型。
  handwritingModel ? null,
  # 本地语音识别用的 sherpa-onnx 运行库（msime-voice-runtime）。default.nix 默认传入，与各发行版的包
  # 一致；传 null 时 msime-voice-local 报告运行库缺失，本地识别不可用，云端识别不受影响。
  voiceRuntime ? null,
}:
let
  # 安装出去的 provider 脚本用的解释器。豆包流式识别要 websockets 的同步客户端
  # （scripts/msime_voice_doubao.py 按特性检查，不限主版本上限）；其余脚本只用标准库。
  python = python3.withPackages (ps: [ ps.websockets ]);
in
stdenv.mkDerivation {
  pname = "msime-fcitx5";
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
  ];
  # python 放在这里，postInstall 的 patchShebangs --host 才会把装出去的脚本改写到它。
  # wayland-protocols 只提供 .pc 和协议 XML，CMake 经 pkg-config 找到其中的 xdg-shell.xml。
  buildInputs = [
    python
    wayland-protocols
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

  # 测试会直接执行源码树里的脚本，构建沙箱里没有 /usr/bin/env。
  postPatch = ''
    patchShebangs platforms/linux/scripts platforms/linux/tests platforms/linux/data
  '';
  # 上面改写的是源码树，装出去的脚本随之指向构建用的 python3；fixup 的 patchShebangs 不动已经指向
  # store 的 shebang，所以在这里按宿主的 PATH 重新改写，换成带 websockets 的 python。
  postInstall = ''
    patchShebangs --update --host $out/bin
  '';

  cmakeFlags = [
    (lib.cmakeBool "MSIME_ENABLE_FCITX5" true)
    (lib.cmakeFeature "MSIME_HOST_LIBRARY" "${msime-host-api}/lib/libmsime_host_api.so")
    # NixOS 的 systemd.packages 只从包里的 lib/systemd/user 与 etc/systemd/user 取用户单元，
    # 默认的 share/systemd/user 会被忽略。
    (lib.cmakeFeature "MSIME_SYSTEMD_USER_UNIT_DIR" "lib/systemd/user")
  ]
  ++ lib.optional (bundledResources != null) (
    lib.cmakeFeature "MSIME_ENGINE_RESOURCES" "${bundledResources}/${bundledResources.directory}"
  )
  ++ lib.optional (handwritingModel != null) (
    lib.cmakeFeature "MSIME_HANDWRITING_MODEL_DIR" "${handwritingModel}/${handwritingModel.directory}"
  )
  ++ lib.optional (voiceRuntime != null) (
    lib.cmakeFeature "MSIME_VOICE_RUNTIME_DIR" "${voiceRuntime}"
  );

  doCheck = true;
  # msime-linux-setup 切换词库前用 pgrep 确认宿主进程，setup_update 测试会走到这一步。
  # linux-ibus-startup-telemetry 和带词库时的 ibus-page-number-visibility 要起 dbus-daemon，
  # 与门禁镜像装 dbus 的理由相同。
  nativeCheckInputs = [
    procps
    dbus
  ];

  # 剪贴板监视器在 Wayland 上只靠 wl-paste 取剪贴板（--watch 与读取都是），找不到它时一直空转，
  # 剪贴板历史什么也记不下。录音、提示音和静音用的音频工具不随包：provider 取 PATH 上找到的第一个
  # （parec、pw-cat、arecord），带上 PulseAudio 的工具会让只有 PipeWire、没开 pipewire-pulse 的
  # 系统选到连不上的 parec，所以交给系统的音频栈。
  postFixup = ''
    wrapProgram $out/bin/msime-linux-clipboard-monitor --prefix PATH : ${
      lib.makeBinPath [ wl-clipboard ]
    }
  '';

  # ctest 跑的是构建目录，看不到装出去的插件能不能加载。fixup 之后再核对一次：Fcitx5 按插件的
  # RUNPATH 找 Host API，它必须落在本包自己的 lib/msime-client 里。语音运行库同理，另外它的依赖都要
  # 能单独解析：msime-voice-local 自己已经载入了 libstdc++，只看它能否打开运行库发现不了缺依赖。
  doInstallCheck = true;
  installCheckPhase = ''
    runHook preInstallCheck
    resolves() {
      local resolved
      resolved=$(ldd "$1" | awk -v name="$2" '$1 == name { print $3 }')
      echo "$2 => $resolved"
      [[ $(realpath -- "$resolved") == "$out/lib/msime-client/$2" ]]
    }
    resolves $out/lib/fcitx5/libmsime-fcitx5.so libmsime_host_api.so
    ${lib.optionalString (voiceRuntime != null) ''
      resolves $out/lib/msime-client/libsherpa-onnx-c-api.so libonnxruntime.so
      [[ $(ldd $out/lib/msime-client/libsherpa-onnx-c-api.so $out/lib/msime-client/libonnxruntime.so) != *"not found"* ]]
      python3 ../platforms/linux/tests/voice/local_runtime.py $out/lib/msime-client/msime-voice-local
    ''}
    runHook postInstallCheck
  '';

  meta = {
    description = "水杉输入法的 Fcitx5 插件与 Linux 原生宿主";
    homepage = "https://github.com/metasequoiaime/msime";
    license = [
      lib.licenses.gpl3Only
    ]
    ++ lib.optional (handwritingModel != null) handwritingModel.meta.license
    ++ lib.optionals (voiceRuntime != null) voiceRuntime.meta.license;
    platforms = lib.platforms.linux;
  };
}
