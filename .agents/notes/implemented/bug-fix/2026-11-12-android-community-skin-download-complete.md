# Agent Note: Android 工具栏社区皮肤走完整下载

Status: implemented

## Problem

Android 输入法工具栏的皮肤面板从 `CommunitySkinCache` 读取未获取皮肤的预览设计。用户点按时，`:ime` 进程直接把这份目录缓存写进 `CustomSkinLibrary`，没有调用 `keyboard_skin_download`，因此包含 `photo` 的完整设计没有被下载，换上的皮肤只有颜色参数。

## Decision

未获取社区皮肤的点按流程按缓存 ID 请求主进程执行 `CommunityCatalog.downloadSkin`，主进程经已有的非导出 `AccountSessionProvider` 返回完整设计 JSON。输入法进程只解析返回值、把完整设计写进共享皮肤库并切换皮肤；下载或保存失败时不写偏好、不记录换肤统计，并保留当前皮肤。缓存里的设计继续只用于列表预览，不能作为安装数据。

`AccountSessionRoutingPolicy` 将 `skin_download` 限定为同 UID 调用，输入法不持有会话、不直接访问社区网络。provider 复用现有主进程会话和 Rust 社区实现，下载失败只返回可展示的错误文案。

## Alternatives considered

- **让 `:ime` 直接调用 `CommunityCatalog.downloadSkin`**：改动最小，也能拿到完整设计；但 Android 宿主约定输入法进程从不联网，直接调用会把网络、账号刷新和失败重试带入按键进程，违背 [Android 按需资源与输入法进程边界](../feature/2026-11-07-all-platform-package-slimming.md) 的既有决定。
- **继续使用目录缓存设计并补齐照片字段**：不需要跨进程契约；但缓存是列表预览且可能过期，无法代替服务端的完整下载和下载计数，仍会漏掉服务端规范化后的字段。
- **让输入法只通知设置应用、由用户重新打开设置完成下载**：守住进程边界，但无法满足工具栏点按后直接使用皮肤的预期，也会保留当前 issue 的可见缺陷。

## Consequences

工具栏点按未获取皮肤时会走与设置社区页相同的完整下载流程，照片底图和其他规范化字段会随设计保存；网络失败不会误切换到只有颜色的半成品。新增 provider 返回 JSON 的跨进程契约，设计大小受 Android Binder 单次传输上限约束，当前社区设计上限低于该上限；若以后提高照片上限，应改为共享文件交接而不是继续扩大 Bundle。

验证覆盖 `CommunityCatalog` 的皮肤 ID 校验、provider 同 UID 路由和既有社区目录策略冒烟。由于本机没有可用 Android SDK 且磁盘仅约 13 GiB，未运行 Gradle、模拟器或真机验收；PR 保持 draft，设备验证缺口写入正文。
