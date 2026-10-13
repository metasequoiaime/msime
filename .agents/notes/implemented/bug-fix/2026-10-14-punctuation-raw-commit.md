# Agent Note: 拼音标点保留原始输入

Status: implemented

## Problem

#7651：macOS 中文状态下输入未完成的拼音后按标点，宿主在引擎未处理标点时用普通完成命令结束组字，结果提交首候选，再把标点交给应用。

## Decision

全拼和双拼的 ASCII 标点回退路径改用 `MSIME_COMMIT_RAW`，先上屏用户实际输入的原始字母，再把标点交还应用。其他方案仍沿用原来的 `MSIME_FINISH_COMPOSITION`，中文标点和引擎已处理的标点路径不变。

## Alternatives considered

- **在引擎的通用标点接口改变候选选择策略**：会影响所有宿主和移动端的标点契约，而问题只发生在 macOS 把标点交回应用前的回退路径。
- **直接由 macOS 拼接 `editing_text`**：会绕过引擎的原文提交、学习和脚本插件契约；`COMMIT_RAW` 已是各宿主共享的原样上屏接口。

## Consequences

输入 `Observable` 后按 `<` 时，macOS 先提交 `Observable`，应用再接收 `<`；中文候选不会被自动选中。韩文、越南文、藏文和中文标点开关等其他路径保持现有行为。

## Verification

新增 macOS `ShortcutTest` 回归用例，合成候选「白色」并断言标点使用 `MSIME_COMMIT_RAW`、提交 `Observable`。已运行 `git diff --check`；本机未配置 Sparkle，无法运行完整 CMake macOS 测试套件。
