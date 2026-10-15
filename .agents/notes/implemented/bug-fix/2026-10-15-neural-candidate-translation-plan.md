# Agent Note: 键盘模型整句进入用户服务翻译计划

Status: implemented

## Problem

引擎的 `CandidateSource::NeuralKeyboard` 固定为 11，表示键盘模型选出的普通整句候选。共享 `msime_client_custom_translation_plan` 把 10 以上的来源全部排除，导致该整句虽经宿主查询保留来源，仍在规划腾讯、小牛或自定义翻译时消失。Windows 与 macOS 原生用户服务路径都调用此入口。

## Decision

翻译计划显式接收已知可翻译来源 0–5、8、9、11。表情、颜文字、AI 占位、日文混输、插件和未知来源继续被拒绝。方向检测、文本边界和去重沿用既有规则。

## Alternatives considered

- **由各宿主把来源 11 改为来源 0**：可以绕过旧过滤；但会再次丢失真实来源，重复已经修过的宿主来源漂移问题。
- **只排除 `source_gets_no_gloss` 的四类来源**：与通用释义策略更集中；但该谓词不排除 AI 占位和未知编号，不能直接替代本入口原有的拒绝边界。

## Verification

从 C ABI 传入真实枚举来源的合成整句，旧实现返回空计划，新增回归失败，特殊来源过滤和既有计划测试两项控制通过。修复后，七种目标语言均生成翻译计划；英文反向翻译保持一致。特殊来源、AI 占位和未知编号仍输出空计划，已有页大小、文本过滤和去重测试通过。

`cargo test --locked -p msime-host-api --lib translation_` 的 22 项相邻回归通过，包含来源元数据、日文混输、目标语言切换、文本校验与翻译计划。`cargo clippy --locked -p msime-host-api --all-targets -- -D warnings` 通过。

## Consequences

恢复的是用户已配置翻译服务应处理的普通候选，仍经过原来的服务选择和文本限制。测试使用合成文本，不请求网络。

## Existing notes audit

检索活跃笔记中的 `custom_translation_plan`、翻译计划和 `NeuralKeyboard`。

- [Windows 来源过滤](../../implemented/bug-fix/2026-10-14-windows-translation-source-filter.md)部分重叠，负责宿主保留来源；本篇修共享计划拒绝已知来源，保留并互链。
- [日文混输](../../implemented/feature/2026-10-12-mixed-japanese-candidates.md)部分重叠，其中记载旧计划排除 10 以上来源；同步为当前只放行普通已知来源 11，日文来源 13 的边界不变。
- 九键整句、模型证据和行所有权笔记负责生成或排序，不管理翻译计划；候选页大小笔记负责数量上限；Windows 释义分栏笔记区分账号与用户服务，均保留。
