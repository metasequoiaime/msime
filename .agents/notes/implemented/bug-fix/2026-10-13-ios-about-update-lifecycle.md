# Agent Note: iOS 关于页更新检查随页面失效

Status: implemented

## Problem

`AboutView.checkForUpdate()` 用按钮创建独立 `Task` 查询 App Store。离开关于页后，这个任务仍可在超时或返回时写页面状态，并通过全局 `ToastCenter` 在别的页面显示「暂时无法检查更新」。页面重现时，进行中的状态也不应阻止用户重新检查。Android 关于页已记录过相同的生命周期边界，见 [Android 关于页更新状态生命周期](../../implemented/bug-fix/2026-10-11-android-about-update-view-lifecycle.md)。

## Decision

关于页用 `AboutUpdateRequestState` 给每次查询分配代次，并保存按钮任务句柄。退出页面时取消任务、使代次失效；旧请求无论成功或失败都不能写回状态或显示提示。进行中的按钮状态恢复为可重试，已经完成的更新结果继续保留。页面自带的 `.task` 查询同样核对代次和取消状态；按钮发起新查询后，较早的预取不会覆盖它。

## Alternatives considered

- 只在退出时调用 `cancel()`：改动更少，但 Swift 任务取消是协作式的，网络调用返回或抛错后仍可能继续执行状态更新和全局提示。
- 只检查 `Task.isCancelled`：能够挡住被取消的任务，但没有显式请求代次，无法独立测试旧请求与返回页面后新请求的竞争。

## Verification

- 请求状态测试先复现旧请求覆盖新请求，修复后通过。
- `swiftc -parse`、iOS 工程配置测试、笔记门禁与本地 quick 门禁通过；iOS CI 继续验证原生工程与页面编译。

## Consequences

离开页面时放弃尚未完成的检查，用户返回后可以重试；已完成的结果仍保留。商店条目预取与按钮查询两条路径共用请求代次，避免预取晚到覆盖按钮结果。代价是关于页多维护一个小型状态对象和按钮任务句柄。
