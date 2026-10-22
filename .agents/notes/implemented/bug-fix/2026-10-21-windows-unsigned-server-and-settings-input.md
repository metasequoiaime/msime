# Agent Note: Windows 未签名包保留普通输入能力并校正设置窗口缩放

Status: implemented

## Problem

#9074 的发布包和 nightly 在 Windows 11 只有行内编码，没有候选窗和工具栏。报告人的后续检查发现 Server 没有签名；`Build-Client.ps1` 却无条件给 x64 Server 开启 `MSIME_SERVER_UIACCESS`。Windows 要求 `uiAccess=true` 的程序带受信任签名并安装在安全目录，否则不能启动。发布工作流默认不签名，nightly 也不走发布签名，安装冒烟只核对文件和 TIP 注册，漏掉了 Server 的可启动性。

#9201 报告的 Windows 0.3.0 同样只有行内拼音、没有渲染窗口，按数字后无法上屏。公开版本 `windows-v0.3.0-beta` 早于本修复，不能用当前源码已有开关推断旧包已经可用。用户具体下载来源尚未核实，故该报告只关联本修复，不据此宣称真实编辑器验收完成。

同一报告还指出原生 WinUI 设置文字模糊、鼠标滚轮无效。设置工程未显式提供 DPI 清单，最终生成的清单与真实窗口的输入行为需要在 Windows 上核对，不能从编译成功推断修复完成。

## Decision

让构建入口默认生成普通权限 Server，显式传入 OFF 以覆盖旧 CMake 缓存。仅可信生产签名流程显式启用 UIAccess；SimplySign 和本地受信任测试签名保留原能力，SignPath 不受信任测试证书使用普通权限。签名开关和服务账户配置不变。

Windows 的基线链接产物经 `mt.exe` 提取后没有 `dpiAware` 或 `dpiAwareness` 节点。设置工程以 `Manifest` 项目项合并 `app.manifest`，声明 PerMonitorV2 与 PerMonitor 回退，保留 SDK 注入的 WinRT 激活清单。

