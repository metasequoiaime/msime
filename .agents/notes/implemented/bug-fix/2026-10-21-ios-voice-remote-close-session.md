# Agent Note: iOS 豆包传输关闭、迟到回调与会话释放

Status: implemented

## Problem

`DoubaoWebSocketTransport` 的远端关闭回调直接清空 `session` 和 `task`。协调器随后在 `defer` 中调用 `finish()`，也已经拿不到原 session。Apple 的 [URLSession 文档](https://developer.apple.com/documentation/foundation/urlsession)规定 session 会持有 delegate，直到显式失效；只关闭 WebSocket 任务不能代替这个步骤，因此断连后传输对象和会话可能一直保留到 App 退出。

最初用真实 Foundation session、暂停的合成 WebSocket 任务及失效观察者调用产品回调，再执行 `finish()`，旧实现始终没有发出失效通知。后续审计又发现 `didOpen` 无条件写回 session 和 task：关闭之后的迟到打开回调会恢复死连接，替代连接建立后也可能被旧回调覆盖。真实回环握手与二进制往返成功后注入旧打开回调，下一次发送稳定报已取消。Apple 的 [delegateQueue 文档](https://developer.apple.com/documentation/foundation/urlsession/delegatequeue)说明回调运行在 session 的队列上，不能把它与调用方的 `finish()` 当成串行操作。

## Decision

`start()` 是连接的唯一登记入口；把 session、task 和连接状态保存在同一记录中，用 `NSLock` 保护登记、身份检查、状态更新和摘除。打开回调只确认仍登记的同一 session 与 task，Ping 完成同样检查任务身份；旧握手失败或取消只摘除自己的记录。这样已结束的连接不能恢复，也不能把替代连接的状态改掉。

`finish()`、`cancel()` 和远端关闭先原子摘除当前记录，再在锁外取消或失效捕获的 session，避免清空后丢失引用，也避免 Foundation 回调重入状态锁。远端关闭仍立即执行 `invalidateAndCancel()`；不改变传输接口、重试、消息协议或协调器的录音保留策略。

生命周期测试通过 Network.framework 的 WebSocket 协议栈在 `127.0.0.1` 建立真实连接，完成握手、Ping 与二进制收发。关闭测试保持接收方读帧，再等关闭状态与弱引用归零，直接验证 transport 作为真实 session delegate 时的 ARC 释放。未知回调仍可单独注入，但不再借它登记测试连接。等待使用 XCTest 条件期待和截止时间；握手或接收超时后取消任务、传输与监听器并直接退出，不再等待未完成任务。ARC 释放给 15 秒截止时间：本机探针显示 session 立即发出失效通知，transport 仍约 5 秒后才释放，原来的 5 秒上限会与 Foundation 内部收尾撞线。不用固定睡眠猜时序。

完整 `TransportTests` 另在 macOS Foundation 上运行：用 SwiftPM 临时清单逐字编译两份产品源码和所有现有传输测试，不引入第三方依赖，也不构建原生 App。本地契约门禁自动发现 `scripts/test-ios-doubao-session-lifecycle.py`；iOS CI 的主模拟器作业在原生构建前执行同一入口，改动路径分类也包含该脚本，单独修改 runner 时仍执行测试。原有独立测试 scheme 保留。

## Existing notes audit

按 `DoubaoWebSocketTransport`、`URLSession`、`WebSocket` 与豆包关键词检索活跃笔记。迟到回调与线程安全属于本篇的会话生命周期约束，因此就地扩充，不新建笔记。[iOS 可见请求代次](../../implemented/simplification/2026-10-13-ios-visible-request-generation.md)只管理页面任务失效；[Android 握手 token](../../implemented/testing/2026-10-09-connection-token.md)只管理另一宿主的握手字段，其余命中属于 Windows、Android、HarmonyOS 的语音功能、配置界面或打包，均与本决定无关。

## Alternatives considered

- 继续只靠协调器 `defer finish()`：能集中正常完成路径的清理，但当前关闭回调已经清空引用，协调器无法失效原 session。
- 关闭时保留成员 session，等 `finish()` 再失效：可以沿用清理入口，但传输层被直接使用或协调器仍在等待另一个并行任务时，会延迟释放；回调本身已明确知道该连接结束。
- 增加工厂或可替换 session 协议：容易记录方法调用，但为单个释放缺口扩大产品注入接口没有必要，真实回环连接与 ARC 释放已经可以验证行为。
- 只给 `didOpen` 加身份守卫：改动最少，但关闭可在检查与写回之间发生，Ping 完成路径仍会复活关闭状态；共享锁使检查与更新成为同一步。
- 继续用打开回调人为登记测试任务：无需建立监听器，但依赖产品不应允许的状态迁移，无法检查真实替代连接是否还能收发；改用仅绑定回环地址的系统 WebSocket 服务，不向产品增加测试入口。

## Verification

最初修复的三项失效通知回归在旧实现失败，替代连接控制项及既有五项通过，修复后九项通过；当时本地 quick、笔记校验和独立审查通过，并补齐 runner 的 CI 路径分类。后续收发任务错误退出修复另增加三项控制测试。

本轮真实回环正向控制与原有行为控制保持通过，四项新回归在修复前失败：未登记打开、完成后打开、取消后打开、旧打开覆盖当前可收发连接。修复后完整 17 项通过，其中三种远端关闭分别验证不额外收尾、随后完成、随后取消时真实 transport 的 ARC 释放；另验证远端关闭后重新连接和未知关闭不会中断替代连接。最终版再次串行完成旧实现四项失败、修复版 17 项通过的对照；本地 quick、`pnpm run verify-notes`、`git diff --check` 与独立审查通过。审查发现并修复握手和接收 helper 超时后继续无界等待的问题。测试缺少 macOS 或 Swift 时按仓库约定明确跳过。

## Consequences

当前连接远端关闭时立即失效其 session，释放 Foundation 对 delegate 的保留，协调器晚些收尾仍保持幂等。原子登记与摘除增加很小的同步成本，网络等待和失效操作都不持锁。回归现已直接测量 transport delegate 的 ARC 释放；本机 Foundation 与 Network 验证仍不等于 iOS 设备验收，不覆盖真实最终帧紧邻关闭帧的顺序、麦克风、真实服务或 UI。回环监听器只接受本机合成数据，测试结束显式停止。新增本地门禁只编译两份产品 Swift 文件和传输测试，会增加少量 SwiftPM 编译时间与缓存；产物位于既定 `target/ios/derived-tests` 下。
