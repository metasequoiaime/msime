{
  description = "水杉输入法（MSIME）的 Linux 原生宿主：Fcitx5 插件与它链接的 Host API";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # 实际的构建写在 platforms/linux/nix/ 下；这里只把它们接到 flake 的各个输出上。
  # flake.nix 必须放在仓库根目录，因为 Cargo workspace 和 rust-toolchain.toml 都在这里。
  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: lib.genAttrs systems (system: f system nixpkgs.legacyPackages.${system});
      linuxPackages = pkgs: import ./platforms/linux/nix { inherit pkgs crane rust-overlay; };
      # 每个系统只求值一次，packages 与 devShells 共用。
      perSystem = forAllSystems (_: linuxPackages);
    in
    {
      packages = lib.mapAttrs (
        _: linux: linux.packages // { default = linux.packages.msime-fcitx5; }
      ) perSystem;

      # 插件被加载进 fcitx5 进程，应当和系统上运行的 Fcitx5 出自同一份 nixpkgs。
      # 用这个 overlay 时包按使用方的 nixpkgs 构建；直接取 packages 则用本 flake 锁定的那份。
      overlays.default = final: _prev: (linuxPackages final).packages;

      checks = forAllSystems (
        system: pkgs: {
          # 构建本身就跑 ctest（doCheck），所以这里只需要列出包。
          inherit (self.packages.${system}) msime-host-api msime-fcitx5;
          # NixOS 配置用的是 overlay，按它构建一次：经 overlay 时 callPackage 能从 pkgs 里取到
          # 这几个包，只查 packages 发现不了参数被它们自动填上的问题。
          msime-fcitx5-overlay = (pkgs.extend self.overlays.default).msime-fcitx5;
        }
      );

      devShells = lib.mapAttrs (_: linux: { default = linux.devShell; }) perSystem;

      formatter = forAllSystems (_: pkgs: pkgs.nixfmt);
    };
}
