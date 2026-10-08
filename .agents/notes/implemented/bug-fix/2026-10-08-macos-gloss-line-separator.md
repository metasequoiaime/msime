# Agent Note: macOS 多语言释义经会话传递时用 U+2028 分行

Status: implemented

## Problem

macOS 宿主在候选下按目标语言逐行显示释义：设置了第二释义语言（`translation_secondary_language`）时，同一个候选的英文行和日文行在宿主内存里用 `"\n"` 连接，再整批交给 `msime_client_apply_translations`。共享会话每个候选只存一条释义，并按 `is_bounded_text` 拒收所有控制字符，`"\n"` 也算。于是只要页面上有一个候选同时有两行释义，整批释义就被拒收，候选下一条释义都不显示，关闭在线服务、只用随包的 `msime-english.db` 和 `offline-glosses/zh-ja.db` 时也一样（#5598）。控制器测试用的替身会话不做这项校验，所以测试一直是绿的。

## Decision

会话契约不变，由 macOS 宿主在边界上适配。`InputController.mm` 的 `MSIMESessionTranslations` 在 `applyCandidateTranslationResults` 调 `applyTranslations:` 前处理每条释义：先把来源自带的 U+2028、U+2029 折成空格，再把行分隔 `"\n"` 换成 U+2028 LINE SEPARATOR（Unicode Zl，不是控制字符，会话照收、原样交回）。唯一读取会话释义的 `CandidateTranslation` 把 U+2028 换回 `"\n"`，卡片排版、`MSIMECandidateTranslationColumn` 按列上屏和义项页都经由它，所以宿主内部仍按 `"\n"` 分行。

先折掉来源自带的分隔符是必要的：自定义接口、本机模型、账号或词典返回的释义里若本来就有 U+2028，读回时会被当成换行，第二语言被挤到第三行，按列上屏也会取错列。

测试替身 `CustomTranslationSession` 现在和真实会话一样拒收含 Unicode Cc 字符的释义，`TextClientTest` 用真实会话钉住「`"\n"` 被拒、U+2028 原样往返」，`TestGlossLinesSurviveSession` 覆盖编码、读回和按列取值。

## Alternatives considered

- 放宽 `msime_client_apply_translations`，让释义允许 `"\n"`：会话视图是所有宿主共用的，Windows 用 `" / "` 拼接、iOS 和 HarmonyOS 把非英文释义留在宿主侧，放开换行等于给它们都引入一种没人处理的格式，而控制字符拒收本身是有意的输入边界。
- 把第二语言释义留在宿主侧、不经会话（iOS 的做法）：要重写 macOS 释义的合并、缓存和渲染路径，改动面远大于这个缺陷。
- 外部贡献 PR #2607 采用同样的 U+2028 方案，但没有处理来源自带的 U+2028；维护者评审建议在 `format_translation_gloss` 里折叠它。那个函数只覆盖经它格式化的在线来源，随包词典和账号释义不经过它，所以这里在 macOS 交给会话的唯一入口统一折叠。

## Consequences

设置两种释义语言后，随包已有两种释义的候选同时显示两行；只有一种语言时行为不变。代价是 macOS 宿主与会话之间多了一条私有编码约定：任何新的读取路径都必须经过 `CandidateTranslation`，直接读视图里的 `translation` 会看到 U+2028。释义原文里的 U+2028、U+2029 会显示为空格。
