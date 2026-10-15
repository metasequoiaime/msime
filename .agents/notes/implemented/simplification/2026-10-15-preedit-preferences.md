# Agent Note: 预编辑偏好与样式共用叶子类型

Status: implemented

## Problem

`Preferences`、预编辑设置组件和样式选择器重复声明预编辑样式；公共偏好与组件又重复声明三个字段。行内预编辑允许原始按键、拼音分词和隐藏，候选窗预编辑只允许后两种，分别手写取值会让两层类型漂移。

## Decision

`settings/preedit-preferences.ts` 持有三个可选偏好字段及样式类型。`CandidatePreeditStyle` 持有拼音分词与隐藏两值，`PreeditStyle` 在它之上增加 `raw`，`TsfPreeditStyle` 是三值样式的别名。总偏好组合这份对象类型；组件保留原名接口并继承它。选择器从叶子模块导入及重导出原样式名，原公共导出路径继续可用。默认值、控件顺序、可见条件和存储格式不变。

同一叶子模块的 `preeditControlState` 解析候选样式、候选预编辑可见性、双拼显示样式和行内样式。设置组件、内置候选预览与外部皮肤预览直接消费它：缺键时候选显示拼音分词，双拼和行内显示原始按键；显式 `false` 将双拼显示为拼音分词，`empty` 保持隐藏。函数只产生显示状态，不补写文档。`preedit-style-options.ts` 持有三种控件的有序选项与模式类型；选择器继续重导出原模式类型，并直接映射为 `<option>`，供 Harmony 手机选择面板读取。

## Historical audit

按 `PreeditSettingsPreferences`、`PreeditStyle`、`preedit-style` 和预编辑类型检索活跃笔记，没有已有 React 类型归属决定。[Windows 组字细节](../../implemented/feature/2026-10-10-windows-input-compose-mac-parity.md)持有双拼预编辑的宿主能力与行为，独立保留；[正常候选设置入口](../../implemented/bug-fix/2026-10-13-macos-primary-settings-tabs.md)持有候选设置标签及挂载，独立保留。脚本插件、五笔反查、大小写入口、粤语上屏、候选展开和选择分段笔记只涉及输入或预编辑行为，与类型归属无关。[标点叶子类型](../../implemented/simplification/2026-10-15-punctuation-preferences.md)提供组件接口、对象组合与字段门禁的既有兼容做法，契约独立。

## Alternatives considered

- **组件从总入口挑选三个字段。** 复用现有公开契约，改动文件更少，但组件继续反向依赖 UI 聚合入口，选择器也要依赖入口或组件的类型。叶子模块让三者直接引用契约。
- **只给两处字面量换成已有 `PreeditStyle`。** 代码最少，但契约仍挂在 TSX 选择器上，三个字段的重复声明也还在；完整抽取使字段和取值各自只声明一次。
- **直接用接口组合总偏好。** 可以省去组件兼容接口，但会失去 `Record<string, unknown>` 的对象类型兼容性；沿用已验证的叶子对象加原位置接口。

- **只迁移类型，显示判定继续留在组件。** 局部阅读直接，但设置页与两种候选预览仍各自解释可选偏好；把有效显示状态集中后，三个消费者遵循同一缺省规则。

## Consequences

三个字段及样式取值各自只有一份声明，选择器、设置组件和总偏好直接依赖叶子契约。叶子模块同时持有类型与纯显示状态解析，原位置保留兼容接口；原类型导出路径、接口声明合并能力、字段可选性及整份偏好的 `Record<string, unknown>` 兼容性保持。格式器对总偏好的三个交叉部分自动换行，偏好对象内仅随之调整缩进。

既有预编辑控件、行内默认值与保存、设置页面测试抽取前后均为三个文件、六项通过；独立 TypeScript 编译核对旧类型导出、空偏好、样式范围、组件声明合并与 `Record` 兼容性。桌面构建与 Harmony 类型检查通过。字段一致性门禁仍核对 35 组共享类型，既有四项组合解析回归通过。类型抽取本身不增加复述类型字段的运行时测试。显示状态抽取另用行为回归覆盖缺键、显式关闭、已存样式与输入不变性；真实选择器核对选项顺序、文案和单字段补丁，两种候选预览核对默认显示及已存隐藏/显示样式。状态结果增加一个短生命周期对象，换取三处显示规则的统一；选项数据独立于 React，原公开类型路径和控件结构保持。

本片 CI 的 `Workflow validation` 连续两次在准备 action 时失败：`setup-go` 发布提交 `b7ad1dad31e06c5925ef5d2fc7ad053ef454303e` 的 SHA 归档返回 404，官方 API 与 Git 标签仍指向它。同一仓库发布后的固定提交归档可下载；`quality.yml` 改为固定在 `fba3fb4ead8c8e09a06f1d32ae5436e5818eff15`。该提交比发布版前进两次，更新上游依赖并修复其审计问题，`action.yml` 与 `src/` 没有变化，Go 1.26、禁用缓存及 actionlint 命令保持。来源与差异见 [上游对比](https://github.com/actions/setup-go/compare/b7ad1dad31e06c5925ef5d2fc7ad053ef454303e...fba3fb4ead8c8e09a06f1d32ae5436e5818eff15)。此项是推进本片时发现的门禁修复；后续依赖更新仍须核对实际 SHA 归档可下载。
