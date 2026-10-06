# msime-mcp-server 的 Release 二进制 msime-mcp：设置窗口的 MCP 页在自己旁边找它，助手的 MCP 配置经
# PATH 调用它。与 deb、rpm 一样由 fcitx5.nix 交给 CMake 的 MSIME_MCP_BINARY 装进同一前缀。
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
  # 与 host-api.nix 相同：只要 apps/desktop/src-tauri 的清单和 .rs，供 Cargo 解析 workspace。
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      cargoSources
      (craneLib.fileset.commonCargoSources (root + "/apps/desktop/src-tauri"))
    ];
  };
  commonArgs = {
    inherit src version;
    pname = "msime-mcp";
    strictDeps = true;
    cargoExtraArgs = "--locked -p msime-mcp-server --bin msime-mcp";
    # Rust 单测由 verify-local.sh 负责。
    doCheck = false;
    nativeBuildInputs = [ pkg-config ];
    # 经 msime-host-api 链进来的 cpal 要 ALSA。
    buildInputs = [ alsa-lib ];
  };
  cargoArtifacts = craneLib.buildDepsOnly (commonArgs // { cargoCheckCommand = "true"; });
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    # build.rs 按 platforms/linux/version.txt 报告版本，源码里没有它，与发布脚本一样显式给出。
    env.MSIME_VERSION = version;
    installPhaseCommand = ''
      install -Dm755 target/release/msime-mcp -t $out/bin
    '';
    meta.mainProgram = "msime-mcp";
  }
)
