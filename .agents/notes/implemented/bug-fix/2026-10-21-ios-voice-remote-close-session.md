# Agent Note: iOS 豆包远端关闭后失效传输会话

Status: implemented

## Problem

`DoubaoWebSocketTransport` 的远端关闭回调直接清空 `session` 和 `task`。协调器随后在 `defer` 中调用 `finish()`，也已经拿不到原 session。Apple 的 [URLSession 文档](https://developer.apple.com/documentation/foundation/urlsession)规定 session 会持有 delegate，直到显式失效；只关闭 WebSocket 任务不能代替这个步骤，因此断连后传输对象和会话可能一直保留到 App 退出。

用真实 Foundation session、暂停的合成 WebSocket 任务及失效观察者调用产品回调，再执行 `finish()`，旧实现始终没有发出失效通知。测试不启动任务、不连接远端。

## Decision

保留既有 task 身份守卫和断连状态清理，关闭当前任务时显式失效回调参数中的 session。使用局部参数，避免清空成员后失去失效入口。`finish()`、`cancel()` 和握手失败已经采用相同的 `invalidateAndCancel()`，不增加传输接口、重试或协议状态。

完整 `TransportTests` 另在 macOS Foundation 上运行：用 SwiftPM 临时清单逐字编译两份产品源码和所有现有传输测试，不引入第三方依赖，也不构建原生 App。本地契约门禁自动发现 `scripts/test-ios-doubao-session-lifecycle.py`；iOS CI 的主模拟器作业在原生构建前执行同一入口，改动路径分类也包含该脚本，单独修改 runner 时仍执行测试。原有独立测试 scheme 保留。

## Existing notes audit

检索活跃笔记的 `DoubaoWebSocketTransport`、`URLSession`、`WebSocket` 与豆包关键词，没有同一传输会话释放的决定或提案。[iOS 可见请求代次](../../implemented/simplification/2026-10-13-ios-visible-request-generation.md)只管理页面任务失效；[Android 握手 token](../../implemented/testing/2026-10-09-connection-token.md)只管理另一宿主的握手字段，其余命中属于 Windows、Android、HarmonyOS 的语音功能、配置界面或打包，均与本决定无关。

## Alternatives considered

- 继续只靠协调器 `defer finish()`：能集中正常完成路径的清理，但当前关闭回调已经清空引用，协调器无法失效原 session。
- 关闭时保留成员 session，等 `finish()` 再失效：可以沿用清理入口，但传输层被直接使用或协调器仍在等待另一个并行任务时，会延迟释放；回调本身已明确知道该连接结束。
- 增加工厂或可替换 session 协议：容易记录方法调用，但为单个释放缺口扩大产品注入接口没有必要，真实 Foundation 的失效通知已经可以验证行为。

## Verification

三种远端关闭状态分别覆盖不收尾、随后 `finish()`、随后 `cancel()`，都检查真实 session 失效通知；第四项确认旧关闭回调不会断开替代连接。旧实现的三项失效回归因期待未满足而失败，替代连接控制项和既有五项保持通过。修复后完整九项传输测试通过。测试缺少 macOS 或 Swift 时按仓库约定明确跳过。本地 `bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check` 均通过；独立审查复跑九项测试通过，指出的 runner 路径分类遗漏已修复并复核。

## Consequences

当前连接远端关闭时立即失效其 session，释放 Foundation 对 delegate 的保留，协调器晚些收尾仍保持幂等。本机 Foundation 验证与 iOS 设备验收不同，不直接测量 transport 作为 delegate 时的 ARC 释放，也不覆盖真实最终帧紧邻关闭帧的顺序、麦克风、真实服务或 UI。新增本地门禁只编译两份产品 Swift 文件和传输测试，会增加少量 SwiftPM 编译时间与缓存；产物位于既定 `target/ios/derived-tests` 下。
