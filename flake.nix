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

      # programs.msime：插件、命令行入口与 provider 用户服务。
      nixosModules.default = import ./platforms/linux/nix/module.nix { inherit linuxPackages; };

      checks = forAllSystems (
        system: pkgs:
        {
          # 构建本身就跑 ctest（doCheck），所以这里只需要列出包。
          inherit (self.packages.${system}) msime-host-api msime-fcitx5;
          # 不用模块的 NixOS 配置经 overlay 取包，按它构建一次：经 overlay 时 callPackage 能从 pkgs 里取到
          # 这几个包，只查 packages 发现不了参数被它们自动填上的问题。
          msime-fcitx5-overlay = (pkgs.extend self.overlays.default).msime-fcitx5;
        }
        # 起一台虚拟机核对模块注册的用户单元真能被 systemd 拉起（需要 KVM）。只注册在 x86_64 上：
        # flake check 只构建本机系统的 checks，却会求值所有系统的，多一个系统就多求值一整套 NixOS
        # （实测约 9 秒、600 MB），哪怕它在这台主机上永远不会被构建。这台虚拟机核对的是模块接线
        # （单元目录、wantedBy、socket 激活、脚本的解释器），与架构无关；aarch64 特有的部分（如语音
        # 运行库）由那边的插件包构建里的 ctest 和装后检查覆盖。
        // lib.optionalAttrs (system == "x86_64-linux") {
          nixos-module = import ./platforms/linux/nix/module-test.nix {
            inherit pkgs;
            module = self.nixosModules.default;
          };
        }
      );

      devShells = lib.mapAttrs (_: linux: { default = linux.devShell; }) perSystem;

      formatter = forAllSystems (_: pkgs: pkgs.nixfmt);
    };
}
