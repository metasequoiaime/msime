# Agent Note: Windows 发布构建使用短 Cargo 缓存路径

Status: implemented

## Problem

`Release msime-engine-capi` 和 nightly 的 Windows 发布作业在解析 `llama-cpp-2` 时，Cargo/libgit2 检出其 `llama.cpp` 子模块失败。失败路径超过 Windows 路径长度限制；仓库已经有 `platforms/windows/Use-ShortCargoHome.ps1`，但部分发布 workflow 没有调用它。

## Decision

所有会解析该 Git 依赖的 Windows 发布构建在第一次运行 Cargo 之前调用 `Use-ShortCargoHome.ps1`，把 `CARGO_HOME` 设为短目录联接。Linux 和 macOS 保持原有路径；Windows 的 `ci-platforms.yml`、ONNX、engine-capi、现代 Windows 发布矩阵、Windows 7 发布 workflow 和 nightly 发布入口统一采用同一入口。

## Alternatives considered

- **只重新运行失败的 job** — 最强的理由是无需改代码，可能避开偶发的 runner 路径差异。没有采用：同一长路径在两个独立 Windows runner 上重复失败，重跑不会改变 Cargo 的检出位置。
- **开启 Git `core.longpaths`** — 最强的理由是 Windows Git 的常见长路径开关。没有采用：失败发生在 Cargo/libgit2 检出 Git 子模块，仓库已有说明确认该开关不能解决这条路径；短 `CARGO_HOME` 才能缩短 libgit2 的工作目录。

## Consequences

Windows 发布构建复用现有 Cargo 缓存，只增加一个短路径联接，不改变依赖版本、构建目标或产物。现代 Windows 的六个 edition 和 Windows 7 的 runtime/package job 都在首个 Cargo 调用前建立短路径，因此资源安装和声明收集不会再回到默认长路径。该 workflow 的 Windows job 会在后续 PR 运行中覆盖长路径回归；本机 macOS 无法复现 Windows runner 的 Cargo/libgit2 限制。资源安装工具的跨 edition 复用、跨 edition 原生代码复用仍未实现，作为 #11080 的后续工作。

## Verification

- `platforms/windows/tests/tools/short_cargo_home.ps1` 已有短路径联接、缓存复用、幂等和冲突保护的行为测试，仍由 Windows release script tests 执行。
- 通过 workflow 静态校验确认现代 Windows 的 `release` job 和 Windows 7 的 `runtime`、`package` job 均在首个 Cargo 调用前运行 `Use-ShortCargoHome.ps1`；`ci-platforms` 也会把这两个发布 workflow 的改动路由到 Windows 脚本测试，使 `short_cargo_home.ps1` 真正执行而不是按 scope 跳过。
