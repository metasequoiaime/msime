# Agent Note: Android 长按候选按义项上屏释义

Status: implemented

## Problem

Android 长按候选弹出的菜单里，每一项就是一条可上屏的释义，来源是 `CandidateTranslationPolicy.insertionGlosses(translation)`。它只按换行切，换行是 `joinGlosses` 写进去的「每种目标语言一行」。Engine 的 `candidate_gloss_display`（`crates/engine/src/host/glosses.rs`）把英文释义的前两个义项用 `"; "` 连成一条，所以「我的」到宿主是 `my; mine`，菜单只有一项，点了连分号一起上屏（#6997 第 1 项）。macOS、Linux、Windows 都经 `shared/input/GlossSenses.h` 的 `gloss_senses` 按分号切开，鸿蒙有自己的 `TranslationSensePolicy.split`，只有 Android 没有切义项这一步。

## Decision

`insertionGlosses` 先按换行切成语言行，再把每行按 `;` 或全角 `；` 切成义项，逐项去首尾空白，保留原有的过滤（空项、重复、超过 4096 字节、控制字符、无效 Unicode）。顺序是语言行在前、行内义项在后。重复判断跨行进行，两种语言给出同一个义项时菜单只列一次。

上限从「两行」改成新常量 `MAX_INSERTION_SENSES = 9`，与鸿蒙的 `MAX_SENSES` 相同；`MAX_TARGETS` 不动，它还是 `targets`、`offlineTargets`、`mergeGlosses` 的语义上限。计数时给后面的语言行各留一个位置，第一种语言有九个以上义项时第二种语言仍至少出现一项。

菜单（`ImeCandidates.showCandidateMenu`）和两条上屏路径（`insertCandidateGloss`、`insertExpandedCandidateGloss`）本来就只上屏被点的那一项，不用改。

## 与 gloss_senses 的差别

分号规则与 `gloss_senses` 一致；换行处有意不同。`gloss_senses` 把换行当义项内部的折行，Android 的换行是语言分隔，所以先按换行切。去空白用 `String.trim`，比 `gloss_senses` 只去空格、制表、回车、换行宽一些，影响只在首尾的其他 ASCII 控制字符，而它们本来就被控制字符过滤拒掉。

## Alternatives considered

- 经 JNI 调 C++ 的 `gloss_senses`：规则只有一条正则，Android 的原生层 `client_jni.cpp` 不包含 `shared/input`，为它接一条 JNI 通道得不偿失；鸿蒙同样在宿主侧重写了这条规则。
- 只在整串上按分号切、不先按换行切：`my; mine\nmon` 会切出 `mine\nmon` 这样跨两种语言的一项。
- 不留位置、按顺序数满九项：在线账号译文的义项不受 `candidate_gloss_display` 两项上限约束，一条长的主语言译文会把第二语言整行挤出菜单。

## Consequences

`我的` 长按后菜单是 `my` 和 `mine` 两项，点哪项上屏哪项。代价有两点：菜单不再按语言分组，两种语言共有的义项合并成一项；账号或在线译文里本身含分号的整句也会被切成几项，这和桌面、鸿蒙的行为一致。用逗号分隔义项的在线译文仍然是一项。

iOS 的 `KeyboardViewController.candidateGlosses(at:)` 同样按语言整条上屏，没有切义项，属于另一个平台，不在这次改动里。本地模式、方案 3 和韩语关闭长按上屏（`candidateGlossInsertionEnabled`）是现有设计，这里不动。候选提示位被辅助码占用时译文只在长按里出现的问题另见 [辅助码和释义共用一行](2026-10-11-android-helpcode-gloss-share-hint-row.md)。实体键盘的 Ctrl+Enter 上屏译文用同一份义项列表，见 [Ctrl+Enter 上屏候选译文](../feature/2026-10-12-android-ctrl-enter-translation.md)。

## Verification

`platforms/android/tests/candidate/CandidateTranslationPolicySmoke.java` 新增分号、全角分号、空项、重复、跨行去重、九项上限和第二语言保位的用例；改动前第一条新用例 `my; mine` 失败，改动后整个冒烟通过，原有三条按行用例不变。设备上长按的菜单项和上屏结果没有在模拟器上跑。
