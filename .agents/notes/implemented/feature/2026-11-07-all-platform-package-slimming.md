# Agent Note: 全平台安装包按需资源瘦身

状态：已实现

## Problem

日文词典组（`msime-japanese.dat` 及两份 Mozc 许可文本）约 63.5 MiB，五笔码表 `msime-wubi.db` 约 135.9 MiB，语言词库约 23.4 MiB。macOS、Android 和 iOS 已经有按需资源包；鸿蒙的实现位于同一切片中。Windows 现代安装包仍把三组可选资源写进安装包，而共享校验规则也只允许移动端和 macOS 缺席。

## Decision

把日文词典组和五笔码表纳入桌面三平台、Android 和鸿蒙的按需资源组；iOS 继续只省略日文词典组。共享资源校验按 `target_os` 放行整组缺席，资源包目录仍由锁文件校验和原子发布。

这篇笔记关于 iOS 五笔码表是否随包的决定已由[iOS 五笔码表改为按需资源](2026-11-07-ios-wubi-on-demand.md)接替；其余平台和共享资源包规则仍以本篇为准。

Windows 现代 full 与 pinyin 安装包省略三组可选资源，原生设置应用在用户选用日文、五笔、粤拼、注音或笔画时，在方案下显示对应词库行，用户点击「下载」或「从文件导入」安装到 `resource-packs`。读取、下载、取消、错误和重试均复用已有资源包安装器；安装后切换一次输入框，Server 与 TIP 各自在聚焦时刷新资源。wubi 版本保留五笔码表，japanese 版本保留日文词典，因为它们分别是默认方案；Windows 7 没有这套资源包入口，继续带完整词库。桌面资源包入口因此在 macOS、Windows、Linux 三个平台都提供日文、五笔和语言词库资源包。Linux 发布页的 deb、rpm 和 tar.gz 本来就不把词库打进包，本次只让其共享运行时规则与桌面资源包语义一致；发行版仍可选择打包完整资源。

本次不改变任何偏好默认值，也不改变资源包的下载地址、锁文件或用户词库更新代次。保留已有的资源包安装、收编、校验和会话刷新流程。

### 原生 Windows 入口补齐（#11436）

0.4.1 nightly 的 Full 安装器已经省略五笔码表，WinUI 的 `select_scheme` 却只有偏好保存；原生资源包目录还只保留神经联想和语法模型，因而没有任何五笔下载按钮。用户看到五笔选中，实际会话因缺少码表而继续使用全拼，重装不会补出码表。共享 React 页的资源下载入口不属于 Windows 原生设置，不能代替它。

`SchemeResourcePackModel.h` 负责方案到资源包的对应关系、完整随包资源识别和词库状态用语，`main.cpp` 只接入现有异步目录与安装控件。完整内置码表显示「随安装包提供」，不提供删除；资源包文件仍由共享层按固定摘要校验，设置页不另造下载器。目录状态只在工作线程读取，不把文件探测放到 UI 线程。五笔版、日文版与已有用户的出厂默认值不变。

本轮检索：本篇完全覆盖安装器与设置入口的既有决定，原地修正；Android、iOS、macOS 与鸿蒙的按需资源笔记分别属于各自宿主，不修改。

## Alternatives considered

- 只改 Windows 安装器：会让各平台对同一份发布资源的校验和运行时规则继续分裂，后续平台再瘦身时还要重复处理。
- 所有版本都删除两组资源：日文版和五笔版的默认方案会在首次启动时失效，违反安装后即可使用默认方案的约定。
- 让输入法进程自己联网补文件：会把网络、权限和失败重试带进按键路径；现有设置应用资源包安装器已经覆盖这些边界，因此不采用。

## Validation

- `cargo fmt --all -- --check`：通过。
- `git diff --check`：通过。
- `pnpm run verify-notes`：通过。
- `bash -n platforms/harmony/stage-resources.sh platforms/macos/stage-resources.sh platforms/android/build-apk.sh`：通过。
- 共享设置页的 Vitest 定向测试未运行：该 worktree 没有 `vitest` 依赖，且当前磁盘余量不足以重新安装完整前端依赖。
- Windows 安装器的合成包测试已更新为检查现代 full 包默认不含三组可选资源、通过 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1` 检查完整语言词库、wubi 包仍含 `msime-wubi.db`，以及预检失败不污染旧暂存；尚未在本机运行 PowerShell/Windows 构建。
- 尚未在本机重打六个平台安装包或跑真机设备套件。当前工作机可用空间约 14 GB，无法安全启动跨平台完整构建。Native Platform CI 的 Windows release script tests、Windows 原生设置、Linux（含 Fedora 和资源验收）、HarmonyOS host、Android host 通过，Android API 28 设备验收通过；API 35 在完整输入 smoke 的随机数字键盘状态断言失败，不能记作 API 35 验收通过。iOS CI 与 macOS package 通过；macOS arm64、x86_64 和 sanitizer 仍失败在既有 `caps-lock` 子进程中止。鸿蒙按需资源的既有真机签名失败限制见同目录的 `2026-11-06-harmony-on-demand-resources.md`。

## Consequences

full 与 pinyin 的 Windows 现代安装包以及鸿蒙、Android 的对应瘦包可少带约 222.8 MiB；wubi 版本少带约 63.5 MiB；日文版保留日文主资源。用户第一次选用被省略的方案时，在原生设置的方案组看到对应词库状态和安装入口；需要点击下载或从文件导入，完成后下一次取得焦点即可使用。
