# Agent Note: 导入 Rime 百分比权重时保留合法词条

Status: implemented

## Problem

共享词典导入器只接受整数权重。Rime 的百分比权重会被当成 `Weight` 错误跳过；整份文件只有这种词条时，会报没有可用行。明月拼音正式词典中，多个常用字的不同读音使用百分比，因此这不是损坏文件才会触发的边界。

核对上游实际默认分支的固定提交：[`rime-luna-pinyin` 的 `luna_pinyin.dict.yaml`](https://github.com/rime/rime-luna-pinyin/blob/56b934b099dfbeab842320f13aa8b461a6ab3e42/luna_pinyin.dict.yaml) 有 `99.93%`、`0.07%`、`0%` 与 `100%`；[`librime` 的 `EntryCollector::CreateEntry`](https://github.com/rime/librime/blob/7bc3fb0005a03aff1c086611e4d52bb7f6444fdd/src/rime/dict/entry_collector.cc) 将百分比乘以 Rime 预置词频。这里只核对格式，不引入上游实现或数据资源。

## Decision

仅在 Rime 格式接受有限、非负的百分比数值，以导入器现有的缺省权重 10000 为基数换算，四舍五入为整数，再沿用引擎的最低权重 1。100% 与不填权重相同；同一个字不同读音的相对比例得以保留。超过 100% 的合法放大比例仍可使用，越过 `i64` 范围的结果拒绝。标准 TSV 与 Windows TSV 仍只接受整数权重；[Gboard v1 文本](../feature/2026-10-20-gboard-dictionary-import.md) 识别后使用独立格式，其第三列是语言元数据，不是词频。

水杉导入器没有 Rime 的预置词频表，因此换算只是兼容近似，不能复现 Rime 不同字词之间的绝对频率。不要把百分比误作绝对次数，也不要声称本修复解决 #8643 的外来词频排序和 Android 导入性能问题。

## Alternatives considered

- 所有百分比都回退到默认权重：无需做数值换算，也能保住词条。但会把 `0%` 的罕见读音与 `100%` 的常见读音放到同一权重，丢掉文件提供的比例信息。
- 同时引入 Rime 的预置词频表：能复现上游权重语义。但需要新的资源来源、分发许可和预置词频版本契约，会让一次解析兼容修复变成资源接入。本次沿用既有默认基数，并明确近似的限制。

## Verification

合成多音字回归在修复前得到 `NoUsableRows`。修复后，词典相关测试通过 96 项，1 项依赖外部文件的既有样本测试跳过；覆盖小数百分比、零、大于 100%、非法或非有限值以及其他格式的拒绝规则。集合入口测试确认 3 个词条和换算后的权重通过个人词库队列送达模拟引擎。

## Consequences

基数使用水杉默认值，无法恢复 Rime 的绝对排序。修改只涉及共享文本解析，不改变文件读写、导入队列或宿主接口。笔记检索中，导入行数上限和失败排序笔记持有独立规则；[细胞词库导入](../../implemented/feature/2026-10-13-scel-import.md) 仅在格式选择和调用入口上部分重叠，不改变其二进制转换和权重策略。
