# Agent Note: Windows 轻量包不再清空 server 目录

Status: implemented

## Problem

在装过完整包的机器上运行 `installer/test-light.ps1`，装完之后能打出组字下划线，但没有候选、也上不了屏。`C:\Program Files\metasequoiaime-full\server` 里只剩可执行文件和 DLL，`server\resources` 整个没了（数据目录的 `verified-resources.json` 还指着那里的 `msime-pinyin.db` 等文件）。看门狗在跑，Server 拉不起来，管道列表里没有 `.full` 后缀的管道，TIP 能开组字，却没有 Server 应答。

`msime_setup.iss` 的 `PrepareToInstall` 不分包型，一律 `TryDeleteTree('{autopf}\<install_dir>\server')`；而 `Prepare-PackageFiles.ps1 -Light` 不暂存 server 目录里的任何数据：`resources` 词库、`zh-<lang>.db` 候选释义、词书、粤拼注音笔画词库、重排模型。轻量包于是每装一次就删掉词库，与文件头注释「安装时也不会删本机已有词库」相反。[自定义辅助码表移到状态根](2026-11-03-custom-helpcode-tables-in-state-root.md)记下过「轻量包也一样」删整个 server 目录，当时只处理了 `helpcodes\custom` 的搬迁。

## Decision

清空 server 目录挪进 `PrepareToInstall` 已有的 `#ifndef LightPackage` 分支，和 `CleanAppDataExceptUserFiles` 放在一起：完整包照旧先清空再整体写入；轻量包原地覆盖可执行文件和 DLL（`[Files]` 都带 `ignoreversion`），server 目录里的数据保持不动。`installer/tests/lifecycle.ps1` 断言这条删除只出现在完整包分支里。

## Alternatives considered

- **轻量包删 server 目录时跳过 `resources`。** 除了 `resources`，候选释义、词书、语言词库和重排模型也装在 server 目录、轻量包同样不带，要维护一张会过时的保留名单。
- **轻量包也暂存词库。** 那就不再是轻量包，打包和安装都慢回完整包的量级。

## Consequences

轻量包不清旧版本留在 server 目录里、新版本已不再安装的文件；轻量包只用于本机在已装完整包上迭代 TSF / Server / 设置，下一次完整安装会清掉它们。已经被删掉词库的机器需要重装一次完整包（`test.ps1`）。
