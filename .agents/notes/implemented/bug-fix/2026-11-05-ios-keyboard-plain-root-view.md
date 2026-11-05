# Agent Note: iOS 键盘根视图改用普通 UIView，反复弹出收起不再累积内存

Status: implemented

## Problem

[#10944](https://github.com/metasequoiaime/msime/issues/10944)：iPhone XR（iOS 18.7.11，3 GB）上同一个键盘进程在设置页和试用页之间切换 50 轮，每轮弹出、收起一次键盘，诊断日志 `keyboard_loaded` 的读数从 34 MB 一路涨到 48 MB，`keyboard_shown` 从 36 MB 涨到 67 MB，期间 26 次 `memory_warning`，第 51 次显示时 `mem=76 avail=0` 后进程退出。系统每弹出一次键盘就新建一个 `KeyboardViewController`，所以每轮都是一次「建控制器、显示、收起、释放」。

[iOS 键盘重排内存笔记](2026-11-02-ios-keyboard-reranker-memory.md) 已经查明 `UIInputView` 和 UIKit 挂在它下面的两个 `_UIInputViewContent` 互相强引用、永远释放不掉，当时的处理是控制器释放时摘下全部子视图，认为留下的空壳「约 2 KB」。这次在模拟器里复现并量清楚了，空壳不是 2 KB：

- **在 iOS 18.4 模拟器的系统键盘里复现。** 自己建的模拟器上用 XCUITest 驱动产品 App 的试用页，照 XR 的流程切换 60 轮。纯 develop（不含 #10899、#10894、#10930、#10927）的 `keyboard_loaded` 从第 2 轮的 99 MiB 涨到第 60 轮的 136 MiB，大约每轮 0.6 MiB，与 XR 的斜率相同，所以不是那几个未合并 PR 带进来的。模拟器没有内存上限，`footprint` 也比真机大，只看斜率。
- **留下的是什么。** 在扩展里临时加计数：控制器、会话桥、按键音播放器每轮都释放，`KeyboardInputView` 每轮多一个。iOS 27.0 模拟器上 `heap` 能附着到扩展进程，第 10 轮和第 39 轮的快照之间多了 29 个 `KeyboardInputView`、58 个 `_UIInputViewContent`、29 个 `NSISEngine`，另有约 11 MB 的「non-object」。开 `MallocStackLogging` 后 `malloc_history -callTree` 显示这部分全是 `_table_addStorageBlock`（CoreAutoLayout）：布局引擎为四千来个变量长出来的存储表。在扩展里读 `_layoutEngine`（只在诊断构建里读，没进产品代码）确认引擎的宿主就是控制器的根视图，窗口和上面几层系统视图都没有引擎。摘子视图、停用约束都不会让表缩回去，表随根视图一起留下，每个 400–500 KB。
- **排除了会话。** 引擎会话的建立和销毁在 macOS 上用计数分配器和 `malloc_zone_statistics` 量 40 轮，开不开 ONNX 重排、打不打字，Rust 堆和整个 malloc 区都不涨。

## Decision

- **键盘控制器的根视图是普通 `UIView`。** `KeyboardInputView` 改为 `UIView` 的子类，仍实现 `UIInputViewAudioFeedback`；`KeyboardViewController.loadView` 写 `view = KeyboardInputView()`。普通视图随控制器释放，布局引擎和整棵键盘视图跟着一起释放。
- **不再在 `deinit` 里拆键盘。** 根视图能释放，整棵树随它释放，引擎整个销毁，不经过逐条移除约束，所以删掉 `KeyboardViewTeardown`。#10859 那次 `_engineVar_rawRemove` 断言出在拆除时的引擎迁移和批量停用上，这条路径没有了。`deinit` 只留持锁诊断那一行。
- **扩展里的 `UIInputView` 本来就不画东西。** 键盘底板（iOS 26 起是 Liquid Glass，更早是模糊材质）由系统在扩展窗口外面画。iOS 18.4 和 27.0 模拟器里，原生皮肤下用 `UIInputView` 和用普通视图当根视图的键盘截图除了文本光标和状态栏时钟逐像素相同，打一个字母后的截图也相同；iOS 18.4 模拟器里系统给的键盘高度（`UIView-Encapsulated-Layout-Height`）两种都是 326pt。

## 实测

模拟器的数字只看形状。iOS 18.4、iPhone 16 模拟器，同一套 XCUITest 流程 60 轮，`keyboard_loaded` 的 `mem`（上一轮控制器释放后的残留），单位 MiB：

| 构建 | 第 2 轮 | 第 20 轮 | 第 30 轮 | 第 60 轮 | 根视图存活数 |
|---|---|---|---|---|---|
| develop | 99 | 132 | 124 | 136 | 每轮加一 |
| 本改动 | 46 | 53 | 75 | 75 | 始终一个 |

两组都在第 17 到 21 轮之间有一次 20 MiB 左右的阶跃，发生在进程起来后约 4 分钟，之后不再出现，是进程级的一次性缓存，与轮数无关。本改动在阶跃之后的 40 轮里一直是 75 MiB，develop 继续每轮涨。

测试宿主里在窗口中建、显示、收起、释放三十次控制器，`malloc_zone_statistics` 在用字节的斜率：develop 每次 327 KiB，本改动 -44 KiB（噪声）。

## Alternatives considered

- **输入视图在进程里循环使用** — 根视图释放不掉，就把它和它的引擎交给下一个控制器，引擎的表也跟着复用，内存同样不涨；改动只在根视图上，键盘别处不动。实测内存确实不再涨（计数始终一个），但系统给键盘定的高度每复用一次就多 45pt：326、371、416……第 11 轮时键盘占满全屏，盖住导航栏。复用前把框架清零、停用 UIKit 留下的 `UIView-Encapsulated-Layout-*` 约束、打开 `allowsSelfSizing` 都没有用，说明高度记在系统那一侧、按这个视图对象记着，公开接口清不掉。
- **保留 `UIInputView`，把键盘放进一个能复用的子容器** — 想让子容器自己托管布局引擎、根视图只留几条约束。测试宿主里读 `_layoutEngine` 确认嵌套的 `UIInputView` 和子视图控制器的根视图都不托管引擎，约束一律进最上层根视图的引擎，这条路走不通。
- **用私有接口拆开 `UIInputView` 与 `_UIInputViewContent` 的互相引用** — 碰私有字段，审核和下一版系统都可能出问题。改用普通视图只用公开接口。

## Consequences

- **收益**：反复弹出收起键盘不再每次留下 400–500 KB；控制器释放时不再拆视图树，#10859 那类拆除时的布局引擎断言没有了出处。
- **代价与未验证**：`UIDevice.playInputClick` 要求可见的输入视图实现 `UIInputViewAudioFeedback`，根视图照样实现了，但系统按键声有没有响只能靠人耳在真机上确认，自动化测不到。iPad 的浮动键盘、分离键盘和外接键盘下的键盘高度没有在这次验证里跑过。
- **相关笔记**：[iOS 键盘重排内存](2026-11-02-ios-keyboard-reranker-memory.md) 里「控制器释放时把键盘视图从输入视图上摘下来」那条由本篇取代，那里其余的决定（重排器进程内共用、iOS 关 ONNX Runtime 内存池等）不变。[iOS 原生皮肤](../feature/2026-10-09-ios-native-keyboard-skin.md) 里系统底板的来源已按本篇更正。

## Verification

- `MSIMEKeyboardTests/KeyboardMemoryTests`：`testADismissedKeyboardControllerReleasesItsViews` 现在连根视图一起检查弱引用；新增 `testPresentingTheKeyboardInAWindowThirtyTimesKeepsTheHeapFlat`，在窗口里弹出、收起三十次，第六次起在用字节的最小二乘斜率要小于 100 KiB。两条在 develop 的代码上都失败（根视图和两个 `_UIInputViewContent` 还活着；每次 327 KiB），本改动下通过（-44 KiB）。
- iPhone XR（iOS 18.7.11）真机，develop 加本改动的 Release 构建，同一套 XCUITest 在设置页和试用页之间切换 65 轮，全程同一个键盘进程，`memory_warning` 零次，没有被系统结束。`keyboard_loaded` 的 `mem`：第 2 轮 21 MB，第 11 轮起 32 MB，第 65 轮 32 MB；`keyboard_shown`：第 2 轮 39 MB，第 11 轮起 50 MB（两次 56 MB），第 65 轮 50 MB，`avail` 稳定在 26 MB。第 10 到 11 轮之间的 11 MB 阶跃与模拟器上那次一样只出现一次。修复前同一台设备 50 轮里 `keyboard_shown` 从 36 MB 涨到 67 MB，26 次内存警告，第 51 次显示时 `mem=76 avail=0` 后退出。
