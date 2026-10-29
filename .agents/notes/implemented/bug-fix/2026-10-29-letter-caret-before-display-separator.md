# Agent Note: Android 26 键字母按键的组字光标画在显示分隔前面

Status: implemented

## Problem

#10133：Android 全拼打 `zuotianjintian`，读音行和放大条显示引擎分节后的 `zuo'tian'jin'tian`。二次编辑时从后往前逐格左移，光标从 `zuo't|ian` 跳到 `zuo'|tian`，再跳到 `zu|o'tian`，始终停不到 `zuo|'tian`。用户看到的是 `o'` 被当成了一个整体，光标到不了 o 后面。

`CompositionCaretPolicy.alignment` 把只在显示里才有的分隔跳过，于是分隔前后两个显示下标对应同一个引擎光标（原始光标 3 同时对应 `zuo|'tian` 和 `zuo'|tian`）。`markIndex` 在这种情况下一律取最后一处，也就是分隔后面。这条规则来自[组字光标点按](../feature/2026-10-08-composition-caret-tap.md)，理由是九宫格的退格先删光标前的切分（`NineKeySession` 的 `splits` 不在 `editing_text` 里）。全拼没有这一步：`crates/engine/src/session/editing.rs` 的 `edit_at_caret` 退格直接删 `editing_text` 里光标前的那个字符，原始光标 3 时删的是 o。画在分隔后面既不符合用户预期，也和退格的实际效果对不上。

## Decision

`markIndex`（`displayOffset` 也走它）多收一个 `keyGrid`，按组字走的是哪一种退格来决定同一个光标对应多处时画在哪里。宿主在 `MSIMEInputService` 读音行渲染处按引擎的 `key_grid` 不是 `none` 算出它，同一个值传给读音行和放大条（`ImeCompositionEditor.render`、`CompositionEditorView.update`）：

- 组码网格（九键、14 键、18 键、九键双拼、14 键双拼，组字在 `NineKeySession` 里）取最后一处，仍画在分隔后面（`94'|26`、14 键的 `ba'|ba`），网格的行为和设备断言不变。
- 26 键的全拼、双拼、五笔、粤拼取第一处，画在分隔前面（`zuo|'tian`）。逐格左移依次是 `zuo't|ian`、`zuo|'tian`、`zu|o'tian`，每一格都停在不同的位置，在 `zuo|'tian` 处退格删的是 o。

不能按按键里有没有数字来分。14 键、18 键的 `editing_text` 是组码字母（「你好」打成 `bugao`，见引擎金样 `fourteen_key_session.json`），退格却走 `nine_key.rs` 的网格规则：光标处有用户切分时先删切分。14 键打 `ba`、切分、再打 `ba`，按键是 `baba`，读音行是 `ba'ba`（首选读法对不上时退回 `preedit`，同样是 `ba'ba`），光标在 2 时退格删的是切分，必须画成 `ba'|ba`；按「没有数字就是字母按键」画成 `ba|'ba` 正好把 #10133 的错位搬到 14 键上。

用户自己打的 `'` 和粤拼的空格在按键里，按同一个键一一对齐，分隔两侧本来就是两个光标位置，不受影响。`tapTarget`、`alignment` 和引擎都不改：点分隔前后两侧本来就都换算到同一个原始光标；放大条拖动用 `tapTarget` 求当前光标再加减一格，随 `displayOffset` 一起生效。

只改 Android。iOS 的 `editingTextWithCaret` 和鸿蒙的 `PreeditCaretPolicy` 在光标离开末尾时画的是不带分隔的原始按键（`ni|hao`），不存在分隔两侧二选一的问题。

## Historical audit

- [组字光标点按](../feature/2026-10-08-composition-caret-tap.md)：部分取代。组码网格画在分隔后面的决定不变；它的理由（退格先删光标前的切分）只对用户切分成立，读法自带、不是切分的分隔（`9426` 读成 `xi'an`，没打过切分）也画在后面（`xi'|an`），那里退格删的是分隔前的数字 4，是同一类错位。它在本篇之前就存在，不在 #10133 的 Android 全拼范围内，本篇没有改。26 键字母按键改由本篇决定，那篇的对应条目已改成按是否组码网格区分并链到这里。
- [Android 拼写放大编辑](../feature/2026-10-21-android-composition-magnifier.md)：无冲突。放大条经 `displayOffset` 画光标，随本篇一起生效；它的设备断言里全拼的光标位置不落在分隔上，九键断言不受影响。

## Alternatives considered

- **按按键里有没有九键数字 `2..9` 判断**：不用多传状态，`alignment` 也靠数字认九键。PR 第一版就是这样做的，评审指出它把 14 键、18 键当成字母按键（按键是组码字母，退格却是网格规则），见 Decision。按引擎的 `key_grid` 判断，和退格实际走哪个会话一致。
- **只在分隔正好是 `preedit` 里的用户切分时画在后面，其余一律画在前面**：最精确，连九键读法自带分隔（`xi'|an`）的错位也一起修掉。但要把 `preedit` 也传进来并和读音、按键三方对齐，锁定拼写之间的分隔还要另算，改动和要复核的九键设备断言都超出 #10133 的范围，留待单独处理。
- **两边都能停（分隔前后各算一格）**：光标在显示里多一个位置。但引擎的光标只有一个原始位置，左移一格要在宿主里额外记一个「画在分隔哪边」的状态，左右移、退格、点按都要跟着改，收益只是多停一格，没有对应的编辑效果。
- **把显示分隔整个去掉，二次编辑时只画原始按键**：没有歧义，iOS 和鸿蒙就是这样。但 Android 放大条和输入框按[引擎分节](2026-10-23-quanpin-host-preedit-segmentation.md)显示是已定的展示契约，去掉分隔会让长拼写更难读，也超出这个 issue 要求的范围。

## Consequences

- **收益**：26 键的全拼等字母按键在显示分隔处能把光标停在前一个音节的末尾，光标位置与退格删掉的字母一致；九键、14 键、18 键等组码网格不变。
- **代价**：`markIndex`、`displayOffset`、放大条的 `render` 和 `update` 多一个参数。九键读法自带分隔处画的位置和退格仍然对不上（见 Historical audit），本篇没有处理。
- **验证**：`ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`，`tests/core/CompositionCaretPolicySmoke.java` 新增全拼用例：`zuo'tian'jin'tian` 对 `zuotianjintian` 的原始光标 3 画成 `zuo|'tian'jin'tian`，`displayOffset` 同值；光标 2、4、7 分别是 `zu|o`、`zuo't|ian`、`zuo'tian|'jin`；画在分隔前面后点光标符两侧和分隔后面仍换算到原光标，点 t 后面后移一格；用户手打的 `'` 和粤拼空格两侧仍是两个位置。九键 `94'|26` 和 `ni'hao` 对 `64426` 的原期望保持不变；14 键 `ba'ba` 对 `baba` 光标 2 在网格下画成 `ba'|ba`，同样的按键按 26 键画成 `ba|'ba`。没有在真机或模拟器上逐格移动核对过。
