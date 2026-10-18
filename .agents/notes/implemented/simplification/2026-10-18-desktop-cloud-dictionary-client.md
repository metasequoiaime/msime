# Agent Note: 云词库面板共用桌面客户端装配

Status: implemented

## Problem

设置页内面板与独立云词库窗口在 `main.tsx` 重复装配宿主快照能力和移动端单词条下载，设置页初始化还单独拼装相同的能力字段。单词条下载又隐式依赖入口中的个人词库导入实现，无法直接验证编码、词条类型和错误的传递。

## Decision

在 `apps/desktop/src/dictionary/desktop-cloud-dictionary-client.ts` 提供 `createDesktopCloudDictionaryClient`，显式接收面板客户端、宿主平台与个人词库导入能力。三个调用点复用同一份能力装配；设置页已有的 macOS 快照与导出覆盖仍留在调用方，顺序不变。

单词条下载与原入口一致：仅 Android、iOS 增加动作，`quick` 映射为 `quickPhrase`，其余字段照原样放进版本 1 的个人词库 JSON，沿用 `ui-cloud-download-<时间戳>` 请求标识。工厂不发送请求、不改原客户端；实际导入仍由调用时注入的客户端完成。

## Existing notes audit

检索 `cloudDictionaryCapabilities`、`downloadCloudEntryToLocal`、`DesktopCloudDictionary` 与云词库客户端装配。

- [Windows 桌面云账号](../../implemented/feature/2026-10-10-windows-desktop-cloud-account.md)部分重叠，继续持有快照能力和账号会话规则，本项只统一调用方装配。
- [macOS 云词库账号回退](../../implemented/bug-fix/2026-10-18-macos-cloud-dictionary-account-fallback.md)部分重叠，账号请求与错误展示继续由原实现持有。
- [词库顶部标签](../../implemented/feature/2026-10-17-dictionary-top-tabs-and-lookup.md)部分重叠，页内导航与 macOS 专属覆盖继续保留，不下沉到通用客户端工厂。
- 共享轮询笔记只持有 UI 定时器生命周期，会话存储注入提案只持有 Rust 存储与跨进程锁，均与本项无关。

## Alternatives considered

- **只迁移单词条下载函数**：模块最小，入口可以直接导入函数。但两处完整能力装配和初始化装配仍各自维护，因此将现有组合一起收拢。
- **统一到共享 UI 包**：所有宿主都可直接使用同一个工厂。但 HarmonyOS 使用原生下载桥，快照策略也不同；本项继续复用既有桌面能力策略，共享面板只消费客户端接口。

## Verification

平台能力矩阵、macOS 原生文件选择、原动作透传、移动端三种词条映射与导入失败均有直接测试；两个完整装配与初始化能力装配都使用工厂。另执行类型检查、构建、相关测试、笔记校验、quick 和 CI，实际结果记录在 PR。

## Consequences

入口只选择依赖，客户端装配可以独立复用与验证，代价是增加一个小模块。对象展开顺序变化会覆盖既有可选动作，调用方的 macOS 覆盖也不能提前。保留原顺序并验证原客户端未修改、移动端动作替换及非移动端动作保留。此项只验证客户端契约，不代表原生宿主装机或真实账号联网验收。
