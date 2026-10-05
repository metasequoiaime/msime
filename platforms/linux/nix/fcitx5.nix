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
}:
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
    wayland-scanner
  ];
  # python3 也放在这里，fixup 阶段才会把安装出去的脚本的 `#!/usr/bin/env python3` 改写到它。
  # wayland-protocols 只提供 .pc 和协议 XML，CMake 经 pkg-config 找到其中的 xdg-shell.xml。
  buildInputs = [
    python3
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

  cmakeFlags = [
    (lib.cmakeBool "MSIME_ENABLE_FCITX5" true)
    (lib.cmakeFeature "MSIME_HOST_LIBRARY" "${msime-host-api}/lib/libmsime_host_api.so")
  ]
  ++ lib.optional (bundledResources != null) (
    lib.cmakeFeature "MSIME_ENGINE_RESOURCES" "${bundledResources}"
  )
  ++ lib.optional (handwritingModel != null) (
    lib.cmakeFeature "MSIME_HANDWRITING_MODEL_DIR" "${handwritingModel}"
  );

  doCheck = true;
  # msime-linux-setup 切换词库前用 pgrep 确认宿主进程，setup_update 测试会走到这一步。
  # linux-ibus-startup-telemetry 和带词库时的 ibus-page-number-visibility 要起 dbus-daemon，
  # 与门禁镜像装 dbus 的理由相同。
  nativeCheckInputs = [
    procps
    dbus
  ];

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
    license = [
      lib.licenses.gpl3Only
    ]
    ++ lib.optional (handwritingModel != null) handwritingModel.meta.license;
    platforms = lib.platforms.linux;
  };
}
