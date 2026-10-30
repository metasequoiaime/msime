# Agent Note: 鸿蒙版在拿到 AI 生成类资质之前隐藏全部 AI 生成功能

Status: implemented

## Problem

华为应用市场（中国大陆）审核 `app.msime.hmos` 时以三条理由驳回：

- 应用内容：「应用内【AI对话】为用户提供合成生成内容服务前，未进行真实身份信息认证」。
- 应用资质：「应用缺少AI文本生成的资质证明文件（《安全评估报告》…）… 若应用内已无相关模块，可在…应用审核信息中说明」。
- 「应用含AI生成文本内容，但未提交人工智能生成合成内容标识相关材料」。

鸿蒙版当时有七处会把用户的文字发给大模型、再展示或上屏生成内容：设置页的 AI 对话（含「试用键盘」里的发送）、AI 皮肤、AI 辅助页的润色测试与模型列表；键盘的高情商回复、AI 润色、打字时的 AI 联想候选、「AI 辅助」候选翻译（含 `/fy`）和语音识别后的 AI 润色。维护者决定资质到位之前鸿蒙版一律不提供这些功能，好在审核信息里写明应用内已无 AI 生成模块；资质到位后要能原样恢复，所以只加闸、不删代码。

## Decision

总闸是一个宿主能力：`crates/client-core/src/host_surface.rs` 的 `HostCapabilities.ai_generation`，`for_platform` 里只有 `HostPlatform::Harmony` 为 false，其余五个平台为 true。鸿蒙两个进程和共享设置页都读这同一个值，没有第二个开关。

- **共享设置页**（`packages/ui`）：`settingsCapabilities` 派生 `showAiGeneration = host?.ai_generation !== false`，宿主没报或没有宿主时照旧给出。它为假时：
  - 页面投影去掉「AI 辅助」（`hasAi`）和「AI 对话」（`hasChat`），偏好表单不挂 AI 页，搜索也搜不到；路由到 `ai`、`chat` 落回默认页；`useAiAssistant` 拿不到客户端，不发模型列表和润色测试。
  - 「试用键盘」不带 AI 对话：触屏首页打开不带发送键的试用页（[试用键盘子页面](2026-10-08-touch-try-keyboard-page.md)本来就是给没有对话的宿主准备的）。
  - 语音页不画「文本润色」组（`showVoicePolish`）；翻译服务不列「AI 辅助」，存下的 `translation_ai` 显示成没有选在线服务；皮肤页没有 AI 皮肤入口；「标点与翻译」页不画高情商回复说明；社区不列回复模板分类，「我的内容」不列回复模板的两行（回复模板是高情商回复的提示词）；触屏「隐私」页那句改为只说语音；打字统计的方案与模式分布不列「AI 润色」「高情商回复」两类，模式分布的说明也不提 AI（`statisticsDistributionSlices` 的 `aiGeneration`），历史计数照旧留在统计文件里，只是不画出来。
- **鸿蒙设置宿主**（`apps/harmony/src/main.tsx`）：`ai_generation` 为 false 时不给页面 `chat`、`aiSkins`、`aiAssistant` 三个客户端。
- **鸿蒙原生一侧**：`platforms/harmony/entry/src/main/ets/keyboard/AiGenerationGate.ts` 在键盘进程（`KeyboardSession.start`）和设置进程（`Settings.ets` 桥接的 `attach`）启动时读 `client.hostCapabilities`，只有明确为 true 才打开，`adopt` 之前或回答读不懂时按关闭处理。
  - 功能面板：`FunctionPanelPolicy.items(AiGenerationGate.enabled())` 去掉「高情商回复」和「AI 润色」两格。
  - 请求点：`HarmonyVoicePolisher.polish`（语音润色和面板 AI 润色共用）、`KeyboardSession.thoughtfulReplies`（高情商回复）、`fetchOnline` 的 AI 联想分支、`TranslationPolicy.provider`（AI 翻译当作没有服务，只出离线释义）在发请求前都问它，所以从别的设备同步来、打开了 AI 的偏好也不会触发请求。
  - 设置桥：`startRequest` 先问 `AiGenerationGate.allowsSettingsRequest`，拒绝 `chat`、`ai_skin`、`ai_test`、`ai_models`、账号通道上的 `chat` / `chat_models`，以及探 `ai.assistant`、`voice.polish`、`translation.ai` 的配置测试，回 `ai_generation_unavailable`。

