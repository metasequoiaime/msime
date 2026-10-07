# 包里几个 Rust 产物（Host API、msime-mcp、设置窗口）静态链接的 crate 的许可证声明，即 CMake 的
# MSIME_RUST_NOTICES。与 package-container.sh 相同，用 collect-notices.py 按 cargo tree 收集；crate 的源码
# 取自 crane 按 Cargo.lock 准备的 vendor 目录，不联网。
{
  lib,
  root,
  version,
  craneLib,
  cargoSources,
  python3,
}:
# collect-notices.py cargo 的 PACKAGE[:FEATURES] 参数。
crates:
craneLib.mkCargoDerivation {
  inherit version;
  pname = "msime-rust-notices";
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      cargoSources
      (craneLib.fileset.commonCargoSources (root + "/apps/desktop/src-tauri"))
      (root + "/platforms/linux/collect-notices.py")
    ];
  };
  cargoArtifacts = null;
  doInstallCargoArtifacts = false;
  nativeBuildInputs = [ python3 ];
  buildPhaseCargoCommand = ''
    python3 platforms/linux/collect-notices.py cargo rust-crates-NOTICES.txt ${lib.escapeShellArgs crates}
  '';
  installPhaseCommand = ''
    install -Dm644 rust-crates-NOTICES.txt $out
  '';
}
