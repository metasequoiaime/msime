# Agent Note: Nix overlay 直配启用语音服务

Status: implemented

## Problem

NixOS 的 overlay 直配示例只写了 `systemd.packages`。这会把 `msime-linux-online.socket`、`msime-linux-voice.socket` 和 `msime-linux-clipboard.service` 的 unit 文件注册到用户 systemd，但 NixOS 不会执行 unit 文件 `[Install]` 段里的 `WantedBy`，因此语音 socket 没有被拉起。用户能看到录音设备设置，却无法从语音浮窗开始录音；只用 `systemctl --user` 检查时也容易误以为服务已接入。

模块路径已经在 `platforms/linux/nix/module.nix` 中显式配置这些 `wantedBy`，问题只出现在不使用模块、通过 `overlays.default` 自己组合包的配置。模块的接线背景见 [Nix 的 IBus 接入](../feature/2026-10-08-nixos-module-ibus.md)。

## Decision

`platforms/linux/README.md` 的 Fcitx5 和 IBus overlay 直配示例都显式配置：

- `msime-linux-online.socket` 和 `msime-linux-voice.socket` 的 `wantedBy = [ "sockets.target" ]`；
- `msime-linux-clipboard.service` 的 `wantedBy = [ "graphical-session.target" ]`。

说明同时保留 `systemd.packages` 的作用：它负责注册包内 unit 文件，`wantedBy` 负责把 unit 接入用户会话的启动目标。这样不使用 `programs.msime` 模块的用户也能获得与模块配置相同的在线、语音和剪贴板服务接线；服务自身的偏好开关仍由 unit 和运行时逻辑决定。

## Alternatives considered

- **只在 issue 中说明用户手动执行 `systemctl --user enable`**：能救回单个系统，但把 NixOS 声明式配置变成部署后的手工状态，重建系统或垃圾回收后仍可能丢失，因此不选。
- **修改包内 unit 的 `[Install]` 段，期待 `systemd.packages` 自动启用**：unit 已经声明了对应目标，但 NixOS 的 `systemd.packages` 只注册 unit，不执行 `[Install]`；继续改 unit 文本不能改变 NixOS 的注册语义，因此不选。
- **强制用户改用 `programs.msime` 模块**：模块已经正确接线，但 overlay 直配是公开且有文档的用法，要求迁移会破坏现有配置并扩大本次修复范围，因此不选。

## Consequences

overlay 直配用户重建配置后，两个 socket 会随 `sockets.target` 启动，剪贴板服务会随图形会话目标启动，语音浮窗可以连接 `msime-linux-voice.socket`。模块配置保持不变，README 的两种输入法框架示例现在具有相同的 systemd 接线。

代价是直配示例多出三项 systemd 设置；用户必须在修改配置后重建并重载用户会话，已有按路径手动启用的旧 store unit 仍应按 README 的迁移说明清理。

## Verification

对照 `platforms/linux/nix/module.nix` 核对 README 两个直配示例中的三个 `wantedBy` 名称和目标值完全一致；`git diff --check` 通过。笔记树、格式和归档校验在提交前运行。
