# Agent Note: Windows 的 engine-capi 发布构建使用短 Cargo 缓存路径

Status: implemented

## Problem

`Release msime-engine-capi` 的 Windows 作业在解析 `llama-cpp-2` 时，Cargo/libgit2 检出其 `llama.cpp` 子模块失败。失败路径超过 Windows 路径长度限制；同一 Pull Request 连续两次遇到相同错误。仓库已经有 `platforms/windows/Use-ShortCargoHome.ps1`，但发布 workflow 没有调用它。

## Decision

发布 workflow 的 Windows 构建在第一次运行 Cargo 之前调用 `Use-ShortCargoHome.ps1`，把 `CARGO_HOME` 设为短目录联接。Linux 和 macOS 保持原有路径；Windows 的 `ci-platforms.yml` 和 ONNX workflow 已采用同一入口，本修复让 engine-capi 发布流程与它们一致。

## Alternatives considered

- **只重新运行失败的 job** — 最强的理由是无需改代码，可能避开偶发的 runner 路径差异。没有采用：同一长路径在两个独立 Windows runner 上重复失败，重跑不会改变 Cargo 的检出位置。
- **开启 Git `core.longpaths`** — 最强的理由是 Windows Git 的常见长路径开关。没有采用：失败发生在 Cargo/libgit2 检出 Git 子模块，仓库已有说明确认该开关不能解决这条路径；短 `CARGO_HOME` 才能缩短 libgit2 的工作目录。

## Consequences

Windows 发布构建复用现有 Cargo 缓存，只增加一个短路径联接，不改变依赖版本、构建目标或产物。该 workflow 的 Windows job 会在后续 PR 运行中覆盖长路径回归；本机 macOS 无法复现 Windows runner 的 Cargo/libgit2 限制。
