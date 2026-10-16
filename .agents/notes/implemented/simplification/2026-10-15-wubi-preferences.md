# Agent Note: 五笔控件共用偏好契约和有效状态

Status: implemented

## Problem

`Preferences` 和 `WubiSection` 分别声明混输、剩余编码、四码唯一自动上屏三个可选字段；后者把缺省值规则放在 JSX 中。只消费偏好的模块要依赖整个 UI 入口，其他控件也无法直接复用版本相关的混输缺省与显式选择优先规则。

## Decision

`WubiProfile` 同样归入纯 `wubi-preferences.ts`，输入方案详情保留原路径类型转导出；查码结果组件直接依赖纯模型，公共包入口的类型保持兼容。

由无 React 依赖的 `settings/wubi-preferences.ts` 持有三项类型和 `wubiControlState`。公共 `Preferences` 组合叶子契约，原控件接口扩展叶子类型，保留原名、导入路径和声明合并能力。控件只消费有效状态，仍逐字段写回。

混输缺键时使用调用方传入的版本缺省（未传则关），剩余编码和四码唯一自动上屏缺键时为开。显式布尔值始终优先；读取只投影，不补写文档。五笔码表版本不在这三个开关的契约内。

## Historical audit

检索 `WubiPreferences`、`wubi_mixed_pinyin`、`wubi_code_hint`、`wubi_auto_commit_unique` 的活跃笔记：没有已有的五笔 UI 类型归属笔记。[四码唯一自动上屏](../feature/2026-10-08-wubi-auto-commit-unique-preference.md)与[混输命名](../feature/2026-10-09-wubi-mixed-pinyin-naming.md)部分重叠，分别持有运行时接入、缺省理由以及文案和版本差异，本片只抽取现有规则，互链保留。[中英间距](../feature/2026-10-12-chinese-english-spacing.md)只引用同样的运行时生效时机，[Windows 候选窗](../feature/2026-10-10-windows-candidate-window-mac-parity.md)只持有提示渲染，与本片类型和控件状态归属无关，不吸收。

## Alternatives considered

- **让组件直接从 UI 总入口引用五笔字段。** 可以删除一份字段列表，改动最小；但字段仍分散在庞大入口内，纯状态也继续依赖它。叶子模块让契约和缺省规则归属明确，沿用现有标点、预编辑模型结构。
- **抽取固定默认对象。** 容易与其他偏好默认值统一；但混输缺省由版本决定，固定对象不能表达调用方的覆盖值，且将默认对象合并回文档会抹掉缺键语义。保留只读投影与动态缺省参数。

## Consequences

三项字段只在叶子模块声明，公共偏好和控件共同消费；默认值规则可独立使用。原控件接口、字段可选性、文案、顺序与逐字段补丁保持不变。显式布尔选择不会被版本默认覆盖，也不将缺省补写进文档。

代价是一个运行时叶子模块和每次渲染的小对象分配。五笔码表版本、引擎和原生宿主均沿用现有契约；状态对象不能整体作为保存补丁。

新增两项纯模型测试先因未导出 API 失败，再与既有控件、版本差异和设置保存回归共四文件 384 项通过。共享 UI 构建、Harmony 类型检查、修改文件格式与 lint 通过；独立审查复跑两文件五项测试、lint 和 diff 检查通过。

类型组合与接口扩展参见 [TypeScript 对象类型文档](https://www.typescriptlang.org/docs/handbook/2/objects.html#extending-types)。
