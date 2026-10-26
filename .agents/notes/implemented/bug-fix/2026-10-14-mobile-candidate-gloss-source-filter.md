# Agent Note: 移动端候选释义按来源过滤

Status: implemented

## Problem

共享层 `source_gets_no_gloss` 排除来源 6（表情）、7（颜文字）、13（日文混输）和 14（插件）。iOS 的自选服务请求只带候选文字，含汉字的特殊来源仍会发往在线翻译服务；Android 的用户服务策略漏掉新加入的插件来源 14。iOS 缓存以文字和语言为键，同一文字同时出现在普通候选与特殊来源候选时，即使请求过滤了特殊行，普通行的缓存释义仍可能显示到特殊行或长按菜单。

相关决定见 [Android 用户翻译服务](../feature/2026-10-12-android-user-translation-services.md)、[日文混输来源](../feature/2026-10-12-mixed-japanese-candidates.md)和 [插件候选来源](../architecture/2026-10-13-script-plugin-command-rows.md)。这些笔记分别说明服务、来源和插件的原有设计；本篇记录移动端边界修复。

## Decision

iOS 请求把候选来源与文字按位置传给 `CandidateTranslationStore`。来源缺项时不请求释义；已知来源 6、7、13、14、18 也不请求。候选栏显示、展开面板注释和长按菜单同样按来源过滤，使普通候选的同词缓存不会流到特殊来源行。保留旧的只传文字的 `refresh` 入口供现有调用和测试使用；键盘的产品路径始终传来源。Android 用户服务策略补上来源 14，与共享层集合一致。

宽度也按同一张来源表：这些来源的候选永远没有释义行，所以候选条不为它们预留释义列。iOS 的词块本来就按内容取宽（`KeyboardKeyButton.chipContentWidth`，没有释义行时只按候选字），禁止来源拿不到释义行，自然只有符号本身宽；鸿蒙的胶囊在预期有释义时至少一栏宽（候选栏的三分之一，`CandidateChipWidth`），原先整页一律如此，符号混输打「立方米」「平方米」时「㎥」「m³」「㎡」「m²」各占一栏，一屏只剩三个候选、一个字宽的符号后面空出一大段。鸿蒙改由 `CandidateChipWidth.reservesColumn` 按候选决定：预期有释义，并且来源不在禁止表里（或这一行带韩语汉字的 훈음）才留这一栏，手机胶囊的 `chipMinWidth` 和 2in1 横排的 `candidateChipWidth` 都走它。能拿到释义的候选照旧整栏宽，释义到达时不撑宽胶囊。Android 的候选按钮按内容自适应宽度，释义空缺时用不换行空格占行，不预留列宽，不受影响。

## Alternatives considered

- **仅在网络请求前过滤**：改动最少，也能避免特殊来源消耗用户额度；但以文字为键的缓存会让同词特殊行显示普通行释义，因此显示和菜单也必须检查来源。
- **将来源加入 iOS 缓存键**：可以隔离同词行，但同一普通文字的请求会分散到多个缓存条目，且展开面板的离线释义仍需来源门控。本次保留现有缓存键，在每个消费入口检查来源。

## Consequences

移动端自选服务不会把特殊来源送出；iOS 不再显示或插入这些来源的释义。缺失来源的 iOS 候选保守隐藏释义，避免来源数组短于候选数组时泄露。来源集合仍分别写在 Rust、Swift、Java 和 ArkTS（`TranslationPolicy.sourceGetsNoGloss`）中，新增来源时须同步这几处；鸿蒙的预留列宽也跟着这张表走。鸿蒙开着释义时，能拿到释义的普通词仍各占一栏，一屏三个，这是「释义到达不挪动候选」的代价，本次不改。

## Verification

Android `check-host.sh` 的 230 项 JVM 冒烟及服务检查通过。iOS 定向测试新增普通候选与插件候选同词、四种禁止来源及来源数组缺项的案例；Swift 编译通过，模拟器测试链接因本 worktree 缺少 `target/ios/simulator/libmsime_host_api.a` 而未运行。`git diff --check` 与 `pnpm run verify-notes` 通过。

符号行宽度（2026-10-26）：iOS 模拟器（iOS 27，测试宿主内构造的键盘）九键与 26 键打 `lifangmi`、`pingfangmi`，释义开、关两种情况下符号词块都只有 39–43pt 宽、间距 6pt，没有复现大间隔；`SymbolCandidateChipTests` 把九键这两例钉住。鸿蒙 `tests/run.sh` 新增 `reservesColumn` 的来源用例，并钉住 `chipMinWidth` 的接线；未在鸿蒙设备上验收。