不归这道闸管、照常提供的：系统语音识别和豆包、HTTP ASR、本地模型等语音识别服务（只做语音转文字，含百炼 `qwen3-asr-flash` 走 chat-completions 接口的那一种）；云候选（候选服务器返回拼音候选）；腾讯云、小牛、DeepL / DeepLX 机器翻译和离线释义；本地神经联想和手写模型。

## 重新打开

资质（实名认证流程、《安全评估报告》、生成内容标识材料）到位后，把 `host_surface.rs` 里 `ai_generation: platform != HostPlatform::Harmony` 改成 `true`，同步改 `only_harmony_withholds_ai_generation` 测试和 `apps/desktop/tests/support/host.ts` 的 `ai_generation: !harmony`，再按审核要求补上 AI 对话前的实名认证与生成内容标识。其他代码不用动：页面、宿主客户端、键盘格子和请求点全部跟着这一个值回来。测试里带 `ai_generation: true` 的用例覆盖的就是那时重新出现的界面。这道闸也决定鸿蒙的社区列不列 AI 生成的皮肤（`exclude_ai_generated`，见[鸿蒙版关闭社区投稿](2026-10-28-harmony-withhold-community-contribution.md)）：打开之前要先让社区画出「AI 生成」标识。

## Alternatives considered

- **保留功能、补齐资质**：最强的理由是这些是鸿蒙版有差异化的功能，隐藏会让鸿蒙用户少一块体验，而且迟早要补。不选是因为《安全评估报告》、算法备案和标识材料的周期以月计，审核驳回正卡着当前版本上架，先下线再补是唯一能马上过审的路。
- **在 AI 对话前加手机号实名认证**：直接回应第一条理由，账号体系里也有短信登录。不选是因为它只解决「应用内容」那一条，另外两条资质材料仍然缺，审核照样不过；而且 AI 联想、润色、回复这些没有对话框的入口同样属于生成合成服务，实名一个对话页不够。
- **按版本（edition）或编译开关裁掉代码**：最强的理由是包里连代码都没有，审核方拆包也找不到。不选是因为鸿蒙版现在不分 edition，另起一个编译变体要改签名、打包和发布流水线；而且维护者要求以后能原样恢复，运行时总闸改一行就回来，裁代码则要再合一次。
- **只在 ArkTS 里放一个常量**：改起来最直接。不选是因为设置页的 React 一侧也要知道，两处常量迟早会对不上；宿主能力本来就是平台差异的唯一出处。

## Consequences

- **收益**：鸿蒙版的设置页、键盘和设置桥都不再有把用户文字发给大模型的路径，审核信息可以如实写「应用内已无 AI 生成模块」。闸只有一个值，Windows、macOS、Linux、Android、iOS 的界面和行为不变。
- **代价**：鸿蒙用户暂时用不了 AI 对话、润色、回复、AI 联想、AI 翻译和 AI 皮肤，已经配好的 AI 设置留在偏好里但不起作用。功能代码仍在包里，只是走不到；如果审核方要求连代码都不能有，就要回到「按编译开关裁掉」那条路。
- **要重访的信号**：资质到位；或者以后新增任何调用大模型的功能——它必须同时接 `showAiGeneration`（页面）和 `AiGenerationGate`（鸿蒙原生），否则会绕过这道闸。

## Verification

- Rust：`crates/client-core/src/host_surface/tests.rs` 的 `only_harmony_withholds_ai_generation`。
- 共享设置页：`apps/desktop/tests/settings/harmony-ai-generation.test.tsx`（能力派生、2in1 侧栏与路由、其余桌面平台保留 AI 页），`core/mobile-tab-bar.test.tsx`（手机首页列表、试用键盘、路由），以及 `voice-handwriting-harmony`、`expression-harmony`、`skin-grid`、`account/account-page`、`community/community-category-tabs`、`statistics-distribution-model`、`typing-statistics` 里的对应用例。
- 鸿蒙原生：`platforms/harmony/tests/ai-generation-gate.test.ts` 覆盖能力解析、功能面板、翻译服务和设置桥的放行规则；`tests/run.sh` 钉住各 `.ets` 请求点和两个 `adopt` 的接线；`hvigorw assembleHap` 编译过 ArkTS。
- 相关笔记：[触屏 AI 对话改为二级页](2026-10-16-mobile-chat-secondary-page.md)描述的是总闸打开时鸿蒙的对话入口。
