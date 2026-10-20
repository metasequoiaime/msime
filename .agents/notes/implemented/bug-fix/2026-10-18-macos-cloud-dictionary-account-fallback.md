# Agent Note: macOS 云词库使用共享账号回退

Status: implemented

## Problem

#8580 中设置进程没有 IME 注入的原生会话，也没有 Linux 的运行时 socket。账号已经登录，云词库却返回 unavailable；输入法菜单还漏掉已有的云词库动作。

## Decision

macOS 保留显式 provider、原生会话和发现 socket 的优先级；没有服务入口时，调用与 Windows 相同的 desktop_cloud_dictionary，使用既有 AccountState 及同一份会话文件与锁。已有入口的服务错误直接返回，避免失败后重复发送写操作。快照预览状态注册在设置应用自己的缓存目录。菜单增加云词库，设置页与账号页按 macOS 能力隐藏重复入口。面板按机器错误码显示登录、网络、冲突等原因，不展示原始错误负载。

## Alternatives considered

只补菜单能用上原生会话且改动小，但已打开的设置进程面板仍失败；只给设置进程注入令牌能复用原生通道，却复制刷新与凭据交接协议。复用共享账号会话不增加凭据来源。

## Verification

原生输入法 `MSIMEClientInputMethod` 与 `shortcut-test` 编译通过，`ctest -R ^shortcut$` 通过。云词库面板完整 UI 文件 17 项通过，入口与错误提示专项共 21 项通过；前端类型检查和构建通过。未使用真实账号做联网验收。

## Consequences

不把已有 provider 的业务失败当作缺少入口；当时保持 Linux 的 provider-only 行为，后来由 [Linux 账号云功能对齐](../feature/2026-10-20-linux-account-features-and-input-parity.md) 翻转：Linux 没有 provider 时也走账号会话，provider 优先、失败不重发照旧。真实账号联网验收不使用用户凭据。

设置页云词库入口由[词库顶部标签](../feature/2026-10-17-dictionary-top-tabs-and-lookup.md)接管：提供页内客户端时显示标签，原生菜单与账号回退协议继续沿用本篇。

## 历史笔记检索

检索 desktop_cloud_dictionary、云词库和 macOS。[Windows 桌面云账号笔记](../feature/2026-10-10-windows-desktop-cloud-account.md)部分重叠，保留账号共享锁及快照规则；[macOS 工具设置裁剪](../simplification/2026-10-16-macos-tools-support-settings-prune.md)部分重叠，本篇收窄云词库入口；会话存储注入提案无关，不推进其架构迁移。
