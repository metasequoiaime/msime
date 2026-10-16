# Agent Note: 共享标点偏好的叶子类型

Status: implemented

## Problem

`Preferences` 与 `PunctuationPreferences` 重复声明十二项标点及字符宽度字段，引号和括号样式的取值也各写两次。修改设置契约时，公共偏好和组件补丁可能漂移。

## Decision

`settings/punctuation-preferences.ts` 用对象类型持有十二个字段和两种样式类型，并通过 `punctuationControlState` 为共享控件补齐缺省状态。`Preferences` 与该类型组合；组件保留原名接口并继承叶子对象类型，原有类型导出路径和接口扩展能力保持。所有必填性、可选性、默认值与存储格式不变。

`punctuation-options.ts` 持有桌面与手机共用的标点开关文案和固定标点、引号、方头括号的有序选项。React 组件直接将数据映射成 `<option>`，不包一层选项组件，让 HarmonyOS 的 `SelectRow` 能继续读取面板项。原字段名、文案、顺序、可编辑状态和单字段补丁保持不变（之后的[「标点与翻译」页精简](../../implemented/simplification/2026-10-16-expression-page-prune.md)把中文标点和固定标点合成一个选择、让智能标点总开关连同两半一起写，这两处不再是单字段补丁）；解析只生成显示值，不把补齐后的字段写入文档。缺少 `paired_punctuation`、`smart_punctuation`、`smart_punctuation_repeat`、`smart_punctuation_direct_digit`、`smart_punctuation_direct_letter` 时开关默认开启（智能标点这几项见[智能标点默认开启](../../implemented/feature/2026-10-16-smart-punctuation-default-on.md)），其他可选标点开关默认关闭。

`test-preferences-field-parity.py` 原本只识别直接对象声明。门禁收集所有模块的唯一对象类型后，递归展开 `Preferences` 可达的命名交叉部分，再沿原路径比较 Rust 字段；引用缺失、重名而无法解析或循环时明确失败，不能通过忽略抽取字段来放行。其他 UI 组件的不可达接口组合不参与偏好遍历，门禁仍核对 35 组共享类型。

## Historical audit

按 `PunctuationPreferences`、`punctuation-section`、`punctuation-preferences` 和标点类型检索活跃笔记，没有已有类型归属决定。[引号与括号样式](../../implemented/feature/2026-10-13-punctuation-mark-styles.md)持有样式行为，与本篇部分重叠，互链保留。[大写锁定标点](../../implemented/feature/2026-10-09-caps-lock-ascii-punctuation.md)持有 ASCII 路线行为，[脚本插件指令](../../implemented/architecture/2026-10-13-script-plugin-command-rows.md)仅共用设置页入口，均无关。[快捷键类型](../../implemented/simplification/2026-10-15-keybinding-preferences.md)提供叶子类型归属的既有做法，契约独立。

## Alternatives considered

- **直接移动接口，让 `Preferences` 与它交叉。** 最少代码且复用完整接口，但 TypeScript 实测结果不能赋值给 `Record<string, unknown>`，原对象类型具有的隐式索引兼容性会丢失。叶子对象类型避免这项变化，旧组件接口继续保留。
- **组件从总入口 `Pick<Preferences, ...>`。** 保持总入口声明不变，容易核对，但必须继续维护十二个键名，而且组件反向依赖 UI 聚合入口；独立模块让字段只声明一次。
- **把原组件接口改成类型别名。** 少一层继承，但改变公开接口的声明合并能力；兼容接口保留在原模块，不把已有消费者的扩展传播到整份偏好。

- **选项和缺省值继续内联在组件中。** 文件较少且容易追踪一行 JSX，但宿主需要加载 UI 才能复用选项，纯逻辑验证也被组件依赖拖入；独立数据和解析保留原展示契约。

## Consequences

十二个字段及两种样式取值只声明一次。新增叶子偏好模块、选项数据模块和兼容接口层，公共入口保留原导出面，组件保留原模块接口的声明合并能力，整份偏好保留对象类型的 `Record<string, unknown>` 可赋值性；组件消费者的接口扩展仍不会自动进入持久化偏好。控件缺省值与选项可在不加载 React 的情况下复用，代价是多一个选项模块；后续若要统一宿主存储契约，仍须单独核对默认值和枚举。

既有标点控件和设置页面测试抽取前后均为四文件、十项通过；桌面构建与 Harmony 类型检查通过。独立 TypeScript 编译核对原导入路径、最小偏好、组件接口声明合并和 `Record<string, unknown>` 兼容性。新增四项门禁解析回归，先确认原解析器失败，再确认跨文件和传递组合会展开、未知字段仍失败、缺失引用及循环会拒绝。类型声明不靠重复字段的运行时测试验证。控件状态解析另用行为测试覆盖缺键、显式关闭、已存枚举及输入不变性；手机真实选择面板回归验证从有序数据生成的选项仍可读取，并分别写回引号、括号和固定标点的字段。所有默认值与存储行为维持原样。

对象类型、接口扩展与交叉类型语义见 [TypeScript 文档](https://www.typescriptlang.org/docs/handbook/2/objects.html)。
