# Agent Note: 升级 ZIP 解压库以修复路径安全公告

Status: implemented

## Problem

`zip 8.6.0` 被 RustSec 公告 `RUSTSEC-2026-0367`、`RUSTSEC-2026-0368` 和 `RUSTSEC-2026-0369` 标记为高危或严重：解压时可能通过符号链接写出目标目录，或错误保留特权位。CI 的 `cargo audit` 要求升级到 `9.0.0` 或更高版本。

## Decision

两个使用 ZIP 的 crate 固定到 `zip 9.0`，锁文件解析到 `9.0.4`。`zip 9` 将成员名读取改为可失败的 `Result<Cow<str>>`；导入路径先读取并传播名称错误，再继续现有的普通相对路径、封闭路径、符号链接和目录检查。没有放宽归档边界或把不合法名称当作可安装文件。

## Alternatives considered

- 忽略 RustSec 公告：可以保持最小 diff，但会让 CI 继续失败并保留已知解压风险。
- 只在 `cargo audit` 配置中豁免：不修复实际依赖，且会掩盖未来同类风险。
- 改用另一套归档库：需要重写现有路径、预算和平台行为；当前升级已提供兼容的安全修复。

## Verification

`cargo check -p msime-client-services -p msime-client-providers --locked --quiet`、`cargo test -p msime-client-services --locked --quiet`（1043 通过、1 忽略）、`cargo fmt --all`、差异检查和 `cargo audit` 通过；审计结果不再报告 ZIP 漏洞。

## Consequences

归档成员名称的读取错误现在在导入阶段显式拒绝，已有路径和符号链接策略保持。锁文件增加 `nix` 间接依赖；所有使用 ZIP 的路径都得到上游安全修复。该 PR 只为解除新安全公告造成的合并阻塞，不改变输入法运行时模型路径。
