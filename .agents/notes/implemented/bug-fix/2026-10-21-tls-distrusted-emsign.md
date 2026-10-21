# Agent Note: TLS 拒绝已撤销信任的 emSign 证书链

Status: implemented

## Problem

RustSec 于 2026-10-20 公布同一 CVE 的两条公告：[RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) 要求 `rustls-webpki` 0.103.16 拒绝失去信任的中间证书公钥；[RUSTSEC-2026-0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html) 要求 `webpki-roots` 1.0.10 移除 emSign ECC Root CA - G3。当前锁文件的 0.103.15、1.0.9 和转发包 0.26.11 均命中审计。

`reqwest` 将这套校验与根证书用于 client-services 和 client-providers；豆包凭据探测的 `tokio-tungstenite` 0.30 另通过 `rustls-tls-webpki-roots` 拉入 0.26 转发包。它只是重导出 1.x 的根证书，但公告的修复范围不包含 0.26，所以单更新锁文件仍留下审计失败。

## Decision

依赖锁定上游修复版 `rustls-webpki` 0.103.16 与 `webpki-roots` 1.0.10。豆包探测通过已有的 `Connector::Rustls` API 明确传入相同 Mozilla 根证书策略的配置，使用 `tokio-tungstenite` 的 `__rustls-tls` 公共 feature 启用 TLS，不启用旧根证书转发包。配置的构建逐项沿用上游默认实现：正常证书校验、无客户端证书，不引入原生根证书或自定义 verifier。

## Existing notes audit

按 `webpki`、`rustls`、`emSign` 与两个公告编号检索活跃笔记，没有同问题记录。[云剪贴板中继](../../implemented/feature/2026-10-12-cloud-clipboard-relay.md)及其 Android 接入明确要求内置公共根证书而非系统信任库，本改动保留这一取舍，其余命中与本决定无关。

## Alternatives considered

- 只更新锁文件：运行时转发包会使用修复后的根证书，改动最小；但 `cargo audit` 仍报告 0.26 转发包，无法得到没有漏洞忽略项的审计结果。
- 忽略根证书公告：能绕过转发包误报，但同一个公告也覆盖真正受影响的 1.x 根证书；永久忽略会削弱未来回退时的检测。
- 改用系统根证书：有公开 feature、无需自组配置，但会改变现有跨平台信任来源，也会让用户安装的证书进入探测连接，与既有根证书策略不符。
- 直接传入 Rustls 配置：使用上游已支持的连接器，根证书内容来自当前 1.x 包；代价是显式依赖 TLS 库，并使用名称带内部前缀的基础 feature。版本仍限制在 0.30 系列，未来能直接使用 1.x 根证书 feature 时可以删掉这层配置。

## Verification

同一锁文件的 `cargo audit --json` 在修复前命中三条漏洞记录，修复后为零；没有增加忽略项，仍单独报告三条现有未维护公告。`cargo test -p msime-client-providers --locked` 的 247 项测试通过、两项既有忽略；`cargo test -p msime-client-services --locked` 的 1,272 项常规测试及隔离测试通过、一项既有忽略。独立审查对照 registry 中上游默认连接器实现，确认标准证书与主机名验证、TLS、根证书来源和截止时间保持一致。

本地 quick 通过，其中 Linux 容器的 102 项原生测试、完整版与五笔版安装布局通过，macOS 原生宿主构建、Android 目标与 JVM 宿主检查、WebAssembly 检查通过。Windows 交叉编译和 HarmonyOS ArkTS 真编译因本机缺少预备环境按脚本跳过；没有设备验收。`cargo fmt --all -- --check`、相关两 crate 的 `cargo clippy --all-targets --locked`、笔记校验与 `git diff --check` 通过。

## Consequences

显式配置必须与上游默认校验一致，不能绕过主机名或证书链验证；不更改握手超时、重定向、响应上限或合成 PCM 探测行为。真实 TLS 服务和各平台设备验收需在报告中区分。
