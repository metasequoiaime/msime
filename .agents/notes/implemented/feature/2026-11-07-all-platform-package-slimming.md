# Agent Note: 全平台安装包按需资源瘦身

状态：已实现

## Problem

日文词典组（`msime-japanese.dat` 及两份 Mozc 许可文本）约 63.5 MiB，五笔码表 `msime-wubi.db` 约 135.9 MiB。macOS、Android 和 iOS 已经有按需资源包；鸿蒙的实现位于同一切片中。Windows 现代安装包仍把两组资源写进 `server_exe/resources`，而共享校验规则也只允许移动端和 macOS 缺席。

## Decision

把日文词典组和五笔码表纳入桌面三平台、Android 和鸿蒙的按需资源组；iOS 继续只省略日文词典组。共享资源校验按 `target_os` 放行整组缺席，资源包目录仍由锁文件校验和原子发布。

Windows 现代 full 与 pinyin 安装包省略两组资源，设置应用在用户选用日文或五笔时下载到 `resource-packs`。wubi 版本保留五笔码表，japanese 版本保留日文词典，因为它们分别是默认方案；Windows 7 没有这套资源包入口，继续带完整词库。桌面资源包入口因此在 macOS、Windows、Linux 三个平台都提供日文和五笔资源包。Linux 发布页的 deb、rpm 和 tar.gz 本来就不把词库打进包，本次只让其共享运行时规则与桌面资源包语义一致；发行版仍可选择打包完整资源。

本次不改变任何偏好默认值，也不改变资源包的下载地址、锁文件或用户词库更新代次。保留已有的资源包安装、收编、校验和会话刷新流程。

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
- Windows 安装器的合成包测试已更新为检查现代 full 包不含四个可选文件、wubi 包仍含 `msime-wubi.db`，以及预检失败不污染旧暂存；尚未在本机运行 PowerShell/Windows 构建。
- 尚未在本机重打六个平台安装包或跑真机设备套件。当前工作机可用空间约 19 GB，无法安全启动跨平台完整构建；CI/Windows、Android、iOS、HarmonyOS 的原生安装包和设备门禁仍需在 PR 上执行。鸿蒙按需资源的既有真机签名失败限制见同目录的 `2026-11-06-harmony-on-demand-resources.md`。

## Consequences

full 与 pinyin 的 Windows 现代安装包以及鸿蒙、Android 的对应瘦包可少带约 199.4 MiB；wubi 版本少带约 63.5 MiB；日文版保留日文主资源。用户第一次选用被省略的方案时会看到已有资源包下载入口，下载完成后下一次取得焦点即可使用。
