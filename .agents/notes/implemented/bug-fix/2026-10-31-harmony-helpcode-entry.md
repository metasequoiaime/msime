# Agent Note: 鸿蒙辅助码按键接通与间接触发键的拼写符号发布

Status: implemented

## Problem

HarmonyOS 触屏键盘上，全拼和双拼组字时 Shift 加字母打不出辅助码。`KeyboardView.tapLetter` 把字母以小写交给 Engine，只附带 Shift 标志。Engine 的 `dispatch_character` 只把组字中的大写字母当辅助码，Shift 标志只决定能不能进快捷模式，所以这个字母被当成又一个拼音字母。Android（`Character.toUpperCase(key)`）和 iOS（`character.uppercased()`）都发大写字母，只有鸿蒙从 #2465 起就发小写。

间接辅助码也用不了。双拼的「末尾间接触发」「句中间接辅助码」开关（[直接、间接与句中触发实现](../../implemented/feature/2026-10-22-shuangpin-helpcode-trigger-modes.md)）在鸿蒙设置页「更多选项」里看得到。但触发键 `;` 和反引号只能从符号层按，符号层把它们当标点送出：2in1 符号行走 `punctuation`，runtime 只把 `spelling_symbols` 里列出的符号转给 `dispatch_character`，而 Engine 只为 Tab（[Tab 手动辅助码](../../implemented/feature/2026-10-23-shuangpin-manual-tab-helpcode.md)）发布了标记键，所以组字先按首选上屏，再打出一个分号。触屏 123 层和数字九宫格更直接：中文层的键一律按原样上屏组字再插入键面。全拼的「辅助码触发键」（[Linux 前导键](../../implemented/feature/2026-10-22-linux-helpcode-trigger-key.md)）只在 Linux 显示。

## Decision

三处改动，各管一层：

- **鸿蒙字母键**：`ChineseHelpcodePolicy.letterInput` 在告诉 Engine「带 Shift」时发大写字母。辅助码和英文组合的大写都走这条，与 Android、iOS 相同。双拼「末尾间接触发（大写字母）」「句中触发键（大写字母）」由此一并可用。
- **Engine 发布标记键**：`InputSession::spelling_symbols` 对 Tab、反引号、`;`、`/` 逐个问 `shuangpin_helpcode_marker`。此刻会记成辅助码标记的就列出来，不会时（末尾直接开着时的 `;`、未完成音节、光标不在末尾、全拼）不列。宿主和 runtime 的既有规则是「已发布的符号当字符送进来」，所以 2in1 符号行、外接键盘和各端的 `punctuation` 路由不用改，就能把触发键交给 `dispatch_character`。
- **鸿蒙触屏符号层**：`KeyboardView.typeHelpcodeTrigger` 用 `ChineseHelpcodePolicy.layerTrigger` 把键面（中文层的「；」对应 `;`，其余触发键画的就是它自己）对照 Engine 发布的符号。只在全拼或双拼组字、没有本地模式时成立，而且只认五个可能的触发键（`;` 反引号 `/` `\` `=`），网址的 `.` `:` 和邮箱的 `@` 不受影响。Engine 收下就回到字母层，接下来要打的是辅助码字母；不收才照常上屏键面。123 层（`tapLayerCharacter`）和数字九宫格（`typeNumberPadText`）共用这一条。

宿主能力 `helpcode_trigger_key` 在 HarmonyOS 上也打开，全拼和双拼的「辅助码触发键」出现在鸿蒙手机设置页「输入 › 辅助码 › 更多选项」。Engine 早已在 `spelling_symbols` 里发布前导键，鸿蒙按上面同一条路由把它交给 Engine。

## Alternatives considered

- **宿主自己判断 `;` 是不是触发键**：鸿蒙可以读偏好里的触发方式，自行决定把 `;` 送进 `character`。这样不用碰 Engine。但「此刻是否会开一段」取决于前面是不是完整音节、是不是微软双拼的 `ing` 位置、光标在哪里，这些只有 Engine 知道。宿主再写一份，就会重走[移动端直接辅助码](../../implemented/feature/2026-10-17-mobile-direct-helpcode.md)否掉的「三平台各写一套」。
- **Engine 不看 Shift 标志，宿主照旧发小写**：在 `dispatch_character` 里把「小写字母加 Shift」当大写处理，鸿蒙一行不改。但 Shift 标志现在的含义是「这一键是否可以开快捷模式」，Android、iOS 发的已经是大写字母加 Shift，两种含义叠在一个标志上，会让快捷模式入口和辅助码互相牵连。
- **在 123 层给所有已发布符号开字符路由**：一般性更好，`@` 后面的邮箱、`www` 后面的 `.` 也会进 Engine。但中文层的约定是「按所画的样子上屏」，这会改变与本问题无关的键义，所以只限辅助码触发键。

## Consequences

收益：鸿蒙上 Shift 加字母的辅助码、双拼四种间接触发、全拼和双拼的前导键都接通了，判断仍只在 Engine 一处。标记键发布在共享 Engine 里，Android、iOS 用 `punctuation` 送 `;` 时同样会进 Engine，桌面宿主原本就按字符送这些键，结果不变。

代价与边界：用户把 `;` 设为触发键后，runtime 的「`;` 选第二候选」在会开辅助码段的位置上让给触发键，与微软双拼 `ing` 已有的让位一致。鸿蒙触屏没有反引号键，反引号触发在手机上按不到；分号在中文 123 层和设计稿符号层里有，在中文语境的数字九宫格出厂左列里没有。Android、iOS 的触屏符号层仍按原样上屏，没有接 `layerTrigger` 的对应逻辑；它们的原生设置页也没有间接触发开关。

验证：Engine 单测覆盖四种触发方式何时发布、何时不发布，以及全拼不发布；Host API 用 `punctuation_with_context` 送 `;`，断言记成 `nihc|` 且没有上屏。鸿蒙 `run.sh` 覆盖 `letterInput`、`layerTrigger`，并钉住三处视图接线，`hvigorw assembleHap` 编译通过。没有在鸿蒙真机上按过。
