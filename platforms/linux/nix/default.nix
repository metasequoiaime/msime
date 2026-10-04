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

  msime-host-api = pkgs.callPackage ./host-api.nix (common // { inherit craneLib; });
  msime-resources = pkgs.callPackage ./resources.nix { };
  # 需要随包词库时：msime-fcitx5.override { bundledResources = msime-resources; }
  msime-fcitx5 = pkgs.callPackage ./fcitx5.nix (common // { inherit msime-host-api; });
in
{
  packages = { inherit msime-host-api msime-resources msime-fcitx5; };

  devShell = pkgs.mkShell {
    inputsFrom = [
      msime-host-api
      msime-fcitx5
    ];
    # 后几项是 Tauri 外壳（msime-desktop、tauri-mobile-platform）的开发依赖，与
    # platforms/linux/tests/tools/Dockerfile.desktop-check 装的同一组；verify-local.sh 的
    # cargo check 会编到它们。
    packages = with pkgs; [
      rustToolchain
      gtk3
      webkitgtk_4_1
      libsoup_3
      openssl
    ];
  };
}
