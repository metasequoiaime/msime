# Agent Note: classicui 横排把候选释义接在同一行，不再占第二行

Status: implemented

## Problem

候选词翻译开着、翻译范围是「仅当前高亮」（`candidate_translation_scope` 的出厂值）时，[候选词翻译的范围](../feature/2026-10-26-candidate-translation-scope.md)让视图只给高亮那一行标 `translation_reserved`，宿主据此在这一行一直留出译文的行数，回答还没到也不缩回去，免得竖排窗口移动高亮时一会儿高一会儿矮。Linux 由 Fcitx5 宿主做：FcitxPage 把 `translationReservedLines()` 交给 `candidateDisplay`，后者给标了 `translation_reserved` 的那一行接上 `"\n　"` 的全角空格占位行，译文到了再把占位行换成 `"\n<释义>"`。

占位行在 classicui 里不是「多出来的高度」，而是一行文字。classicui 的候选行高度来自 `candidateLayouts_[i].text.size()`（行数）：`text.height = fontHeight * max(1, lines)`，整条候选条的高度取整页最高的那一行。行内三段（序号、候选字、注释）又都是 yoga 的 `alignSelf: center`，所以高亮行一旦变成两行：

- 横排候选条本身被顶成两行高，下半截是空白（`#11821` 的两张截图里卡片高 99 px，按 2 倍缩放算一行文字约 19 px，加上 `TextMargin` 正好两行）；
- 候选字按两行的高度居中落在第一行，序号按自身一行高居中落到两行分界附近，看起来就是「序号掉到候选字下面」；
- 这不是主题的问题：`[InputPanel/TextMargin]`、`[InputPanel/Highlight/Margin]`、`LabelTextSizeFactor` 只决定边距和字号，行数由文字决定。报告者把 `Highlight/Margin` 的 Top/Bottom 改成 0、换成 Fcitx5 自带默认主题都没变化，正是因为问题在候选文字里。

`#11821` 的截图里第二行是空白，说明那时还没有译文到达，而高亮行的预留已经生效：视图只在 `candidate_translations` 开着、翻译范围是「仅当前高亮」、这一页请求释义时才标 `translation_reserved`（`crates/input-runtime/src/runtime.rs` 的 `reserve_translation`）。旧设置文档缺 `candidate_translations` 这个键时它读作开启（`crates/client-core/src/preferences.rs` 的 `enabled_by_default`），`translation_socket_` 又默认指向随包 provider 的 socket 路径，所以在没有配置译文服务、或译文只是还没到的这段时间里，`translationReservedLines()` 一直返回至少一行，空白的占位行常驻。就算译文能到，横排的释义本身也会另起一行，同样把整条候选条和那一格的序号顶坏。

竖排里留高是设计本身（Windows、macOS、HarmonyOS 2in1 都按 `translation_reserved` 留高，macOS 的横排还在整张卡片上留高），classicui 的横排没有整卡留高的手段，也不能把序号单独对齐到第一行——`alignSelf` 是 classicui 写死的。

## Decision

`platforms/linux/src/candidates/CandidateTranslationPolicy.h` 新增两条规则，由 FcitxPage 把 `layout_ == Vertical` 传给 `candidateDisplay`：

- `classicui_gloss_placeholder_lines(vertical, reserved)`：占位行只在竖排加，横排返回 0。
- `classicui_gloss_inline(vertical)`：横排把释义接在候选字后面同一行（`" · " + 释义`，与 IBus 查找表同一写法），竖排仍是下一行。

竖排行为不变：占位行、译文下一行、读音再下一行都照旧。原生画布（X11 画布、canvas 皮肤）那条路径也不动——它按 `content` / `item.reserved_gloss_lines` 在整张卡片或整行上留高，本来就没有这个毛病。

横排的代价是：释义到了以后候选条的宽度会随释义变长（高度始终一行），停用译文时横排的释义出现在候选字右边而不是下面。这是 classicui 横排唯一能同时保住「高度一行、序号不错位」的画法。

## Alternatives considered

**横排只去掉占位行，释义仍另起一行。** `#11821` 截图里那种常驻空白会消失，但译文真到的时候那一格又变成两行、序号又掉下去，等于把常驻的错位换成随译文到达的错位；报告者开着翻译时仍会看到「选中就变高」。

**横排也给整页每一行都加占位行，让候选条高度统一。** 高度确实不再跳，但每一行的序号都会像报告里那样掉到第二行、每行下面都留一行空白，从「一格坏了」变成「整条候选条都坏了」。

**横排干脆不翻译高亮行。** 翻译范围的作用就是给高亮行译文；为了布局关掉一条已经上线的功能，代价和收益不成比例。

**改主题把序号的对齐改到行顶。** classicui 的 `alignSelf: center` 是写死的，主题里只有字号和边距，没有对齐；改不了。

## Consequences

- 横排经典界面的候选窗始终一行高，被选中那一格的序号回到候选字左边；不管有没有译文、译文到没到。
- 横排的译文变成「候选 · 释义」，与 IBus 查找表一致，也与「显示读音」的接法一致；竖排经典界面、X11 原生画布、canvas 皮肤、IBus 宿主的画法不变。
- 横排开着「仅当前高亮」时，移动高亮不再改变候选条高度；候选条宽度会在释义到达时变长一行文字的量。
- 验证边界：`linux-candidate-translation-policy` 的占位行与同行规则是本机可跑的纯函数断言；Fcitx5 宿主的 `classicui` 文本构造由容器里的 `fcitx5-*` 单测覆盖（`platforms/linux/fcitx5/tests/native.cpp` 的 `reservedTranslationRow`）。没有 KDE Wayland 桌面，classicui 的画面复验留给 `#11821` 的报告者用后续构建确认。
