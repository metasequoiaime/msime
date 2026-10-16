# Agent Note: 宿主能力与输入方案契约脱离设置入口

Status: implemented

## Problem

`HostCapabilities`、`HostPlatform`、`EditionInfo`、`InputScheme` 和 `ChineseScheme` 定义在 React 设置入口。主题、候选、引导和设置的纯模型因而反向依赖整个入口。现有方案一致性门禁也直接解析入口中的联合类型，迁移定义时必须同步检查来源。

## Decision

`core/input-scheme.ts` 持有输入方案与中文方案类型；`core/host-capabilities.ts` 持有平台、宿主能力和版本信息，并单向引用输入方案。入口兼容转导出原来的五个名字，内部消费者直接引用模型。字段、可选性、数组可变性、枚举顺序和门控行为不变；Rust 仍拥有能力和方案语义，TypeScript 仅映射宿主已提供的契约。

`test-scheme-traits-parity.py` 改为解析方案叶子模块，继续逐项比较 Rust、引擎和各宿主的方案分类，不删除或放宽断言。

## Historical audit

检索活跃笔记的 `HostCapabilities`、`EditionInfo`、`InputScheme`、`input-scheme-options` 和 `scheme-traits-parity`：

- 工具栏选项、候选布局选项和本机模式偏好笔记持有各自的能力门控；本篇只调整它们依赖的类型位置，保留行为决定。
- [iOS/HarmonyOS 设计](../../implemented/feature/2026-10-07-ios-harmony-all-platform-design.md)持有 `core/host-contracts.ts` 的主题、输入法启用与反馈服务接口，本片的数据能力模型不改变这些服务。
- [面板客户端契约](../../implemented/architecture/2026-10-15-panel-client-contracts.md)持有共享面板调用接口，与本篇同属依赖方向收敛，但契约不同，保留。
- [繁体转换变体](../../implemented/feature/2026-10-15-traditional-chinese-variants.md)持有能力字段的产品含义；方案功能、版本裁剪和原生宿主笔记同理，迁移保留这些字段及注释。
- 越南语与辅助码偏好笔记分别拥有偏好模型，本片不改变其字段或默认值；没有已有笔记拥有这五个公共类型的位置决定。

## Alternatives considered

- **保留入口定义，依靠类型擦除避免运行时循环。** 没有运行时成本且改动最少，但纯模型仍反向依赖设置入口，无法建立清楚的类型归属，因此改为叶子模块。
- **扩充已有的 `core/host-contracts.ts`。** 能少建一个模块，但该文件组织应用主题、系统栏、启用状态与反馈服务；能力快照有独立的 Rust 对应模型和大量纯数据消费者，独立归属便于维护这份映射。
- **全部放入一个宿主能力文件。** 导入文件更少，但输入方案还被偏好和插件独立使用；拆成两个小模块能让只需要方案的消费者不依赖整套宿主契约。
- **只迁移纯函数消费者，组件继续依赖入口。** 改动行数更少，但同一契约会保留两套内部引用路径；本片一起迁移共享 UI 的内部直接引用，外部包入口继续兼容。

## Verification

类型测试先因新模块不存在而失败，迁移后实际 `tsc --noEmit` 通过。实际 TypeScript 检查证明五个公开导出与叶子类型一致。源码对照确认迁移前后的定义只有注释、排版与归属变化。方案分类门禁继续通过；以 `target/` 中的临时副本去掉 `ChineseScheme` 的 `zhuyin` 后，门禁准确报告缺项并失败，验证完删除副本。相关方案、版本、能力门控、工具栏、主题和候选共 14 个文件 126 项测试通过；生产构建、lint、格式与笔记检查通过。25 个内部消费者除类型导入外的内容完全一致。`bash scripts/verify-local.sh --quick` 通过；原生平台阶段按范围或本机工具条件跳过，不计为设备验收。

## Consequences

25 处内部消费者直接引用公共模型，减少对 React 设置入口的反向依赖。代价是入口多两处兼容导出，后续增添方案或能力需维护对应的纯模型；方案一致性脚本继续守住跨语言语义。类型引用可能隐藏在混合导入和内联类型查询中，因此迁移后全量检索并实际运行类型检查。宿主产品行为不变，没有原生窗口或设备验收。
