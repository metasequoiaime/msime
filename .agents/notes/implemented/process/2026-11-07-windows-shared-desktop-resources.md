# Agent Note: Windows 发布复用一次已校验桌面资源

Status: implemented

## Problem

现代 Windows 发布的六个 edition 各自在自己的 job 中运行 `msime-client-core` 的 `install_resources`，重复解析 Cargo 依赖、下载并校验同一份 `resources/desktop-dictionary.lock.json`。资源准备在主要编译之后发生，资源或 Cargo 失败会浪费已经完成的 edition 构建。

## Decision

在 `release-windows.yml` 增加独立的 `desktop-resources` job。它固定使用 `source` job 解析出的提交，在短 `CARGO_HOME` 下运行一次 `install_resources`，确认输出目录非空后上传带版本和提交摘要的 artifact。六个 edition 的 `release` job 等待该 job，下载同一个 artifact，并把下载目录写入 `MSIME_DESKTOP_RESOURCES`；edition job 不再运行 `install_resources`。

资源仍由现有锁文件和 `install_resources` 做摘要校验，artifact 名称绑定版本和源提交，避免不同源码来源之间误复用。edition 间原生代码复用、sccache 策略调整和 Windows 7 资源链路不在本次范围内。

## Alternatives considered

- **每个 edition 继续独立准备资源** — 最强的理由是失败隔离且不需要新增 job。没有采用：这正是重复 Cargo 解析和下载的来源，资源失败也会在主要编译完成后才暴露。
- **让六个 edition 共享 runner 文件系统目录** — 最强的理由是可以避免 artifact 传输。没有采用：矩阵 job 不保证共用 runner，跨 job 写入 runner 临时目录也不能提供可追溯的来源绑定。
- **把资源目录直接写进 Rust artifact** — 最强的理由是减少 artifact 数量。没有采用：Rust 输出和桌面资源的生命周期、大小和失败重试边界不同，混在一起会让资源失败重新触发三个架构的 Rust 构建。

## Consequences

资源下载、校验和 `install_resources` 的 Cargo 编译从六次降为一次；六个 edition 增加一次 artifact 下载和本地目录检查。资源准备 job 通过后才会启动 edition 矩阵，资源准备失败会更早阻止后续打包。artifact 存在于当前 workflow run，仍可由版本和源提交追溯。

## Verification

- `actionlint .github/workflows/release-windows.yml .github/workflows/ci-platforms.yml`
- `python3 scripts/test-windows-desktop-resources.py`
- Windows release script tests 会执行同一份 workflow contract；真实资源下载和六 edition 打包需在手动 Windows release/nightly run 中验收。
