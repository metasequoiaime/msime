# NixOS 模块 programs.msime：把 Fcitx5 插件或 IBus engine、命令行入口和 provider 的 systemd 用户单元接进
# 系统配置。
# linuxPackages 是 flake.nix 里给定 pkgs 返回本平台各包的同一个函数。
{ linuxPackages }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.msime;
  supportedTypes = [
    "fcitx5"
    "ibus"
  ];
in
{
  options.programs.msime = {
    enable = lib.mkEnableOption "水杉输入法（Fcitx5 插件或 IBus engine、命令行入口与 provider 用户服务）";

    package = lib.mkOption {
      type = lib.types.package;
      # 插件被加载进 fcitx5 进程，按本系统的 nixpkgs 构建才与系统上的 Fcitx5 出自同一份。用 IBus 时
      # 取不带 Fcitx5 插件的 msime-ibus。
      default =
        let
          inherit ((linuxPackages pkgs).packages) msime-fcitx5 msime-ibus;
        in
        if config.i18n.inputMethod.type == "ibus" then msime-ibus else msime-fcitx5;
      defaultText = lib.literalMD "按本系统的 nixpkgs 构建的 `msime-fcitx5`；`i18n.inputMethod.type` 是 `ibus` 时为 `msime-ibus`";
      description = "提供 Fcitx5 插件或 IBus engine（`passthru.ibusEngine`）、`msime-linux-setup` 与 provider 用户单元的包。";
    };

    # 默认与 msime-linux-setup 首次配置时为用户启用的单元一致（scripts/msime-linux-setup 的
    # service_units）。这里决定的是系统层面默认拉起哪些单元；首次配置、设置窗口仍会按用户启用它们。
    services = {
      online.enable = lib.mkEnableOption "按 socket 激活的在线候选与翻译服务（msime-linux-online.socket）" // {
        default = true;
      };
      voice.enable = lib.mkEnableOption "按 socket 激活的语音输入服务（msime-linux-voice.socket），本地与云端识别都经过它" // {
        default = true;
      };
      clipboard.enable =
        lib.mkEnableOption "剪贴板历史监视器（msime-linux-clipboard.service），只在偏好里开启剪贴板历史时才采集"
        // {
          default = true;
        };
    };
  };

  config = lib.mkIf cfg.enable {
    # 跟着 i18n.inputMethod.type 接入：两处都填上，nixpkgs 只读当前框架的那一个。默认用 Fcitx5，
    # 但比 mkDefault 低一级：GNOME 模块以 mkDefault 设为 "ibus"，同级的两个值会以冲突报错，
    # 这样 GNOME 用户不设 type 也能求值，并跟着用 IBus。
    i18n.inputMethod = {
      enable = lib.mkDefault true;
      type = lib.mkOverride 1001 "fcitx5";
      fcitx5.addons = [ cfg.package ];
      ibus.engines = [ cfg.package.ibusEngine ];
    };
    # 只接入了 Fcitx5 和 IBus；换成别的输入法框架时模块什么也不提供，说一声而不是静默。
    warnings = lib.optional (!lib.elem config.i18n.inputMethod.type supportedTypes) ''
      programs.msime 只接入了 Fcitx5 和 IBus，i18n.inputMethod.type 是 "${toString config.i18n.inputMethod.type}" 时水杉输入法不会出现在输入法列表里。
    '';
    # 用 Fcitx5 却换上了不带插件的包（msime-ibus），输入法列表里不会有水杉。
    assertions = [
      {
        assertion = config.i18n.inputMethod.type != "fcitx5" || cfg.package.enableFcitx5 or true;
        message = "programs.msime.package 不带 Fcitx5 插件（enableFcitx5 = false），而 i18n.inputMethod.type 是 \"fcitx5\"；换成 msime-fcitx5，或把 type 设为 \"ibus\"。";
      }
    ];
    # 首次配置要用的 msime-linux-setup 等命令；也让剪贴板的 XDG 自启动项（etc/xdg/autostart）生效。
    environment.systemPackages = [ cfg.package ];

    # NixOS 不理会包里单元的 [Install] 段，wantedBy 照单元文件（platforms/linux/data）里的 WantedBy 给出。
    systemd.packages = [ cfg.package ];
    systemd.user.sockets.msime-linux-online.wantedBy =
      lib.optional cfg.services.online.enable "sockets.target";
    systemd.user.sockets.msime-linux-voice.wantedBy =
      lib.optional cfg.services.voice.enable "sockets.target";
    systemd.user.services.msime-linux-clipboard.wantedBy =
      lib.optional cfg.services.clipboard.enable "graphical-session.target";
  };
}
