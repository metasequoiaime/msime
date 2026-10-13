# Agent Note: iOS 账号释义缓存绑定登录代次

Status: implemented

## Problem

`TranslationRoute.account.cacheScope` 只有固定的 `account` 字符串。键盘扩展进程可能在账号设置页退出或切换账号后继续存活，而 `BackendCandidateTranslationService` 每次请求都会重新读取共享会话；旧账号已经缓存的候选释义因此会被新账号会话沿用。

## Decision

账号释义缓存 scope 绑定 `BackendAccountSession.login()` 返回的 `sessionID`。这个 ID 在同一次 token 刷新中保持不变，每次登录都会重新生成。键盘同步账号翻译路由时先进入待确认 scope，清空旧释义并暂停账号请求；异步读取当前登录代次后，绑定实际 scope 并立即刷新候选条。未登录或读取不到会话时使用独立的匿名 scope，避免沿用账号释义。

## Alternatives considered

- **继续使用固定 `account` scope**：账号切换后无法区分旧缓存，保留了跨账号显示风险。
- **每次键盘出现都无条件清空账号缓存**：可以避开旧缓存，但会丢掉同一账号的可复用释义，且不能表达账号会话边界。

## Consequences

同一登录代次内可复用候选释义；退出、重新登录、切换账号或切换到匿名会话时旧释义会被丢弃。身份查询期间不发送账号释义请求；每次键盘重新同步账号路由都会清理一次缓存，以关闭异步查询前短暂绘制旧账号释义的窗口。

## Verification

新增 `TranslationProviderTests.testAccountCacheScopeChangesWhenTheLoginChanges`，断言不同登录代次得到不同 scope、同一代次保持稳定。XcodeGen 工程已生成，Swift 编译阶段成功编译了 `KeyboardViewController.swift` 及测试源；完整模拟器测试被当前 worktree 缺少 `target/ios/simulator/libmsime_host_api` 阻断，`xcodebuild` 明确报告 `Library 'msime_host_api' not found`。
