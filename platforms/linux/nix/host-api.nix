# msime-host-api 的 Release cdylib，即 Fcitx5 插件链接的 libmsime_host_api.so。
{
  lib,
  root,
  version,
  craneLib,
  cargoSources,
  pkg-config,
  alsa-lib,
}:
let
  # apps/desktop/src-tauri 是 workspace 成员，Cargo 解析 workspace 时要读它的清单，图标和 Tauri
  # 配置用不到，改它们不必重编这个库。
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      cargoSources
      (craneLib.fileset.commonCargoSources (root + "/apps/desktop/src-tauri"))
    ];
  };
  commonArgs = {
    inherit src version;
    pname = "msime-host-api";
    strictDeps = true;
    # 不带 -p 时会构建 default-members，其中 tauri-mobile-platform 要拉起整套 Tauri 依赖。
    cargoExtraArgs = "--locked -p msime-host-api";
    # Rust 单测由 verify-local.sh 负责；这里的验证是 fcitx5.nix 里链接产物后跑的 ctest。
    doCheck = false;
    nativeBuildInputs = [ pkg-config ];
    # cpal 在 Linux 上经 ALSA 采集麦克风。
    buildInputs = [ alsa-lib ];
  };
  # 只用得到 cargo build 的产物。crane 默认还先跑一遍 cargo check，按 dev-dependencies 另编一套依赖。
  cargoArtifacts = craneLib.buildDepsOnly (commonArgs // { cargoCheckCommand = "true"; });
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    # crate 同时产出 staticlib，crane 默认会把 .a 也装进去；插件只链接 cdylib。
    installPhaseCommand = ''
      install -Dm755 target/release/libmsime_host_api.so -t $out/lib
    '';
  }
)
