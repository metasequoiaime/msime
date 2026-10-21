# Agent Note: iOS 实时语音首个收发错误立即退出

Status: implemented

## Problem

`DoubaoVoiceCoordinator.runLive` 将录音发送和识别接收放进同一任务组，然后调用 `waitForAll()`。该 API 会等所有子任务结束后才抛出首个错误，不会因单个子任务失败而取消其他任务。接收失败时，发送任务仍等尚未结束的录音流，设置页直到用户停录或录音限时才显示失败；发送失败时，接收任务可能仍等待外部传输回调。

[Apple 的 waitForAll 文档](https://developer.apple.com/documentation/swift/throwingtaskgroup/waitforall())明确规定该等待行为。合成接收错误和未结束的 `AsyncStream` 已复现错误不返回；仅逐个读取结果又不能恢复等待外部回调的接收 continuation。

## Decision

逐个读取子任务结果，让首个错误进入协调器的错误处理。错误路径先调用 `transport.finish()`，恢复仍在等待外部传输回调的收发任务，再抛出原错误，让任务组取消其他子任务并等待它们退出。外层 `defer finish()` 保留，覆盖正常结束及其他路径；当前传输实现收尾幂等。

正常路径仍等发送、接收双方完成，保持最终音频包和文字代次的语义。设置页继续保留本机录音，供停录后重新发送；本次只结束流消费和传输，不改变录音器或界面策略。

## Existing notes audit

检索活跃笔记的 `DoubaoVoiceCoordinator`、`waitForAll`、`任务组`、`AsyncStream`、豆包和实时语音。没有持有该收发任务组错误退出决定的笔记。[iOS 可见请求代次](../../implemented/simplification/2026-10-13-ios-visible-request-generation.md)只管理页面任务与迟到结果，属于相邻边界；其余命中属于其他平台的语音功能、设置与配置、设备采集或请求格式，与本决定无关。

## Alternatives considered

- 保留 `waitForAll()`，等待录音自然结束：成功路径无需改动，但网络已经失败时继续消费录音会延迟错误反馈，接收等待也没有退出保证。
- 只改成逐个读取结果：能让接收错误取消 `AsyncStream` 消费，但任务取消标记本身不能恢复由外部回调驱动的 continuation；发送失败仍可能卡在任务组排空前，外层收尾不可达。
- 用新的传输协议或包装层表达中止：可以区分结束与异常，但现有幂等 `finish()` 已能关闭 WebSocket 和失效 session，不需要扩大产品接口。

## Verification

合成接收失败时，未结束的录音流自动停止消费并返回原错误；合成音频发送失败时，等待外部回调的接收由传输收尾恢复；成功路径仍发最终音频包并应用正确代次的文字。所有等待均以 XCTest 事件驱动，并在失败断言后解除等待，避免测试进程挂起。旧实现的两个失败退出用例都超时且未收尾，成功控制项和原有五项通过。修复后八项测试全部通过，独立审查逐字核对生成输入并复跑八项通过。同步远端关闭会话修复后的完整十二项测试也由仓库 runner 自动发现并全部通过；`bash scripts/verify-local.sh --quick`（包含 Linux 容器编译）、`pnpm run verify-notes` 和 `git diff --check` 通过。

## Consequences

收益是实时语音不再等待其他已无意义的收发工作才能报告错误。代价是错误路径在组内和外层分别调用幂等收尾；新增测试使用受锁保护的合成传输与真实 Swift 并发，不连接远端、不访问麦克风，也不等同于 iOS 设备或真实服务验收。
