# Agent Note: Android 皮肤 Tab 同步本地库状态

Status: implemented

## Problem

Android 皮肤页的「我的」和「社区」分类分别持有本地皮肤模型与社区卡片状态。获取或删除皮肤后切换分类时，前者可能继续显示旧的本地库，后者也会保留已删除皮肤的「已获取」状态。

## Decision

切换到「我的」分类时重新读取本地皮肤库，让异步结果更新卡片模型。社区列表收到本机已获取结果时按完整结果对账：当前结果中没有的旧状态被移除，结果中存在的状态被更新；正在进行的 `BUSY` 操作由自己的异步完成回调收尾，不能被较早的磁盘读取覆盖。缺少状态的卡片继续由适配器的默认分支显示为可获取。

旧的社区列表复用、皮肤库文件格式、跨进程锁和社区目录缓存契约保持不变。相关历史决定见[Android 社区皮肤 Tab 直接承载完整列表](../feature/2026-10-13-android-skin-community-tab-list.md)、[本地皮肤独立为主题页顶部标签](../feature/2026-10-18-local-skin-top-tab.md)和[社区皮肤库代次](../simplification/2026-10-12-community-skin-library-generation.md)。

## Alternatives considered

- **只在切换「社区」时重新请求社区目录**：可以刷新卡片，但不会重读本地皮肤库，也会增加不必要的网络请求；本 issue 的状态来源是本地库，故不采用。
- **仅给 `setActions()` 补写当前条目**：改动最小，但删除后的旧 `DONE` 仍会留在内存中，切回社区仍显示错误状态；改为完整对账并保留 `BUSY`。

## Consequences

切回「我的」会发起一次本地读取，读取完成后显示刚获取或导入的皮肤；切回「社区」会立即以最新本地库状态重绘，已删除条目恢复为可获取，正在获取的条目不会被异步读取抢回。社区目录、搜索、分页和下载流程不变。

## Verification

`CommunityActionPolicySmoke` 覆盖保留已获取、清除已删除和保留进行中三种状态；提交前运行 Android JVM 冒烟、`platforms/android/check-host.sh`、本地 quick 门禁、`pnpm run verify-notes` 与 `git diff --check`。设备验收按 Android 宿主流程执行；若受本机磁盘空间限制未能启动模拟器，将在 PR 验证段明确记录。
