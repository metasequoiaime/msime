# Agent Note: 候选项排列方式选项模型

Status: implemented

## Problem

候选项排列方式的取值类型和两个选项只写在 `CandidateLayoutSection` 组件文件里。宿主只要复用这个设置契约，就必须依赖组件模块，选项顺序和文案也没有独立的模型测试。

## Decision

`candidate-layout-options.ts` 负责公开 `CandidateLayout` 类型和 `candidateLayoutOptions` 常量。组件继续渲染同一常量，包入口同时公开它们，让桌面和其他宿主可以复用排列方式契约。选项顺序、中文文案和默认值保持不变。

## Alternatives considered

- **继续把选项留在组件中**：改动最少，但复用方仍需依赖 UI 组件文件，无法单独验证共享选项契约。
- **把选项放进通用设置选项总表**：可以集中管理，但会把候选窗口专属类型和文案耦合到不相关的设置目录，扩大修改范围。

## Consequences

候选排列方式的模型可以脱离 React 组件导入和测试，页面渲染行为不变。新增了一个包入口导出，维护者需要在修改排列方式时同步更新模型测试；此片不改变偏好字段、宿主能力判断或页面布局。

验证包括模型测试、组件回归测试、前端构建、lint、笔记门禁和仓库 quick 门禁。
