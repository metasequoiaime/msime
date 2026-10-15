# Agent Note: iOS 语言词库按需下载

Status: implemented

## Problem

iOS full 仍把日语词典和粤拼、注音、笔画词库打进键盘扩展，约 86 MB 附加数据；Android 已使用共享资源包的按需安装。

## Decision

主应用复用 host-api 的 resource-packs 接口，装入 App Group 的共享状态目录；只在用户添加语言或点下载时联网，显示大小、进度、取消、重试，对计费或受限网络先提示。共享层使用固定大小与 SHA-256 校验后原子发布，键盘只读取已发布资源，下次取得焦点重读可用方案。full/拼音的发布构建省略日语与附加语言词库，开发构建仍可完整暂存。日文独立版本继续带主词库。运行时据以放行瘦包的规则是 client-core 的 `on_demand_artifacts`：它的目标列表必须包含 `ios`，否则 host-api 按完整清单校验键盘扩展的 `EngineResources`，发布包一启动就以 `ExistingGeneration`（缺日文词典那三个文件）报「输入运行时准备失败」。

## Alternatives considered

- 只从安装包删词库：立刻省空间，但用户无法添加语言，必须同时接上下载和键盘可用性。
- 在键盘扩展联网下载：操作更直接，但延长键盘进程寿命、要求完全访问且放大内存压力；下载交给主应用。
- Swift 另写下载和校验：平台实现灵活，但会重复 Android 与共享层已有资源锁和原子发布，复用 C ABI。

## Consequences

真机 Release `.app` 解压体积由约 340 MB 降至 254 MB；日语、粤拼、注音、笔画数据库均不内嵌。新增资源包单测 4 项通过，模拟器和真机原生构建通过，暂存脚本与本地 quick 通过。联网下载到系统键盘的最终真机验收另行记录，不以构建成功替代。最初落地时 `on_demand_artifacts` 只列 macOS 和 Android，本地和开发构建都完整暂存，所以没有一项验证走到瘦包的运行时校验；发布包装上后键盘无法启动，补上 `ios` 后由 `the_platform_shipping_rule_decides_whether_a_slim_bundle_prepares` 覆盖。验证瘦包要用 `MSIME_IOS_OMIT_ON_DEMAND=1` 暂存后装机，完整暂存的构建证明不了它。

缺词库的语言可以添加，但下载完成前不会切换键盘。App Group 与键盘扩展生命周期不同，取得焦点时刷新资源可用性。iOS 内嵌动态运行库继续随包，当前范围只拆词库。首次下载需要网络；取消和失败均保留重试入口。

## Related notes audit

检索现有 resource、dictionary、pack 和按需笔记：共享资源读写/发布边界相关，沿用实现；Android 语音资源导入部分重叠但本任务不涉及语音运行库；没有已有 iOS 词库下载决策。
