# Agent Note: Android 工具栏社区皮肤走完整下载

Status: implemented

## Problem

Android 输入法工具栏的皮肤面板从 `CommunitySkinCache` 读取未获取皮肤的预览设计。用户点按时，`:ime` 进程直接把这份目录缓存写进 `CustomSkinLibrary`，没有调用 `keyboard_skin_download`，因此包含 `photo` 的完整设计没有被下载，换上的皮肤只有颜色参数。

## Decision

未获取社区皮肤的点按流程按缓存 ID 请求主进程执行 `CommunityCatalog.downloadSkin`，主进程经已有的非导出 `AccountSessionProvider` 通过只读管道返回完整设计 JSON。输入法进程只解析返回值、把完整设计写进共享皮肤库并切换皮肤；下载或保存失败时不写偏好、不记录换肤统计，并保留当前皮肤。缓存里的设计继续只用于列表预览，不能作为安装数据。

`AccountSessionProvider.openFile` 将 `skin-download/<uuid>` 限定为同 UID、只读模式和合法作品编号，输入法不持有会话、不直接访问社区网络。provider 复用现有主进程会话和 Rust 社区实现，下载失败只返回可展示的错误文案。

## Alternatives considered

- **Bundle 字符串直接返回设计**：接口简单且小图可用，但合法大照片在 UTF-16 Parcel 中已经超过 Binder 容量；真机负对照复现后改用只读管道。

- **让 `:ime` 直接调用 `CommunityCatalog.downloadSkin`**：改动最小，也能拿到完整设计；但 Android 宿主约定输入法进程从不联网，直接调用会把网络、账号刷新和失败重试带入按键进程，违背 [Android 按需资源与输入法进程边界](../feature/2026-11-07-all-platform-package-slimming.md) 的既有决定。
- **继续使用目录缓存设计并补齐照片字段**：不需要跨进程契约；但缓存是列表预览且可能过期，无法代替服务端的完整下载和下载计数，仍会漏掉服务端规范化后的字段。
- **让输入法只通知设置应用、由用户重新打开设置完成下载**：守住进程边界，但无法满足工具栏点按后直接使用皮肤的预期，也会保留当前 issue 的可见缺陷。

## Consequences

工具栏点按未获取皮肤时会走与设置社区页相同的完整下载流程，照片底图和其他规范化字段会随设计保存；网络失败不会误切换到只有颜色的半成品。共享设计允许 512000 字节照片，Base64 放进 Bundle 字符串后可超过 Binder 的 1 MiB 共享缓冲；Pixel 3 XL 真机用不同进程的合成回复确认 1333412 字节事务被拒绝。因此使用 `openPipeHelper` 的描述符和有界 UTF-8 管道传输，不把照片字符串塞入 Bundle，也不创建临时落盘文件。读写两端都限制回复最多 1 MiB，写端异步避免填满管道后阻塞返回描述符，双方由自动关闭流收回句柄。

设备回归通过独立测试进程调用安装产品中的实际管道实现，验证合法大 PNG 的 Base64 逐字节保留、错误不变成可安装设计、空回复与超限回复被拒绝，以及其他 UID 无法访问产品 provider。入口接入 `smoke.sh` 的系统输入验收之后，PR、API 28 核心和完整矩阵均实际执行；最终构建、真机、模拟器与 quick 的结果写入 PR 验证段，未完成前不合并。
