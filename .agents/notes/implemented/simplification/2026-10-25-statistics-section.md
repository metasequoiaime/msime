# Agent Note: 抽取统计页区块标题外壳

Status: implemented

## Problem

统计页的趋势、时段、候选命中、按键热力图、按日明细和分布图分别重复维护 `section`、二级标题以及 `aria-labelledby` 的关联。标题 id 或区块 class 需要调整时，多处 JSX 容易只改一处，统计内容组件也因此同时承担相同的无障碍外壳。

## Decision

共享 UI 提供 `StatisticsSection` 和 `StatisticsSectionProps`。组件接收标题 id、标题内容、子内容和可选区块 class，统一渲染 `section`、`aria-labelledby` 与 `h2`；默认 class 保持 `section m-0`，分布图可显式保留 `@container`。图表、数据计算、条件显示和没有标题的统计卡片继续由原模块持有。

## Alternatives considered

- **继续各处内联。** 不增加公共 API，但标题关联和默认 class 继续复制，未来的统计区块容易出现无障碍标记漂移，因此不采用。
- **把整个统计图表页合成一个组件。** 可以一次迁移更多 JSX，但会把数据模型、标签切换和移动端条件混进展示外壳，扩大复用边界，因此不采用。
- **让组件自行生成 id。** 调用方可以少传一个属性，但表格、子内容和既有测试已经引用稳定 id，生成新 id 会改变跨节点关联，故保留调用方提供的 id。

## Consequences

统计区块共用标题外壳和无障碍关联，分布图仍可使用自己的容器 class；数据和交互状态没有移出原模块。代价是统计页面新增一个公共导出，调用方需要显式提供稳定的标题 id；没有标题或使用 `aria-label` 的概览、状态卡继续保持原有结构。

## Verification

组件契约测试覆盖默认与自定义区块 class、标题等级、标题 id、`aria-labelledby` 和子内容保留；统计页相关回归、类型检查、格式、lint、笔记门禁与本地 quick 门禁通过后再提交。
