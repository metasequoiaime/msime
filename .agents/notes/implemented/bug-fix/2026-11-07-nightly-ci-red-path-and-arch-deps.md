# Agent Note: nightly Windows 长路径与 Arch 二进制依赖

Status: implemented

## Problem

2026-11-05 的 nightly 跟踪单 #10980 同时暴露了两个独立的发布链缺口：Windows Rust job 在检出 `llama.cpp` 时因 Cargo home 路径过长失败，Arch 二进制包在 `msime-linux-local-translation` 上缺少 `gtk4` 运行时依赖。两项失败都发生在打包链，现有 PR 平台检查无法覆盖 nightly 的完整路径。

## Decision

Windows 发布工作流的 Rust job 在安装目标和构建之前调用现有的 `Use-ShortCargoHome.ps1`，把 Cargo home 通过短路径 junction 暴露为 `C:\mc`，并写入后续步骤使用的 `CARGO_HOME`。这样 Cargo 与 libgit2 检出深层 `llama.cpp` 路径时不再触发 Windows 路径上限，同时继续复用 runner 的 Cargo 缓存。

Arch 的 `msime-bin` 包把 `gtk4` 同时加入 `PKGBUILD` 和提交的 `.SRCINFO`。该包内的本地翻译程序实际链接 GTK 4；依赖声明必须与源码包和打包产物的动态链接检查一致。

## Alternatives considered

- **缩短或重命名 `llama.cpp` 子模块路径**：最强理由是只改依赖检出位置；没有采用，因为 Cargo 的 git checkout 目录由 Cargo 管理，局部改名会破坏缓存复用且不能覆盖其他深层依赖。
- **在 Arch 包中把 `gtk4` 做成可选依赖**：最强理由是减少默认安装体积；没有采用，因为缺少它时包内程序无法启动，属于硬运行时依赖。

## Consequences

Rust job 多一个幂等的路径准备步骤，首次运行会创建 junction，但不会复制 Cargo 缓存。Arch 用户安装 `msime-bin` 时会得到 GTK 4 运行库，包内本地翻译程序不再因动态库缺失而被 `ldd` 检查拒绝。Windows nightly 和下一次 Arch nightly 仍需在真实发布链上重跑确认；本地没有 Windows runner 或 Arch 打包环境，不能用 macOS 构建替代这两项验收。

验证：`git diff --check`、`pnpm run verify-notes`；PR 必须通过工作流校验和相关发布链检查后再合并。
