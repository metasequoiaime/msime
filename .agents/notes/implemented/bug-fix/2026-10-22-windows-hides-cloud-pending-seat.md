# Agent Note: Windows 候选窗隐藏云占位席位，数字只数看得见的行

Status: implemented

## Problem

打任何常见词，候选窗第 2 格都是一朵不能选的云：

```
bushang                     1 / 23
[1 补上]  2 ☁  3 不上  4 不伤  5 部尚  6 不上来
```

它是引擎的 `CandidateSource::CloudPending`（文字就是「☁」），[云候选固定席位](../feature/2026-10-17-cloud-candidate-seat-placeholder.md)为固定云候选的位置而预留。按它什么都不会发生，而它后面每一行的编号都比看到的大一位——按 3 上屏的是第 4 格的词。云回包到达却没有新词时席位**保留到组字结束**（`crates/engine/src/ime/online_batch.rs`），而常见词的云端结果本地基本都有，所以这朵云几乎永不消失：第 2 个数字键长期是废的。

那篇笔记写明了宿主该怎么办——「Android、iOS 和 HarmonyOS 隐藏该内部占位行」「三个移动宿主都把触控、硬件数字键、候选维护操作和展开面板的显示序号重新映射回 Engine 原始索引」。移动端照做了（`CandidatePresentationPolicy.isVisible` / `visibleIndices`），**桌面端一侧从未实现**：Windows 把这行内部占位原样画进候选窗，并按它在引擎列表里的位置分配数字。

## Decision

候选窗**不收**这一行：`candidate_presentation` 的两个构造循环遇到不可见来源直接跳过，于是布局、编号、测宽、分页全都自然连续，界面上不再出现那朵云。每行仍携带自己的引擎 `index`，点击、置顶、删除这些按候选身份走的操作不受影响。

数字键**只数看得见的行**：`ServerSession` 原本按 `digit - '1'` 取 `candidates[slot]`，改为跳过不可见来源后再数到第 `slot` 个。显示与选择都在 Server 进程内按同一条规则，**never** 各算各的——这正是[上屏的词以 Server 的回复为准](2026-10-22-windows-host-candidate-identity-by-text.md)那次缺陷的成因。

可见性判定落在 `common/CandidateSources.h` 的 `candidate_source_is_visible`，与移动端 `CandidatePresentationPolicy.isVisible` 同义：只排除云占位席位；AI 占位行（12）照旧显示，仍画成次要色。

## Alternatives considered

- **让引擎不要产出这个席位** — 界面和选择都不必特殊处理，最省事。否决：席位的作用是让云回包到达时不挤动用户正在看的候选（`online_batch.rs` 与那篇 feature 笔记），去掉它就把误选问题换回来了，而且会同时改变移动端和 Web 的行为。
- **改共享 runtime 的数字映射，让它跳过席位** — 一处改动覆盖所有桌面端。否决：移动端不走 runtime 的数字槽位（它们在宿主侧用 `visibleIndices` 自己映射后调 `select`），改 runtime 会让它们双重映射；而桌面端各自的候选渲染仍画着席位，显示与选择照样对不上。隐藏与重映射必须在同一个宿主里成对出现。
- **只隐藏显示，不改数字映射** — 改动最小。否决：那是最坏的一种——用户看到 5 行却按不中其中 4 行，比现在还难解释。
- **只在绘制时跳过席位** — 不动构造。否决：编号、测宽、分页、命中测试散布在多处，漏掉任何一处就又是一次显示与选择分叉；在构造处不收它，下游全部自动正确。

## Consequences

- **收益**：第 2 个数字键恢复可用，所有编号与看到的行一一对应；界面上不再有一个永远选不中的格子。
- **代价**：云请求进行中时，这一页看得见的候选比引擎的 `page_size` 少一个（席位仍占引擎的位置，只是不显示）。这是移动端早就接受的同一笔代价。
- **未覆盖**：macOS、Linux 和 Web 仍把这行画出来——它们同样缺少隐藏与重映射。本篇只改 Windows，那三个宿主各有自己的候选渲染与选择路径，应各自跟进。
- **重访信号**：共享内核若改为不再产出 `CloudPending`，或移动端的 `CandidatePresentationPolicy` 调整了可见性规则，这里要跟着对齐。

## Verification

Windows 11、Windows App SDK 1.7.250606001，装机后在记事本逐格对照候选窗截图：候选窗不再出现「☁」，`bushang` 的五格依次为 补上 / 不伤 / 不上 / 部尚 / 布商；按 2、3、4、5 分别上屏 补上、不伤、部尚、布商（各次列表因选词学习而变，均与当次截图逐格一致），空格上屏高亮首选。修复前按 2 什么都不发生，按 3 上屏的是第 4 格。

本机 VS 2022 下 `experiments/tsf-edit-control` 与几个候选测试目标有 17 个既有编译错误（min/max 宏一类），暂存本次改动重编确认数量一致，与本修复无关。
