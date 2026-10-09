# Agent Note: 日文罗马字扫描直接写入请求与检查完整性

Status: implemented

## Problem

日文 `build_request_into` 已保留请求字符串，但先调用拥有型 `convert_romaji`，构造平假名与待定尾部后再复制。预热后的完整 `nihongo` 与待定 `NiHoNg` 请求构造分别分配 2 与 3 次。会话完整性检查只读取 `complete`，却也构造两个字符串；完整 `nihongo` 检查分配 2 次。

## Decision

把现有罗马字状态转移抽成一个私有扫描函数：逐个发出静态表中的假名，返回规范化输入中尚未消费的尾部借用。拥有型 `convert_romaji` 继续收集为原结构；请求构造直接把假名和尾部写入已有 `normalized_segmentation`；会话完整性检查只记录是否发出假名以及尾部是否为空。保留现有大小写规范化和匹配顺序，不引入第二套解析规则或会话缓存。

## 既有笔记审计

[借用小写输入](../../implemented/testing/2026-10-08-japanese-romaji-borrow-lowercase.md) 部分重叠：保留 `Cow` 规范化及公开拥有型接口的分配预算，本篇补充结果物化成本，两篇互链。[罗马字拼法与假名位置](../../implemented/bug-fix/2026-10-08-japanese-romaji-spellings-and-kana-slot.md) 约束 `nn`、促音和补充拼法，继续有效，本篇不改变它们。搜索其余 `JapaneseRomaji` 命中没有另一套转换写入实现；临时日文候选缓冲笔记负责最终候选，与本决定无关。

## Alternatives considered

- 在 scheme 保存 `RomajiConversion` 缓冲：能复用中间字符串，但 `build_request_into` 接口只有共享借用，需要额外内部可变状态；直接写已有请求不需要这份缓存。
- 从 `normalized_segmentation` 推断是否完整：能避免再扫描，但待定尾部与输入完整性规则会散落到会话层；扫描核心共用原规则更稳妥。
- 分别重写直接输出和完整性解析器：可以单独优化各路径，但 `nn`、Hepburn 促音及最长表匹配容易漂移，因此只保留一个状态转移核心。

## Verification

真实红测试中，完整与待定日文请求热构造分别分配 2 与 3 次，会话完整性检查分配 2 次。直接写入与扫描判断后均为 0 次；请求完整字段与输出指针保持一致，覆盖完整、待定、空输入及大小写请求。

直接输出及完整性与拥有型转换对全部罗马字表及各拼法的前缀、特殊 `n`、促音、大小写、Unicode 待定尾部逐项对照；缩短、清空、重新增长的编辑复用不更换输出存储。公开转换沿用原来的结果字符串增长路径和冷分配预算。

引擎 1439 项测试、golden 31 项测试、Clippy、Rustfmt、差异及笔记检查通过；独立设计与代码审查没有遗留问题。完整 quick 门禁退出为 0 并报告 `quick check passed`；Android 目标与宿主 Java 检查、Linux 桌面共享层及原生宿主 73 项 ctest、macOS 原生宿主与共享 Apple bridge 构建通过。

## Consequences

日文逐键构造请求不再物化临时转换结果，只判断完整性也不再构造两个结果字符串。三个入口共用一个扫描核心，额外状态仅为闭包捕获的目标引用或布尔标志，没有增加会话缓存与内部可变性。

尾部借用不超过本次规范化输入的生命周期；已消费位置始终在 UTF-8 边界。空输入继续不是完整读音。直接调用含 ASCII 大写的帮助函数仍为规范化分配一次，请求路径使用已经小写的 `raw_input`，预热后复用实际输出容量，不按最坏扩张上界预留。候选 provider 复用独立转换存储，共用本篇扫描核心，见[provider 转换缓冲](2026-10-09-japanese-provider-conversion-buffer.md)。
