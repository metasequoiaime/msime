# 给定一套 nixpkgs，返回 Linux 平台的几个包和开发 shell。flake 的 packages 与 overlay 共用这一份，
# 区别只在传进来的是本 flake 锁定的 nixpkgs 还是使用方自己的。
{
  pkgs,
  crane,
  rust-overlay,
}:
let
  # 与 rust-toolchain.toml 钉住的版本一致；nixpkgs 自带的 rustc 版本随 channel 漂移，
  # 用它就违背了「六个平台和本地门禁用同一个编译器」的约定。
  rustToolchain =
    (rust-overlay.lib.mkRustBin { } pkgs).fromRustupToolchainFile
      ../../../rust-toolchain.toml;
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);

  # 仓库根目录与本平台版本号，几个包共用。
  common = {
    root = ../../..;
    version = pkgs.lib.fileContents ../version.txt;
  };

  # 两个 Rust 包共用的源码：只放 Cargo 需要的部分，别的平台改动不触发重编。resources 与 shared 里被
  # include_str! 引用的是锁文件、语音模型目录和 shared/contracts，与 Rust 无关的大目录去掉。
  # apps/desktop/src-tauri 由各包按需要的范围自己加。
  cargoSources =
    let
      inherit (common) root;
    in
    pkgs.lib.fileset.unions [
      (root + "/Cargo.toml")
      (root + "/Cargo.lock")
      (root + "/crates")
      (pkgs.lib.fileset.difference (root + "/resources") (
        pkgs.lib.fileset.unions [
          (root + "/resources/eval")
          (root + "/resources/dictionary-sources")
          (root + "/resources/helpcodes")
          (root + "/resources/licenses")
          (root + "/resources/sound-packs")
        ]
      ))
      (root + "/shared/contracts")
    ];
  rustArgs = common // {
    inherit craneLib cargoSources;
  };

  msime-host-api = pkgs.callPackage ./host-api.nix rustArgs;
  msime-desktop = pkgs.callPackage ./desktop.nix rustArgs;
  lockedArtifacts = pkgs.callPackage ./locked-artifacts.nix { };
  # desktop-dictionary.lock.json 钉住的词库，默认不随包（见 fcitx5.nix 的 bundledResources）。锁里只有
  # SCOWL 与 Mozc 的许可文本，逐项列出词库数据来源与上游条款的 NOTICE 是仓库里的固定副本，与插件包
  # 里 CMake 装的同名。
  msime-resources = lockedArtifacts {
    name = "msime-resources";
    lock = ../../../resources/desktop-dictionary.lock.json;
    directory = "share/msime-client/resources";
    notices."msime-engine-dictionary-NOTICE.md" =
      ../../../resources/licenses/msime-engine-dictionary-NOTICE.md;
  };
  # `msime-linux-handwriting --local` 用的 Zinnia 模型和它的许可证（锁里的第二个 artifact）；许可证与
  # debian/copyright 的记载一致。
  msime-handwriting-model = lockedArtifacts {
    name = "msime-handwriting-model";
    lock = ../../../resources/handwriting-model.lock.json;
    directory = "share/msime-client/handwriting";
    meta.license = pkgs.lib.licenses.lgpl21Only;
  };
  msime-voice-runtime = pkgs.callPackage ./voice-runtime.nix { };
  # 需要随包词库时：msime-fcitx5.override { bundledResources = msime-resources; }
  msime-fcitx5 = pkgs.callPackage ./fcitx5.nix (
    common
    // {
      inherit msime-host-api;
      settingsWindow = msime-desktop;
      handwritingModel = msime-handwriting-model;
      # 锁里没有本机架构的运行库时（如 riscv64、i686）不带它，退回只有云端识别的构建，而不是求值失败。
      voiceRuntime =
        if pkgs.lib.meta.availableOn pkgs.stdenv.hostPlatform msime-voice-runtime then
          msime-voice-runtime
        else
          null;
    }
  );
in
{
  packages = {
    inherit
      msime-host-api
      msime-desktop
      msime-resources
      msime-handwriting-model
      msime-voice-runtime
      msime-fcitx5
      ;
  };

  devShell = pkgs.mkShell {
    # msime-desktop 带进 Tauri 外壳（msime-desktop、tauri-mobile-platform）要的 GTK 与 WebKit，
    # verify-local.sh 的 cargo check 会编到它们。
    inputsFrom = [
      msime-host-api
      msime-desktop
      msime-fcitx5
    ];
    packages = [ rustToolchain ];
  };
}
