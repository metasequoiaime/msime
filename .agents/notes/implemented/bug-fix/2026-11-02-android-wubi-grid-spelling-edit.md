# Agent Note: Android 五笔 14 键组字不再能点读音行编辑拼写

Status: implemented

## Problem

#10591 要求三端「点读音行编辑拼写」的行为一致。三端对齐方案（PR #10689 的评审结论，用户 2026-11-02 拍板）定下以 Android 的「读音 / 笔图标 → 放大编辑条」为基准，什么时候能点则采用 Android 的 `CompositionCaretPolicy.editable`，再加上 iOS 的「排除五笔 14 键」。

Android 的 `editable` 只看方案、本地模式、专用英文和 `editing_text` 是否为 ASCII，五笔一律可编辑，包括五笔 14 键网格里打的组字。可那时的 `editing_text` 是每组的代表字母（QW QW 记成 `qq`），不是用户打的码，读音行显示的是首选编码（`wq`），两者对不上；引擎在五笔 14 键组字时也不接 `Session::set_caret`（[五笔 14 键](../feature/2026-10-25-wubi-fourteen-key.md)：「14 键组码没有可落脚的光标」）。iOS 早已用 `typesWubiGrid` 关掉这里的拼写光标、拼写菜单和空格拖动，Android 却仍把读音标成「编辑拼写」、点了能打开放大编辑条。

另外，读音行可点区域（[手机键盘放大读音行的点按区域](2026-11-02-mobile-preedit-tap-target.md)）是三端命中区的基准，但它的计算写在 `ImeToolbar` 里、直接读 `View`，没有任何测试覆盖。

## Decision

- `CompositionCaretPolicy.editable` 多一个参数 `gridComposing`，取引擎 View 的 `grid_composing`。方案是五笔且这次组字在网格里时不可编辑。全拼的九键、14 键、18 键和双拼的网格组字不受影响，它们的 `editing_text` 和按键一一对应。网格开着时硬件键盘打的五笔码走 26 键，`grid_composing` 为假，照常可编辑。判据不用 `key_grid`（布局），原因与光标画在分隔哪一侧相同：布局说不出这次组字实际走哪条路。
- `MSIMEInputService.compositionEditable()` 和 `render()` 都把 `grid_composing` 传进去，`render()` 先读 `caretKeyGrid` 再算 `caretEditable`。于是五笔 14 键组字时读音不可点、不画光标、不显示笔图标，读屏念的是编码本身而不是「编辑拼写」；放大编辑条在 `ImeCompositionEditor.render` 里因不可编辑自动关闭；输入框内点击（`editCompositionSelection`）同样不接管。
- 读音行可点区域的计算抽成纯函数 `ReadingRowPolicy.touchTarget(rtl, rowWidth, rowHeight, frameLeft, frameRight)`，返回 left、top、right、bottom。`ImeToolbar.preeditTouchTarget` 只把结果包成 `Rect`，区域本身不变：从行首（含起始内边距）到读音那一段的末端，上下占满整行（含行底和候选行之间的间距）。

## Alternatives considered

- **保留 Android 五笔 14 键的编辑入口，反过来让 iOS 和鸿蒙也放开。** 理由是不收紧任何一端已有的入口。不选：在代表字母里放光标没有意义，用户看到的编码和光标所在的串对不上，引擎也不在这种组字里落光标；打开编辑条只是一个看起来能用、实际改不了什么的界面。
- **按 `key_grid == "fourteen_key"` 判断。** 宿主里随手可得。不选：五笔 14 键开着时硬件键盘打的码走 26 键，`editing_text` 就是用户打的码，可以正常编辑，按布局判断会误伤它。
- **区域计算继续留在 `ImeToolbar`，只靠设备测试。** 设备套件只在模拟器上跑，CI 不跑；抽成纯函数后 JVM 冒烟就能锁住三种排布（从左到右、从右到左、18 键选择条占去右半行）。

## Consequences

- Android 五笔 14 键用户失去一个入口：组字中点读音不再打开放大编辑条。这是三端统一时有意的收紧，与 iOS 相同；鸿蒙按同一规则移植时也要排除五笔网格。
- `editable` 的签名变了，调用方只有 `MSIMEInputService` 两处；日后新增调用方必须传 `grid_composing`。
- 测试：`CompositionCaretPolicySmoke` 补了全拼九键、全拼 14/18 键、双拼网格可编辑，五笔网格不可编辑，硬件键盘打的五笔码可编辑；`ReadingRowPolicySmoke` 补了三种排布下的可点区域。设备套件 `FourteenKeyWubiDeviceChecks` 原先按读屏描述「编辑拼写」找五笔 14 键的读音，现在改为断言读音显示编码、不可点、描述就是编码，并且屏上没有「编辑拼写」。这一改动只经 `check-host.sh` 编译，没有在模拟器上运行。
