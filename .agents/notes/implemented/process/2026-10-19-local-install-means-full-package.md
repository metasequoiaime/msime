# Agent Note: 本机「安装」指整套安装

Status: implemented

## Problem

维护者在开发机上说「安装」「更新安装」「切到 develop 安装」时，代理多次只跑了 `platforms/macos/scripts/install.sh`。它只替换 `~/Library/Input Methods` 下的输入法本体，`/Applications/MSIME.app` 里的设置应用和 `msime-mcp` 都不动。2026-10-19 输入法已经是 0.54.0，用户打开的设置应用仍是 0.52.0，也带着 develop 上早由 #8837 修好的弹窗透底问题，用户以为修复没有生效。另外 `apps/desktop/src-tauri/tauri.conf.json` 的版本号写死为 0.52.0，只有 `package-release.sh` 用 `TAURI_CONFIG` 覆盖，任何不经打包构建出的设置应用都报这个旧值。

## Decision

在 `AGENTS.md` 新增「本机安装」一节：开发机上的「安装」指用 `platforms/macos/package-release.sh` 从当前分支打出完整的包（设置应用连同内嵌的输入法、核心词库和 `msime-mcp`），备份后替换 `/Applications/MSIME.app`，打开一次让它装好内嵌输入法，再核对设置应用和输入法的版本都等于 `platforms/macos/version.txt`。只换输入法的 `install.sh` 留给明确只改输入法、需要快速迭代的场合。

## Existing decisions

已检索 `install.sh`、`package-release.sh`、本机安装相关笔记；`launch-platform-host` skill 记录了构建与 `install.sh` 的用法，没有约定「安装」一词的范围，本篇不改它的步骤。

## Alternatives considered

- 继续只用 `install.sh`：最快，几十秒就能换上输入法，单改输入法时确实够用；但维护者要验收的是整套产品，设置应用、`msime-mcp` 和版本号都不会跟着更新，正是这次误会的来源。
- 等正式发布包再装：版本号和签名都与用户拿到的一致；但只能装 `main` 上已发布的内容，验收 develop 或 PR 分支时用不上。

## Consequences

一次整套安装要按两个架构编 release 版，耗时二三十分钟、占十几 GB，开编前必须先查磁盘。`tauri.conf.json` 版本号写死的问题仍在，打包路径之外构建的设置应用还会报旧版本，需要单独修复。
