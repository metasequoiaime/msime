# Agent Note: 更新 quinn-proto 以消除三个拒绝服务漏洞

Status: implemented

## Problem

`Cargo.lock` 固定了 `quinn-proto 0.11.17`。RustSec 在 2026-10-12 发布 [RUSTSEC-2026-0338](https://rustsec.org/advisories/RUSTSEC-2026-0338.html)、[RUSTSEC-2026-0340](https://rustsec.org/advisories/RUSTSEC-2026-0340.html) 和 [RUSTSEC-2026-0343](https://rustsec.org/advisories/RUSTSEC-2026-0343.html)：特定网络帧或数据报缓冲路径可使进程 panic。PR 的 Cargo audit 因这三个通告失败。

## Decision

把锁文件中的 `quinn-proto` 精确更新到三个通告共同列出的首个修复版本 `0.11.18`，保留其他依赖的锁定版本。

## Alternatives considered

- 在 Cargo audit 中忽略通告：仍会分发存在已知缺陷的版本。
- 更新整套依赖：改变范围更大，不利于定位构建或行为回归。

## Consequences

所有使用该锁文件的构建都会解析到修复版本。锁文件只改变 `quinn-proto` 的版本和校验和。

## Verification

`cargo metadata --locked --format-version 1 --no-deps`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过，包含 Rust workspace、Android、wasm、Linux 原生宿主 83 项测试、macOS 和 Apple bridge；PR CI 的 Cargo audit 是最终安全门禁。