设置工程从 Windows App SDK 1.7.3（`1.7.250606001`）更新到同一系列维护版 1.7.9（`1.7.260224002`）。微软 [1.7.5 发行说明](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-notes/windows-app-sdk-1-7#version-175-17251014001) 确认修复关闭「悬停时滚动非活动窗口」后滚轮无效的问题。本仓没有另写滚轮转发或改变用户系统设置；原生 UI 验收测试先把窗口固定为 800×560 并打开内容较长的工具栏页，等待实际布局提供滚动范围，再临时切两种路由、发送鼠标滚轮并确认正文位置改变，最后恢复设置与指针位置。输入页默认标签页可能没有溢出，不能用它作为必然可滚动的夹具。

## 历史决策审计

- [SignPath 签名](../../implemented/process/2026-10-08-windows-signpath-ci-signing.md) 部分重叠：保留签名来源、审批和本项目二进制的边界，本篇取代其中未签名包仍无条件带 UIAccess 的行为，两篇互链。
- [CS2 系统目录部署](../../implemented/feature/2026-10-08-windows-cs2-trusted-mode-system32-deployment.md) 不变：本篇不更改 TIP 安装位置和可信游戏的签名要求。
- [WinUI 模板陷阱](../../implemented/bug-fix/2026-10-17-windows-settings-winui-template-traps.md) 不变：已有模板和多行文本修复保持，不以它们解释尚未重现的滚轮现象。

## Alternatives considered

- 所有包永久关闭 UIAccess。路径简单、未签名 Server 可启动；但会使可信签名安装包失去跨完整性级别的界面能力，因此不采用。
- 所有发布必须先签名。能统一 Windows 信任行为；但签名服务登记与审批独立于程序修复，nightly 和普通开发包仍需要可用，因此不以它阻塞普通权限构建。

## Verification

- `bash scripts/verify-local.sh --quick` 在远端 macOS 复用本任务构建缓存通过：Windows 本机可运行测试 171 项、仓库契约、Rust workspace、Android target、Linux 设置外壳和 Apple bridge 编译通过；没有将 MinGW 缺失时跳过的完整 Windows 产品编译算作通过。

- Windows 上 `build_client.ps1` 通过，覆盖默认 OFF、显式 ON、再切回 OFF，以及预构建 Rust 输入。`installer_entry.ps1` 通过，确认本地可信签名入口保留 UIAccess。
- Windows 上 `tests/server-manifest/verify.ps1` 通过：实际 MSVC 链接同一 probe 的 OFF → ON → OFF，读取每次生成 PE 的清单，两个普通权限产物实际启动成功。它复用产品的 `ServerManifest.cmake`，不是完整 Server；安装冒烟新增完整 Server 的 `--help` 启动检查。
- Windows 上 MSVC 编译和链接设置宿主后，`mt.exe` 提取实际 PE：`dpiAware=true/pm`、`dpiAwareness=PerMonitorV2, PerMonitor`，928 项 WinRT 激活声明保留。完整 Build 在后续 PRI 处理阶段遇到本机缺少 `Microsoft.Build.Packaging.Pri.Tasks.dll`，同样发生于未改动的基线；不能当作完整设置构建通过。
- 鼠标滚轮测试仅以 ScrollPattern 重置起点和读取位置，滚动动作来自鼠标输入。完整设置构建与交互式验收由 PR 的 `Windows native settings` 作业继续验证。
- #9201 的公开产物对照（2026-10-22，Windows 原生加载器）：`windows-v0.3.0-beta` 的 full 安装包 SHA-256 为 `9aba661a7171d9d61e5145d9dd20c0a91c8f8817ceb4b6c4b4f945c82a76b55f`，与同页摘要一致；对应发布任务 `39414032228` 的暂存产物 `12261757883` 内 Server SHA-256 为 `534d9c0e30224d49502537185079bef9b72a5997d1992fb6cb66bb0a07755b21`，PE 证书目录为 `(0, 0)`，内嵌清单为 `asInvoker` / `uiAccess=true`。签名任务在该次发布中均被跳过，`Get-AuthenticodeSignature` 确认 `NotSigned`。在同一隔离目录中配齐同一暂存包安装时会复制的运行时 DLL 后，原始 Server 以 `UseShellExecute=false` 启动返回 740（需要提升），以 `true` 启动返回 1459（需要交互窗口站）；仅用 Windows SDK 的 `mt.exe` 将副本清单改为 `uiAccess=false`，普通权限副本在这两条启动路径实际执行 `--help`、退出码均为 0。这个差异证明该构建的启动权限缺口，不证明报告人的安装来源或 TSF 编辑器链路。
- 对 `d88229b20e` 源码重新执行 `build_client.ps1`、`installer_entry.ps1` 与 `tests/server-manifest/verify.ps1`，全部通过。后者由 MSVC 在同一规定目录 `target/windows-server-manifest` 构建 OFF → ON → OFF，两个 OFF 产物实际运行成功。本轮没有重新构建完整 Server，也没有修改、签名或替换公开发布文件。仅补笔记的远端 `bash scripts/verify-local.sh --quick`（任务 `20261022-112107`）退出 0，Rust workspace、仓库契约、笔记和共享 Apple bridge 通过；各平台构建依改动范围跳过。
- 验证缺口：SSH 所在的非交互窗口站无法设置鼠标路由，系统返回 `ERROR_REQUIRES_INTERACTIVE_WINDOWSTATION`（1459）；不能在此宣称滚轮已验收。真实签名安装和报告人的 Windows Insider 26340 不在本机验证范围。

## Consequences

普通权限构建让未签名 nightly 和开发包恢复可启动性，代价是必须在构建时区分签名策略。升级 SDK 引入同一维护系列的修复，增加两个系统路由下的界面验收。

未签名普通权限包仍不能获得 UIAccess 的能力；不能宣称它在提权应用中与可信签名包等价。真实发布签名和用户报告的 Windows Insider 版本需要后续验收。
