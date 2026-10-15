# Agent Note: 反馈契约与诊断逻辑独立模块

Status: implemented

## Problem

反馈页组件同时承载表单展示、服务类型常量、正文上限和诊断字段组装。共享设置宿主需要复用这些稳定契约时，必须依赖包含完整 UI 和提交状态的文件，导致纯数据与纯逻辑不能单独验证或复用。

## Decision

将 `feedbackMaxText`、`feedbackTypes`、`FeedbackDiagnosticsSource` 和 `feedbackDiagnostics` 放入 `feedback-contracts.ts`。`feedback-report-fields.tsx` 继续从旧路径导出这些符号，`packages/ui/src/index.tsx` 也从新模块导出，保持已有导入路径和包入口兼容。诊断字段顺序、键名、缺省省略规则和现有 HarmonyOS 版本格式保持不变。

## Alternatives considered

- 继续把契约留在表单组件中：改动最小，但复用者仍需依赖 UI 模块，纯逻辑测试也会被组件依赖拖入。
- 直接删除旧路径导出：模块边界更干净，但会破坏现有宿主和外部共享层的导入，增加无关迁移风险。

## Consequences

契约和诊断逻辑可以在无 UI 渲染的测试中独立验证；旧路径和包入口保持兼容。反馈表单仍由原文件负责，提交行为没有改变。以后修改服务字段时需要同步维护新模块的契约测试。
