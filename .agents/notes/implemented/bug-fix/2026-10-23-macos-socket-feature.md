# Agent Note: macOS 宿主显式启用 Unix socket 转换

Status: implemented

## Problem

`msime-host-macos` 的面板与云剪贴板连接使用 `socket2::Socket` 的限时连接，再将套接字转换为 `UnixStream`。`socket2` 0.6.5 将这个 `From<Socket>` 实现放在 `all` feature 下，宿主清单却只启用默认 feature。单独执行 `cargo test -p msime-host-macos` 在两处 `socket.into()` 报 `E0277`，无法运行现有测试。

工作区里的 `hyper-util` 和 `tokio` 同时启用 `socket2/all`，Cargo 合并 feature 后掩盖缺项；整个工作区的编译通过不能证明宿主的独立依赖声明完整。Cargo 的 [feature 合并规则](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification)解释了这两个结果的差异。

## Decision

宿主的 `socket2` 依赖显式启用 `all`，为已使用的 Unix socket 转换声明所需 feature。现有连接超时、对端身份校验、帧限制和错误语义沿用原实现。继续用单独的包测试验证，命令不额外传 feature。

## Existing notes

按 `socket2`、`connect_timeout`、`PanelSession` 与独立构建检索活跃笔记，没有持有该依赖 feature 的既有决定。[面板客户端契约](../../implemented/architecture/2026-10-15-panel-client-contracts.md)属于共享 TypeScript 类型归属，与本次 Rust 依赖声明无关，保持原位；本篇记录独立编译缺口。

## Alternatives considered

- 经 `OwnedFd` 中转可使用默认 feature，转换也保持安全；但要改两条现有连接路径，而 `socket2` 已提供被 feature 控制的直接转换。显式声明所用 API 的 feature 改动更小，不额外维护转换代码。
- 只把测试与联网 crate 一起编译可以复用它们开启的 `all`，也能使现有命令变绿；但宿主仍依赖无关 crate 的清单，单独测试和消费继续失败。

## Verification

在 `origin/develop` 的独立 worktree 上，不传额外 feature 的单包测试先因两处 `E0277` 失败。清单显式开启后，`cargo test -p msime-host-macos --locked -- --test-threads=1` 的 21 项单元测试全部通过；单包 `cargo clippy --locked --all-targets -- -D warnings` 也通过。`cargo tree -p msime-host-macos -e features -i socket2` 用于核对 feature 来源，工作区 feature 合并不作为独立编译的替代验证。

## Consequences

宿主可独立编译并执行既有面板和云剪贴板 RPC 测试，不再依赖其他工作区成员替它开启 feature。`all` 额外公开 `socket2` 的平台 API，现有代码只使用原来的转换，不新增运行时调用。该修复验证依赖声明与现有 RPC 单测，不代表在真实输入法中完成面板装机验收。
