# Agent Note: 抽取手写面板状态和动作逻辑

Status: implemented

## Problem

`HandwritingPanel` 把手写笔画、识别队列、候选提交、模型等待、关闭生命周期和面板 JSX 放在同一个组件里。状态逻辑与视图耦合，后续复用或单独验证请求代次、提交代次和客户端切换时需要扫描整段 JSX。

## Decision

`useHandwritingPanel` 集中管理手写面板的状态、异步代次、指针动作、候选提交、资源包等待和关闭清理；`HandwritingPanel` 组合笔迹编辑、候选、模型提示和窗口拖动视图，笔迹区 JSX 由[手写笔迹编辑组件](./2026-10-19-handwriting-ink-editor.md)持有，结果区 JSX 由[手写识别结果组件](./2026-10-19-handwriting-results.md)持有。hook 复用现有 `useAsyncGeneration`、`useLatestRef`、`useResourcePacks`、`usePanelDrag` 和存储工具，返回视图所需的状态与回调。客户端切换时旧识别和提交结果继续由各自代次隔离，卸载和关闭继续取消队列与指针捕获。

## Alternatives considered

- **只把 JSX 拆成子组件**：能缩短文件，但状态仍留在 `HandwritingPanel`，无法形成可复用的状态契约，因此不采用。
- **把识别和提交再拆成多个 hook**：会把同一套客户端切换、关闭和代次边界分散到多个 hook，增加跨 hook 的生命周期协调；本片先保留一个面板状态 hook。

## Verification

- 现有手写指针、撤销/重做、重新识别、候选提交、模型下载等待、激活模式和关闭行为保持不变。
- 识别与提交继续使用 `useAsyncGeneration`，客户端切换和卸载不会接受旧结果。
- `HandwritingPanel` 只负责组合 hook 返回值与现有 JSX，hook 通过公共入口导出。
- 相关测试、桌面构建、HarmonyOS 类型检查、quick 门禁和笔记校验通过。

## Consequences

抽取期间最容易遗漏 JSX 使用的回调或把 effect 依赖改掉；通过保留现有行为测试、补充 hook 契约检查和完整 quick 验证降低风险。

本片把手写模型提示从面板主体中继续拆成独立组件，组件的下载状态与镜像编辑契约见[手写模型下载提示组件](./2026-10-19-handwriting-model-notice.md)。
