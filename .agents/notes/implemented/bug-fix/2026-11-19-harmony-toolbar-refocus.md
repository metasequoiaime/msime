# Agent Note: 编辑器重新聚焦时恢复鸿蒙悬浮工具栏

Status: implemented

## Problem

用户反馈 HarmonyOS 2in1 切换窗口后悬浮工具栏消失。此前宿主只在自己记录的全屏状态变化时才调用 `show()` 或 `hide()`；系统若自行收起状态面板，宿主的缓存仍是「未隐藏」，下次切到非全屏编辑器时就不会请求显示。同一位反馈者还要求全屏应用里打字时也有工具栏，而原有策略在全屏窗口上隐藏它。源代码存在这条可能路径，但没有读取设备日志，不能断言它是用户现象的唯一原因。

## Decision

每次编辑器绑定（`inputStart`，以及键盘面板创建期间到达、创建完成后才挂载的编辑器）都请求 `show()`，调用窗口全屏时也一样，不再查询 `InputClient.getCallingWindowInfo()`，也不再按缓存跳过。这是维护者明确授权的行为变更：Windows（`window_hook.cpp` 的 `OnWinEvent`）和 Linux（`_NET_WM_STATE_FULLSCREEN`）在前台全屏时隐藏工具栏，理由是不让没人在用的工具栏挡住视频或游戏；而鸿蒙这条路径只在有编辑器绑定、也就是用户正在打字时才会走到，那条理由不成立，所以 HarmonyOS 有意与它们不同。这也是 PR 最初一版（f18cf67）的做法和反馈者的要求。

`ToolbarFocus` 串行执行同一面板的显示请求，并用焦点代次丢弃已被新焦点取代的排队请求；编辑器解绑或工具栏被关闭时废止旧代次，旧面板也不再接收请求。显示失败只记录错误，下一次绑定会再试。

`openToolbar` 把面板交给 `ToolbarFocus.beginSetup()`，直到调整尺寸、刷新内容、移到右下角并显示之后才在 `finally` 里 `endSetup()`；这段时间里到达的焦点不显示这个面板，由 `openToolbar` 自己的 `show()` 负责首帧，避免面板先出现在系统默认位置再跳到右下角。

`openToolbar` 在每个 await 之后判断是否放手，依据的是面板归属（`this.toolbar !== toolbar`），而不是 reconcile 代次（`generation !== this.statusPanelGeneration`）。设置页在建面板期间再次触发 reconcile 时，更新的一轮看到 `this.toolbar` 已有面板，只调整尺寸、不定位也不显示，准备期间被丢弃的焦点也不会重放；所以只要面板仍归宿主，原来那一轮就继续把它移到右下角、接上移动队列并显示。`catch` 用同一条归属规则：面板仍归宿主就清掉 `this.toolbar`、作废焦点代次、解绑移动队列和回调后销毁，不论代次是否已变，否则被接手的面板定位失败后会留在宿主里、不在右下角也拖不动，而没有并发 reconcile 时同样的失败却会销毁重建。

## Alternatives considered

- 全屏隐藏（与 Windows/Linux 一致）：每次绑定查询调用窗口，`WindowStatusType.FULL_SCREEN` 时 `hide()`。d1412dc4a3 曾这样实现，未采用：隐藏的理由只适用于没人在打字的场景，而这里总有编辑器绑定；反馈者明确要在全屏时用工具栏，维护者决定鸿蒙保留差异。
- 保留可见状态缓存以减少重复调用。系统可以独立收起状态面板，缓存不能证明真实可见，所以不再据此跳过请求。
- 每个窗口直接异步调用 `show()`，不经队列。代码更短，但没法在面板准备期间或解绑之后丢弃请求，因此保留串行化与过期检查。

## Consequences

- 每次绑定都会请求一次 `show()`，即使面板已可见；代价是多一次系统调用，换来系统自行收起面板后能恢复。
- 全屏视频或游戏里一旦有编辑框获得焦点，工具栏就会出现在全屏画面上，直到用户在设置里关掉工具栏；这是有意接受的差异。
- 若系统的显示调用始终不返回，串行队列会一直等待，之后的焦点请求也无法生效。
- `openToolbar` 期间到达的焦点被丢弃，首帧完全由 `openToolbar` 决定。
- 被更新一轮 reconcile 接手的面板由最初那一轮 `openToolbar` 定位并显示；它中途失败时放手并销毁面板，下一轮 reconcile 重新建一块。

## Verification

- `platforms/harmony/tests/toolbar-focus.test.ts` 用模拟面板覆盖 `ToolbarFocus` 本身：重复绑定都请求显示、旧面板不接收请求、慢显示与后续请求的顺序及被取代请求的丢弃、解绑后排队请求不执行、显示失败后重试，以及面板准备期间焦点不显示、其他面板结束准备不放行、准备完成后照常显示。去掉 `stale()` 里的 `panel === this.settingUp` 后这个测试失败。它不知道调用窗口是否全屏，全屏行为由下面的接线测试覆盖。
- `platforms/harmony/tests/toolbar-setup-adapter.test.cjs`（由 `run.sh` 运行）从 `KeyboardExtensionAbility.ets` 取出真实的 `reconcileStatusPanels`、`openToolbar`、`closeToolbar`、`followCallingWindow`、`started`、`stopped` 和 `onCreate` 里接上待绑定编辑器的那一段，用记录调用的面板替身执行，覆盖：新面板先调整尺寸、移到右下角再显示；调用窗口全屏时每次绑定照常显示，不查询窗口状态、不隐藏；准备期间到达的焦点不抢先显示；被更新一轮 reconcile 接手的面板仍移到右下角、接上移动队列后再显示；被接手的面板定位失败时宿主放手、销毁面板并解绑移动队列，之后的焦点不显示它，下一轮 reconcile 重建；编辑器解绑和关闭工具栏后排队的显示请求不执行；服务创建期间获得焦点的编辑器在面板建好后接上并请求显示。`catch` 仍按 reconcile 代次判断时，定位失败那一条断言失败。
- 设备验证的范围：自签测试包 6000 的 HAP 编译和部分实机检查基于 f18cf67（全屏也显示，但没有准备期间的焦点门控）；自签组合构建 5004、5005 的使用者反馈来自更早的实现。本笔记描述的提交（f18cf67 的全屏策略加上 `beginSetup` / `endSetup` 门控和按面板归属放手）尚未编译 ArkTS HAP，也没有在鸿蒙电脑与真实窗口中验证，不能据合成测试确认设备上的具体故障原因。
