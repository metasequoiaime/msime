# Agent Note: 发布说明模型配置为空时使用默认模型

Status: implemented

## Problem

Android 0.4.2 的发布工作流把未配置的仓库变量作为空字符串传入 `EVERYAPI_RELEASE_NOTES_MODEL`。`os.environ.get` 的默认参数只处理变量不存在，实际请求因此带空模型，网关返回 400，发布说明退回确定性模板。

## Decision

模型配置先去除首尾空白；不存在、空串和纯空白统一使用文档声明的 `claude-sonnet-5-5`，非空配置继续尊重自定义模型。

## Related notes

[发版 PR 正文预算](../process/2026-10-14-release-pr-body-budget.md)处理 PR 正文长度，与本笔记的模型配置入口没有重叠；现有活跃笔记没有持有空模型配置的处理决定。

## Alternatives considered

- 只在 Android workflow 补默认值：可以修好这次发版路径，但其它平台工作流和直接调用脚本仍可能传空串；在脚本读取配置时统一处理，所有入口获得相同行为。

## Verification

现有发布说明测试用合成响应覆盖缺省、空串、空白及自定义模型，请求不得带空模型。

## Consequences

此修复不验证模型是否可路由；网络或模型服务失败时继续使用确定性说明。公开发布说明不随代码修复自动改写。
