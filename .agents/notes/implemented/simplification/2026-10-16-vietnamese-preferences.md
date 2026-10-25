# Agent Note: 越南语偏好模型与有效值投影共用

Status: implemented

## Problem

`VietnamesePreferences` 和缺省值规则原来都在 `packages/ui/src/index.tsx` 与 `InputSchemeDetailsSection` 里。输入方案选项、总偏好契约和控件因此依赖庞大的 React 入口；其他宿主若要读取越南语输入法或复用缺省投影，还要复制 Telex 与新式声调的规则。

## Decision

由无 React 依赖的 `settings/vietnamese-preferences.ts` 持有 `VietnamesePreferences` 类型和 `resolveVietnamesePreferences`。缺省字段只在读取时投影为 `input_method: "telex"` 与 `tone_style: "modern"`，不把缺省值写回文档。`index.tsx` 继续重导出原来的公开类型，组件接口和偏好字段保持兼容；控件消费公共投影，选项模型直接引用叶子类型。

## Historical audit

检索 `VietnamesePreferences`、`input_method`、`tone_style` 和越南语设置相关的活跃笔记：没有已有的 UI 偏好模型归属笔记。命中的是越南语引擎组字、宿主提交、大小写处理和平台能力笔记，分别约束运行时行为，与本片的 TypeScript 设置契约无关，保留不吸收。

## Alternatives considered

- **继续让设置模块从 `index.tsx` 引用类型并在 JSX 中写缺省值。** 改动最小，但纯模型继续依赖 React 入口，其他宿主会复制两项缺省规则；叶子模块让契约和投影规则只有一个归属。
- **把固定默认对象合并回偏好文档。** 可以让控件少写两次默认值，但会把用户从未选择过的字段写入落盘配置，破坏缺键表示默认值的语义；保留只读投影，不改变保存格式。
- **只抽取类型，不抽取投影函数。** 能减少入口中的声明，却让 Telex 与声调缺省仍藏在组件里，下一处调用者还会重复规则；将已有读取逻辑原样集中到无 UI 模块，变更范围更小且可独立测试。

## Consequences

[可选罗马字布局](../feature/2026-10-23-korean-romaja-layout.md)同样使用独立的叶子偏好模型，不把 React 或平台实现带进共享偏好。

越南语偏好类型、有效值投影和总偏好组合现在可被设置控件与其他宿主共同使用；旧的 `@msime/ui` 类型导入路径不变，存储字段、缺省行为和逐字段写回不变。代价是读取时多一次纯对象投影，且新增模块需要在宿主复用时保持与 `client-core` 的枚举同步。

新增纯模型测试覆盖无偏好、单字段覆盖和双字段覆盖；输入方案选项与详情控件回归测试继续通过。类型检查、共享 UI 测试、lint、格式检查、`verify-notes` 和 quick 门禁作为合并前验证。
