# Agent Note: 整句候选来源徽章在 macOS 的取舍

Status: proposed

## Problem

旧版 Windows v0.11.1 用 `sentence_source_badge` 区分多种整句来源。macOS 尚未提供对应徽章设置；照搬旧标签前，需要先核对当前生产者、wire 来源与正常设置路径。

本提案只讨论词格及本地模型产生的整句联想行，不把云候选、AI 回答、插件或原样输入回退混入模型来源统计。它记录暂缓新增徽章的建议，仍待产品决策；合并这份笔记不表示徽章方案已经定案。

## Proposal

本期暂不为 macOS 新增来源徽章，先保留现有整句功能。原审计“桌面只有一种、全平台最多两种、octagram 没有读取端”的前提已经过时，不能继续用它证明暂缓建议。

以 `3579090da7` 为审查基线，当前有以下三种模型来源值；其中 macOS 常规设置路径提供前两种：

| 来源值 | 生产条件 | 证据 |
| --- | --- | --- |
| `Generated = 8` | 普通词格行、纠错及整句改字 | `crates/engine/src/lattice/merge.rs` 的 `lattice_row_source`、`generated_block_linear` 与 `reranked_block` |
| `Collocation = 17` | octagram 语法模型参与词格打分，标记解码首选行 | 同文件的 `include_collocation_best` 分支；`crates/engine/src/lattice/octagram.rs` 与 `merge_collocation_tests.rs` |
| `NeuralKeyboard = 11` | 键盘神经模型挑出的整句行；正常 UI 只在触屏宿主提供开关 | `reranked_block` 与 `packages/ui/src/settings/sentence-association-section.tsx` |

macOS 默认未选择语法模型时主要是 `Generated`；用户安装并启用语法模型后，`Collocation` 已可达，普通词格备选仍可标为 `Generated`。因此常规桌面路径最多两种模型来源，触屏路径可涉及三种，不能再写成“全平台最多两种”。这里统计的是来源类别，不保证一张候选列表同时出现全部类别。

万象、八股文等具体语法模型共用 `Collocation`，没有分别占用 wire 值。它是同一词格解码路径叠加语法分后的首选归属，也不表示每种模型都是一个独立解码器。桌面神经联想仍通过运行时落定重排调整已有候选及其元数据顺序，不将来源改写成新的神经桌面值。

暂缓建议的理由是 macOS 常规路径目前只有两类可见来源，新增标签的收益与展示成本仍需评估；“至少三种才考虑”仅是本提案建议的产品门槛，不是已经存在的仓库规则。触屏已有三种来源，也不能直接代替 macOS 的评估。

## Existing notes audit

- [octagram 读取端与打分](../../implemented/feature/2026-10-21-octagram-grammar-model.md)已经落地，部分重叠，继续拥有模型、许可、下载和 wire 来源的契约；本提案据此更新来源统计。
- [落定重排顺序缓冲](../../implemented/testing/2026-10-17-runtime-settled-order-scratch.md)部分重叠，继续拥有运行时重排与元数据顺序的实现约束，本提案只讨论展示来源。

## Alternatives considered

- **照搬 Windows 的七个标签。** 最强理由是便于旧版用户识别模型；但当前语法模型共用一个 `Collocation` 值，桌面神经仅重排已有行。七个旧标签不能直接从当前来源值一一还原，需要先定义按生产者还是按所选模型展示。
- **现在展示词格与搭配两种徽章。** 最强理由是这两种来源已经在 macOS 可达，不需等待第三种；但是否值得增加设置行与候选宽度尚待评估，不能仅靠来源计数决定。
- **单独标注桌面神经重排。** 最强理由是解释候选顺序为何变化；但重排不产生新候选，现有来源只随行移动。若要记录“经神经重排”的属性，应另行定义元数据含义。
- **先只在触屏做。** 最强理由是正常设置路径能涉及三类来源；但移动端显示空间和产品诉求需要独立评估，不并入本次 macOS 提案。
- **暂缓新增徽章。** 最强理由是先保留当前展示与模型契约，避免过早固化标签语义；代价是用户暂时不能从候选窗区分词格与搭配行。这是本提案的建议，仍待确认。

## Acceptance criteria

- 产品评审确认是否接受暂缓建议，以及是否需要现在展示已经可达的两类 macOS 来源；本篇保持 `proposed`。
- 后续若新增第三种 macOS 常规路径可达的来源，或增加桌面神经重排标记，重新评估展示方案；已有 `Collocation` 不再列作未来尚未接入的能力。
- 具体模型名称与 wire 来源类别分别评估，不新增仅用于凑齐旧标签的枚举值。
- `pnpm run verify-notes` 与本次文档范围的 quick 门禁通过。

## Risks

- 来源与展示策略可能继续变化，判断应核对实际生产分支及宿主可配置路径。
- 暂缓 macOS 不代表否定触屏宿主的独立需求，也不意味着两类来源一定没有展示价值。
- 本次只更新提案事实；未实现徽章或验证真实候选窗外观。
