# msime-host-api 的 Release cdylib，即 Fcitx5 插件链接的 libmsime_host_api.so。
{
  lib,
  craneLib,
  pkg-config,
  alsa-lib,
}:
let
  root = ../../..;
  # 只放 Cargo 需要的部分，别的平台改动不触发重编。apps/desktop/src-tauri 是 workspace
  # 成员，Cargo 解析 workspace 时要读它的清单；resources 和 shared 被 include_str! 引用。
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (root + "/Cargo.toml")
      (root + "/Cargo.lock")
      (root + "/crates")
      (root + "/apps/desktop/src-tauri")
      (root + "/resources")
      (root + "/shared")
    ];
  };
  commonArgs = {
    inherit src;
    pname = "msime-host-api";
    version = lib.fileContents ../version.txt;
    strictDeps = true;
    # 不带 -p 时会构建 default-members，其中 tauri-mobile-platform 要拉起整套 Tauri 依赖。
    cargoExtraArgs = "--locked -p msime-host-api";
    nativeBuildInputs = [ pkg-config ];
    # cpal 在 Linux 上经 ALSA 采集麦克风。
    buildInputs = [ alsa-lib ];
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    # Rust 单测由 verify-local.sh 负责；这里的验证是 fcitx5.nix 里链接产物后跑的 ctest。
    doCheck = false;
    installPhaseCommand = ''
      install -Dm755 target/release/libmsime_host_api.so -t $out/lib
      install -Dm644 crates/host-api/include/*.h -t $out/include
    '';
  }
)
