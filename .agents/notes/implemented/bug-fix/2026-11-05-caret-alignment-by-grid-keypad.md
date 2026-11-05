# Agent Note: 读音对齐按这次组字的键盘组码

Status: implemented

## Problem

#10591 在 iOS 上发现、Android 同样存在：全拼 14 键打 `nihao`，放大拼写条画的是组码 `bugao`，不是读音 `ni'hao`；点字母也没法按读音放光标。

14 键、18 键的 `editing_text` 是每组首字母的组码（引擎 `crates/engine/src/nine_key.rs` 的 `FOURTEEN_KEYPAD`、`EIGHTEEN_KEYPAD`，经 `KeyGrid::code_of` 取），`n` 在 14 键上打成 `b`。Android `CompositionCaretPolicy.sameKey` 只认「同一个字母」和「九键数字键上的字母」，`n` 对不上 `b`，`alignment` 失败，`ImeCompositionEditor` 于是退回画原始按键，读音行光标离开末尾时同样退回组码。数字规则还对所有键盘一律生效，与这次组字实际在哪种键盘上无关。

## Decision

`alignment` 多收一张组码表 `keypad`：`a..z` 各自所在键的组码，读音的字母在这张表里的组码等于按键就算对上，否则只认同一个字母（不分大小写）。表由 `CompositionCaretPolicy.keypad(scheme, key_grid, grid_composing)` 按引擎 View 取：

- 全拼且 `grid_composing` 为真：`nine_key` 取九键数字表，`fourteen_key` 取 14 键表，`eighteen_key` 取 18 键表，三张表与引擎逐字相同。
- 26 键（包括网格开着时硬件键盘打的字母，`grid_composing` 为假）为 null，只认同一个字母，`n` 对不上 `b`，字母也不再对得上数字。
- 双拼网格（九键、14 键、18 键双拼）为 null：一个音节两键、按方案编码，读音的字母与按键不是一一对应，对不上时照旧退回画原始按键。

`MSIMEInputService` 在读音行渲染处取一次表，交给读音行的 `markIndex`、`ImeCompositionEditor.render` 和 `CompositionEditorView`（点按、拖动、边缘滚动都经 `tapTarget`）；输入框里的组字内点按（`editorTapTarget`）同样带表。

五笔 14 键不能编辑：`editable` 多收 `grid_composing`，五笔在网格里组字时为假。它的按键是各组的代表字母（QW QW 是 `qq`），不是用户打的码，画出来没有意义；iOS 的 `editableSpelling` 早就排除了它（`typesWubiGrid`），这里补齐。iOS 按画出来的键面判断，Android 按 `grid_composing`，只在五笔 14 键接硬件键盘时不同：那时按键是真实字母，Android 仍可编辑。

规则与 iOS（#10894）相同。鸿蒙 #10689 不受影响：网格组字时 `SchemeCompositionPolicy.reading` 画引擎的 `nine_key_reading`、光标钉在末尾（`SchemeCompositionPolicy.caret`），没有读音与按键的逐字对齐；14 键、18 键的读音留在候选栏（`keepsReadingOnStrip`），不走点读音编辑的入口。

## 测试

- `CompositionCaretPolicySmoke`：14 键 `ni'hao` 对 `bugao`、18 键 `ni'hao` 对 `bihai` 的点按、画光标和输入框点按；两张字母表用 26 个字母整串核对；换一张表（14 键对 18 键、九键对 14 键）对不上；26 键 `n` 对不上 `b`、字母对不上数字；双拼网格和五笔 14 键没有表；五笔 14 键不能编辑。
- 设备上的 `GridSpellingEditorChecks`（`FourteenKeyDeviceSmoke` 和十八键全拼都调用它）：组字后点读音行，放大条画读音、不画组码，点第二个音节前光标落在分隔后面，「完成」收起。改前的包在第一步就失败，放大条画的是 `bugao`。
- `FourteenKeyWubiDeviceChecks` 原来用「编辑拼写」这个描述找五笔 14 键的读音行，这次读音行不再可编辑，改为按编码文字找，并加一条断言：五笔 14 键的读音行不提供拼写编辑。断言的读音内容（qq、qw、wq、ww 之一）不变。

## Historical audit

- [组字光标点按](../feature/2026-10-08-composition-caret-tap.md)：部分取代。对齐规则里「九键字母按键位对上数字」改为按这次组字的组码表，那篇的对应句已改并链到这里。
- [26 键字母按键的光标画在显示分隔前面](2026-10-29-letter-caret-before-display-separator.md)：无冲突。`keyGrid`（画在分隔哪一侧）与组码表是两件事：双拼网格 `keyGrid` 为真而组码表为 null。
- [Android 拼写放大编辑](../feature/2026-10-21-android-composition-magnifier.md)：无冲突。放大条「读音对不上时画原始按键」的兜底不变，只是 14 键、18 键不再落到兜底。
- [九键未覆盖尾段的读音](2026-11-02-nine-key-uncovered-tail-reading.md)：无冲突，九键仍按数字表对齐。

## Alternatives considered

- **在 `sameKey` 里把三张表都试一遍，不传布局**：调用处不用改。但 18 键 `nihao` 的组码 `bihai` 里 `i` 既是 18 键的 `o`，也是 26 键的 `i`，不看布局就可能把别的键盘的按键当成对上，放出错的光标；26 键也会继续把数字当成字母的按键。
- **按 `key_grid` 取表，不看 `grid_composing`**：少读一个字段。但网格开着时硬件键盘的字母照样走 26 键，按 `key_grid` 会把真实字母 `n` 当成 14 键组码，与 `bugao` 这样的组合串误对齐，正是 [26 键字母按键的光标](2026-10-29-letter-caret-before-display-separator.md) 里被评审否掉的同一个错误。
- **让引擎直接给出读音与按键的对应**：最准确，双拼网格也能编辑读音。但要改 View 协议和三端宿主，超出这个缺陷的范围。

## Consequences

- **收益**：全拼 14 键、18 键的放大条和读音行画读音（`ni'hao`），点字母按读音放光标；26 键不会再把不同的字母或数字当成同一个键。
- **代价**：`alignment`、`tapTarget`、`markIndex`、`displayOffset`、`editorTapTarget` 和放大条的 `render`、`update` 多一个参数；组码表在 Android 和引擎各有一份，引擎改分组时要同步（`CompositionCaretPolicySmoke` 用 26 个字母整串核对两张字母表）。五笔 14 键不再能点读音编辑。
