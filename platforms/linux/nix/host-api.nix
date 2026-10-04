# msime-host-api 的 Release cdylib，即 Fcitx5 插件链接的 libmsime_host_api.so。
{
  lib,
  root,
  version,
  craneLib,
  pkg-config,
  alsa-lib,
}:
let
  # 只放 Cargo 需要的部分，别的平台改动不触发重编。apps/desktop/src-tauri 是 workspace 成员，
  # Cargo 解析 workspace 时要读它的清单，图标和 Tauri 配置用不到。resources 与 shared 里被
  # include_str! 引用的是锁文件、语音模型目录和 shared/contracts，与 Rust 无关的大目录去掉。
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (root + "/Cargo.toml")
      (root + "/Cargo.lock")
      (root + "/crates")
      (craneLib.fileset.commonCargoSources (root + "/apps/desktop/src-tauri"))
      (lib.fileset.difference (root + "/resources") (
        lib.fileset.unions [
          (root + "/resources/eval")
          (root + "/resources/dictionary-sources")
          (root + "/resources/helpcodes")
          (root + "/resources/licenses")
          (root + "/resources/sound-packs")
        ]
      ))
      (root + "/shared/contracts")
    ];
  };
  commonArgs = {
    inherit src version;
    pname = "msime-host-api";
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
    # crate 同时产出 staticlib，crane 默认会把 .a 也装进去；插件只链接 cdylib。
    installPhaseCommand = ''
      install -Dm755 target/release/libmsime_host_api.so -t $out/lib
    '';
  }
)
