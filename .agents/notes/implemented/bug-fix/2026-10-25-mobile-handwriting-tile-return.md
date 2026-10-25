# Agent Note: 手机「手写」磁贴关掉后回到进入前的键盘

Status: implemented

## Problem

#9703：Android 0.5.0 上默认键盘是全拼 9 键、英文 26 键或小鹤双拼时，点水杉图标 → 手写打开手写板，再点一次关掉，键盘落到全拼 26 键；默认是全拼 26 键时看不出问题。报告者的诊断包里 `touch_keyboard_schemes.enabled` 是 `quanpin, nine_key, xiaohe, …, handwriting`，`selected` 是 `nine_key`。

原因在 `MSIMEInputService.toggleHandwritingScheme`：离开手写时取的是启用列表里第一个不是手写的入口，注释写着「切回上一个中文方案」，代码却从没记过上一个。启用列表按方案的固定顺序排，全拼 26 键排在最前，于是除了它以外的布局关掉手写都回不去。英文 26 键是中英状态而不是方案，进入手写时 `finishSchemeSave` 把英文关掉，回来时也没有人再打开。这段代码从 #3782 引入起就是这样，0.5.0 与 develop 相同。

另两端：鸿蒙的磁贴记了进入前的方案（`schemeBeforeHandwriting`），但不记英文，从英文 26 键或英文 9 键进入手写再关掉会落到中文；iOS 的「手写」图块只进不出，再点一次仍停在手写上，要从方案选择离开。

## Decision

三端的「手写」磁贴（iOS 叫图块）行为一致：

- 手写正在显示（选中的是手写，并且不是英文盖在上面）时点磁贴，回到经磁贴进入手写前的入口；进入前是英文就一并回到英文。
- 其他时候点磁贴进入手写，记下当时的入口和中英状态。英文盖在手写上时点磁贴只退出英文，回到手写板，之前记下的入口仍然有效。
- 经其他途径换方案（方案选择、中英键轮换）时记下的入口作废。不知道进入前的入口时（经方案选择进的手写），离开手写取启用列表里第一个不是手写的入口，与原先一样。记下的入口已被用户关掉时同样按这条走，不回到一个已关闭的入口。

回到哪个入口由三端同名的纯函数决定：Android `KeyboardScheme.handwritingReturn`、鸿蒙 `KeyboardScheme.handwritingReturn`、iOS `InputSchemePreference.handwritingReturn`。记忆只在键盘进程的内存里，不写偏好：进程被系统回收后再关手写，退回上面的兜底。

Android 的方案切换是异步保存，英文要在保存成功、新方案应用之后再打开，所以 `selectKeyboardScheme` 多了一个只给磁贴用的 `english` 参数，经保存回调带到 `finishSchemeSave`，在同一次主线程回调里先应用方案再进英文，中间不画中文帧。

## Historical audit

检索 `schemeBeforeHandwriting`、`toggleHandwriting`、「手写」磁贴与图块，没有记录这个往返的笔记。

- [手机输入方式面板只留一种双拼](../feature/2026-10-09-mobile-picker-single-shuangpin.md)：无关，那是方案选择面板列哪些卡片；本篇不改面板。
- [Android 手写笔输入](../feature/2026-10-12-android-stylus-handwriting.md)：无关，手写笔直接在输入框上书写，不经过手写方案和磁贴。

## Alternatives considered

- **把进入前的入口写进共享偏好** — 键盘进程被回收后也能回到原布局，三端也能共用一个字段。但这是给一个短暂的界面往返加一个跨端同步的持久字段：要进 client-core 的偏好模型、账号同步和三端的读写，偏好文档还会留下一个手写之外毫无意义的值。进程在「打开手写、关掉手写」这两下之间被回收的情况很少，回收后退回启用列表第一个入口，与 0.5.0 之前的行为相同，代价可以接受。
- **离开手写时按 `last_chinese_scheme` 回去** — 偏好里已有这个字段，不用新增状态。但它只记引擎方案：全拼 26 键和全拼 9 键都是 `quanpin`，回不到 9 键；英文也不在里面。
- **iOS 保持图块只进不出** — iOS 本来就不会落到错误的布局。但报告的操作是「再点一次关掉手写」，iOS 上这一下什么都不做，与 Android、鸿蒙的同一个磁贴行为不同，手机键盘行为要三端一致。

## Consequences

- **收益**：默认全拼 9 键、英文 26 键、双拼或其他语言键盘的用户，用磁贴开关手写后回到原来的键盘；三端在同一个操作上行为相同。
- **代价与已知上限**：记忆不跨进程，键盘进程在开关手写之间被回收时仍回到启用列表的第一个入口。在设置 App 里改了方案而键盘停在手写上时，记下的入口只要仍启用就仍会回到它。

## Verification

- Android：`platforms/android/tests/keyboard/KeyboardSchemeSmoke.java` 的 `handwritingReturn` 覆盖入口选法；`check-host.sh` 钉住 `toggleHandwritingScheme` 经 `handwritingReturn` 取入口、并把英文交给方案保存。API 35 专用模拟器上装 full 版 APK，从全拼 9 键、英文 26 键、小鹤双拼各用磁贴开关一次手写，都回到原来的键盘。
- 鸿蒙：`platforms/harmony/tests/keyboard-logic.test.ts` 覆盖 `KeyboardScheme.handwritingReturn`；`tests/run.sh` 钉住 `KeyboardView.toggleHandwritingScheme` 的接线；`hvigorw assembleHap` 编译 ArkTS 通过。没有在鸿蒙设备上验收。
- iOS：`KeyboardTests/input/HandwritingTests.swift` 的 `testHandwritingTileReturnsToTheLayoutItWasOpenedFrom` 在真实的 `KeyboardViewController` 上从全拼 9 键、全拼 9 键上的英文各开关一次手写；`testHandwritingReturnPrefersTheSchemeItWasOpenedFrom` 覆盖入口选法。
