# Agent Note: Windows AI 候选测试显式声明初始开关

Status: implemented

## Problem

`platforms/windows/tests/input/candidate_translation_commit.cpp` 要验证 AI 候选从开启到关闭后候选行被清除，再重新开启并恢复候选。测试夹具原来没有写入 `ai_assistant.enabled`，共享偏好在 Linux 上会把它解析为关闭，导致测试没有覆盖「开启后关闭」的转换，并在本机 quick 门禁中出现跨平台假失败。

## Decision

夹具的初始 `preferences` 显式设置 `ai_assistant.enabled` 为 `true`。这样测试在所有宿主上都从相同的有效状态开始，随后已有的偏好更新仍验证关闭、重新开启以及候选翻译恢复流程。

## Alternatives considered

- **把 Linux 默认值加入 `HOST_DIFFERENCES`**：可以隐藏不同平台的默认值，但会让测试放弃验证同一条偏好转换，并继续依赖未声明的初始状态，因此不采用。
- **只在失败断言处跳过 Linux**：能让门禁变绿，但会丢掉 Linux 对候选清除行为的覆盖，不能接受。

## Consequences

测试配置多写一个明确的偏好字段，夹具不再依赖宿主默认值；Windows、Linux 及其他平台都执行同一条 AI 候选开关转换。产品代码和运行时默认值不变。

验证包括 Windows 输入测试目标、相关本地 quick 门禁、格式检查和笔记校验。
