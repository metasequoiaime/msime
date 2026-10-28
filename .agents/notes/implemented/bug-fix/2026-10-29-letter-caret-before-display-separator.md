# Agent Note: Android 字母按键的组字光标画在显示分隔前面

Status: implemented

## Problem

#10133：Android 全拼打 `zuotianjintian`，读音行和放大条显示引擎分节后的 `zuo'tian'jin'tian`。二次编辑时从后往前逐格左移，光标从 `zuo't|ian` 跳到 `zuo'|tian`，再跳到 `zu|o'tian`，始终停不到 `zuo|'tian`。用户看到的是 `o'` 被当成了一个整体，光标到不了 o 后面。

`CompositionCaretPolicy.alignment` 把只在显示里才有的分隔跳过，于是分隔前后两个显示下标对应同一个引擎光标（原始光标 3 同时对应 `zuo|'tian` 和 `zuo'|tian`）。`markIndex` 在这种情况下一律取最后一处，也就是分隔后面。这条规则来自[组字光标点按](../feature/2026-10-08-composition-caret-tap.md)，理由是九宫格的退格先删光标前的切分（`NineKeySession` 的 `splits` 不在 `editing_text` 里）。全拼没有这一步：`crates/engine/src/session/editing.rs` 的 `edit_at_caret` 退格直接删 `editing_text` 里光标前的那个字符，原始光标 3 时删的是 o。画在分隔后面既不符合用户预期，也和退格的实际效果对不上。

## Decision

`markIndex`（`displayOffset` 也走它）按按键种类决定同一个光标对应多处时画在哪里：

- 按键里有九键数字 `2..9`（九宫格，含九键双拼）时取最后一处，仍画在分隔后面（`94'|26`），九宫格的行为和设备断言不变。
- 其余字母按键（全拼、双拼、五笔、粤拼）取第一处，画在分隔前面（`zuo|'tian`）。逐格左移依次是 `zuo't|ian`、`zuo|'tian`、`zu|o'tian`，每一格都停在不同的位置，在 `zuo|'tian` 处退格删的是 o。

用户自己打的 `'` 和粤拼的空格在按键里，按同一个键一一对齐，分隔两侧本来就是两个光标位置，不受影响。`tapTarget`、`alignment` 和引擎都不改：点分隔前后两侧本来就都换算到同一个原始光标；放大条拖动用 `tapTarget` 求当前光标再加减一格，随 `displayOffset` 一起生效。

只改 Android。iOS 的 `editingTextWithCaret` 和鸿蒙的 `PreeditCaretPolicy` 在光标离开末尾时画的是不带分隔的原始按键（`ni|hao`），不存在分隔两侧二选一的问题。

## Historical audit

- [组字光标点按](../feature/2026-10-08-composition-caret-tap.md)：部分取代。九宫格画在切分后面的决定和理由仍成立；字母按键改由本篇决定，那篇的对应条目已改成按按键种类区分并链到这里。
- [Android 拼写放大编辑](../feature/2026-10-21-android-composition-magnifier.md)：无冲突。放大条经 `displayOffset` 画光标，随本篇一起生效；它的设备断言里全拼的光标位置不落在分隔上，九键断言不受影响。

## Alternatives considered

- **按输入方案判断（宿主把 `scheme` 和九宫格开关传进 `markIndex`）**：判据最直接。但 `markIndex` 和 `displayOffset` 的三个调用处（读音行、放大条的 `update` 和 `render`）都只拿着读音和按键，要多传两项状态；而 `alignment` 已经靠按键里的数字认出九键（`sameKey` 只在按键是 `2..9` 时按键位比字母），按同一份按键判断更一致。
- **两边都能停（分隔前后各算一格）**：光标在显示里多一个位置。但引擎的光标只有一个原始位置，左移一格要在宿主里额外记一个「画在分隔哪边」的状态，左右移、退格、点按都要跟着改，收益只是多停一格，没有对应的编辑效果。
- **把显示分隔整个去掉，二次编辑时只画原始按键**：没有歧义，iOS 和鸿蒙就是这样。但 Android 放大条和输入框按[引擎分节](2026-10-23-quanpin-host-preedit-segmentation.md)显示是已定的展示契约，去掉分隔会让长拼写更难读，也超出这个 issue 要求的范围。

## Consequences

- **收益**：全拼等字母按键在显示分隔处能把光标停在前一个音节的末尾，光标位置与退格删掉的字母一致；九宫格不变。
- **代价**：判断九键靠按键里有没有 `2..9`。字母方案的按键将来如果允许混入这些数字，会被当成九键画在分隔后面，只影响画的位置，不影响光标本身和编辑结果。
- **验证**：`ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`，`tests/core/CompositionCaretPolicySmoke.java` 新增全拼用例：`zuo'tian'jin'tian` 对 `zuotianjintian` 的原始光标 3 画成 `zuo|'tian'jin'tian`，`displayOffset` 同值；光标 2、4、7 分别是 `zu|o`、`zuo't|ian`、`zuo'tian|'jin`；画在分隔前面后点光标符两侧和分隔后面仍换算到原光标，点 t 后面后移一格；用户手打的 `'` 和粤拼空格两侧仍是两个位置。九键 `94'|26` 和 `ni'hao` 对 `64426` 的原期望保持不变。没有在真机或模拟器上逐格移动核对过。
