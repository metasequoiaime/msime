# Agent Note: CI 的 sccache 起不来时照常编译

Status: implemented

## Problem

`ci-ios.yml`、`ci-macos.yml`、`ci-platforms.yml` 与 `release-windows.yml` 的 Rust 任务把 `RUSTC_WRAPPER: sccache` 直接写在 job 级 env 里，后端是 Depot runner 自带的 Depot Cache（WebDAV）。缓存服务一时连不上时 sccache 服务启动超时，cargo 连 `rustc -vV` 都跑不过，整个任务失败：#9658 的 `Web engine (wasm)`（run 40160742127）就在「Build the web engine」第一步报 `sccache: error: Timed out waiting for server startup. Maybe the remote service is unreachable?` 退出，一次编译都没开始。缓存只是用来加速的，它的故障不该让 CI 变红。

## Decision

这十个任务改用 [macOS 发版提速](2026-10-20-macos-release-faster-packaging.md)里已经验证过的接法：

- job 级 env 不再设 `RUSTC_WRAPPER`，改设 `SCCACHE_IGNORE_SERVER_IO_ERROR=1`，构建途中服务出错时 rustc 退回直接编译。
- 「Install sccache」带 `id: sccache` 和 `continue-on-error: true`，装不上也继续。
- 紧跟一步「Enable sccache」（条件与安装步骤相同，再加 `steps.sccache.outcome == 'success'`，`shell: bash` 以便 Windows runner 也能跑）：`sccache --start-server` 成功才把 `RUSTC_WRAPPER=sccache` 写进 `GITHUB_ENV`，失败就出一条 warning，照常不带缓存编译。
- `ci-macos.yml` 构建矩阵原来用表达式在 x86_64 腿上把 `RUSTC_WRAPPER` 置空，现在那条腿不安装、不启用 sccache，效果相同。

## Alternatives considered

- 只重跑失败的任务：这次确实是偶发（之前 25 次失败的 Native Platform CI 里都没有这条错误），重跑就能过。但每次遇到都要人工判断、重跑，而且同一个缺陷在十个任务里都有，任何一个碰上都会把 PR 卡住。
- 把 sccache 的启动超时调长（`SCCACHE_STARTUP_TIMEOUT`）：缓存服务短暂抖动时能撑过去。但服务真的不可用时只是让任务晚一点失败，没有解决「缓存故障让构建失败」这件事。

## Consequences

Depot Cache 不可用时这些任务退回不带缓存的完整编译，耗时会变长（Rust 部分从热缓存的几分钟变回冷编译），但不再失败；runner 上会留一条 warning 说明缓存没起来。缓存可用时行为与原来相同。

## Verification

- `actionlint` 1.7.12（与 `quality.yml` 同版本，`SHELLCHECK_OPTS=--severity=warning`）检查全部工作流通过。
- 实际的回退路径（服务起不来时照常编译）要等下一次 Depot Cache 抖动才能在 CI 上看到；`release-macos.yml` 在托管 runner 上用的是同一套步骤。
