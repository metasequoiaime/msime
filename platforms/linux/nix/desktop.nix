# 设置窗口：Tauri 外壳 msime-desktop 的 Release 二进制，前端在编译时嵌进去。步骤与
# package-container.sh 相同：先构建前端，再带 tauri/custom-protocol 编译。装进前缀与 GTK 包装在 fcitx5.nix。
{
  lib,
  root,
  version,
  craneLib,
  cargoSources,
  callPackage,
  stdenvNoCC,
  python3,
  nodejs_24,
  pnpm_11,
  writableTmpDirAsHomeHook,
  pkg-config,
  alsa-lib,
  dbus,
  glib,
  gtk3,
  libsoup_3,
  webkitgtk_4_1,
}:
let
  # 前端嵌进二进制，所以它的 npm 依赖的许可证声明（notices 输出）要随包装出去。
  frontend = stdenvNoCC.mkDerivation {
    pname = "msime-desktop-frontend";
    inherit version;
    outputs = [
      "out"
      "notices"
    ];

    # 设置页只由 apps/desktop 与 packages/ui 构成，测试用不到，改它们不必重编。pnpm 按
    # pnpm-workspace.yaml 核对锁文件，所以另一个 workspace 成员 apps/harmony 的清单也要在。
    src = lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        (root + "/package.json")
        (root + "/pnpm-lock.yaml")
        (root + "/pnpm-workspace.yaml")
        (root + "/apps/harmony/package.json")
        (lib.fileset.difference (root + "/apps/desktop") (
          lib.fileset.unions [
            (root + "/apps/desktop/src-tauri")
            (root + "/apps/desktop/tests")
          ]
        ))
        (root + "/packages/ui")
        (root + "/platforms/linux/collect-notices.py")
      ];
    };

    nativeBuildInputs = [
      # 与 .nvmrc 同一大版本。
      nodejs_24
      pnpm_11
      writableTmpDirAsHomeHook
      python3
    ];

    # 依赖按锁文件里每个包的 integrity 逐个下载（pnpm-lock.nix），锁文件变了不用另改哈希。
    # pnpm 11 会按 packageManager 字段核对自己的版本，这里用的就是 nixpkgs 的 pnpm_11，不让它去
    # 下载别的版本。改写后的 resolution 指向本地 tarball，pnpm 11 的供应链策略会拒绝这种锁文件；
    # 每个 tarball 已由 fetchurl 按锁文件里的 integrity 核对过，所以信任锁文件。
    env = {
      pnpm_config_pm_on_fail = "ignore";
      pnpm_config_trust_lockfile = "true";
      pnpm_config_update_notifier = "false";
    };
    configurePhase = ''
      runHook preConfigure
      cp --no-preserve=mode ${
        callPackage ./pnpm-lock.nix { } { lockfile = root + "/pnpm-lock.yaml"; }
      } pnpm-lock.yaml
      pnpm install --offline --frozen-lockfile --ignore-scripts --filter '@msime/desktop...' \
        --store-dir "$TMPDIR/pnpm-store"
      patchShebangs node_modules
      runHook postConfigure
    '';

    # 包的 build 脚本先跑 tsc 检查类型（连同测试），它不影响产物，交给 verify-local.sh。
    buildPhase = ''
      runHook preBuild
      pnpm --filter @msime/desktop exec vite build
      runHook postBuild
    '';

    # 与 package-container.sh 相同，用 collect-notices.py 从装好的 node_modules 收集。
    installPhase = ''
      runHook preInstall
      cp -r apps/desktop/dist $out
      python3 platforms/linux/collect-notices.py npm $notices/frontend-npm-NOTICES.txt apps/desktop
      runHook postInstall
    '';
  };

  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      cargoSources
      # 图标、Tauri 配置与权限声明都在编译时读；gen 下是 Android 与 Xcode 的生成工程，用不到。
      (lib.fileset.difference (root + "/apps/desktop/src-tauri") (root + "/apps/desktop/src-tauri/gen"))
    ];
  };
  commonArgs = {
    inherit src version;
    pname = "msime-desktop";
    strictDeps = true;
    # tauri/custom-protocol 是 `tauri build` 打开的特性，缺了它会得到加载 devUrl 的开发版。
    cargoExtraArgs = "--locked -p msime-desktop --bin msime-desktop --features tauri/custom-protocol";
    # Rust 单测由 verify-local.sh 负责。
    doCheck = false;
    nativeBuildInputs = [ pkg-config ];
    # dbus 来自 tao；alsa-lib 来自经 msime-host-api 链进来的 cpal；gtk3 也是对话框插件（rfd）要的。
    buildInputs = [
      alsa-lib
      dbus
      glib
      gtk3
      libsoup_3
      webkitgtk_4_1
    ];
  };
  # 只用得到 cargo build 的产物。crane 默认还先跑一遍 cargo check，那会另编一套 Tauri。
  cargoArtifacts = craneLib.buildDepsOnly (commonArgs // { cargoCheckCommand = "true"; });
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    # 应用报告的版本取 Linux 平台的版本号，与发布包相同，应用内的更新检查才是同类相比。
    env.TAURI_CONFIG = builtins.toJSON { inherit version; };
    preBuild = ''
      cp -r --no-preserve=mode ${frontend} apps/desktop/dist
    '';
    installPhaseCommand = ''
      install -Dm755 target/release/msime-desktop -t $out/bin
    '';
    # fcitx5.nix 把它交给 CMake 的 MSIME_FRONTEND_NOTICES，装在 THIRD_PARTY_NOTICES.txt 旁边。
    passthru.frontendNotices = "${frontend.notices}/frontend-npm-NOTICES.txt";
    meta.mainProgram = "msime-desktop";
  }
)
