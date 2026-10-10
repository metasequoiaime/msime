# Agent Note: 混合简拼扫描直接过滤

Status: implemented

## Problem

`PinyinDatabase::query_single_cut_keyed` 的混合简拼分支先按 `max(limit * 16, 128)` 查询并物化一个 SQLite 行页，再过滤每行的完整键，最后把命中行复制到另一个结果 `Vec`。过滤结果为空时仍保留临时页和结果缓冲；命中少于扫描页时同时存活两份行容器。真实调用来自全拼和双拼级联，扫描上限与 `build_mixed_jianpin_scan_limit` 及 `query_capacity` 绑定。

## Decision

复用现有 SQLite 行访问循环，直接把通过 `matches_mixed_segments` 的行写入结果；首次命中时再按原 `limit.min(128)` 预留，达到 `limit` 后不再追加。保持 exact、prefix、mixed、pure 级联顺序、SQL 上限、字段转换、错误语义、稳定排序和返回容量契约。

## Alternatives considered

- 保留临时页：实现最小，但继续为过滤掉的行分配行字符串和页容器。
- 先查 `COUNT` 再精确预留：需要重复 SQL，且两次查询之间有数据库变化边界。
- 在 SQL 中表达完整键匹配：键按音节和来源规则匹配，难以保持 `QuerySource` 的 `zh/ch/sh` 语义，并可能失去现有索引路径。

## Verification

冻结旧级联的完整查询正文，与当前 `query_single_cut_keyed` 对照真实 SQLite 数据。新增 7 项测试覆盖全 miss、唯一高权重命中、limit 1/2/64/128、扫描页内达到 limit、`usize::MAX`、首步 SQLite 转换错误、双拼 `sh` 初始 token 和纯简拼回退；比较字段、顺序、容量和返回非空/空语义。冷测量在区间外调用 `intact_pinyin_set()` 预热，闭包包含数据库打开、查询和显式析构，只返回结果；双方剩余存储和最低 signed 差值一致。

在固定 arm64 debug 与 160 行真实 SQLite 扫描页上，热分配从 268 降到 13；冷峰值从 9603 降到 911 字节，分配从 292 降到 37，命中结果剩余存储均为 68 字节。全 miss 返回零容量；密集命中和各 `limit` 返回容量与冻结旧级联一致。engine 测试中 `session::tests::typing_at_a_caret_reuses_the_editing_text_length` 断言 expected 207、actual 206，当时被当成既有基线失败排除，其余 1690 项通过、13 项忽略。事后二分确认少掉的这一次分配正来自本改动（父提交 `ca7367b208` 为 207，本提交 `a8b0b24f87` 为 206）：光标处插入会经过混合简拼查询，这是本篇要的分配收益，不是回退，预算由 #6755 同步为 206，见 [msime-engine 测试进 CI](2026-10-10-engine-tests-in-ci.md)。golden 31 项、clippy、fmt 与 notes 校验通过，提交前另跑 quick 门禁。

## Consequences

混合简拼不再先物化完整扫描页再复制过滤结果；通过行直接进入结果，首次命中才按原 `limit.min(128)` 预留，达到 `limit` 后停止读取。全 miss 不分配结果容器，命中少于扫描页时不同时存活两份 `DictRow` 字符串。`visit_rows` 默认回调继续逐行读取，其他查询入口行为不变。

直接过滤仍为每个扫描行执行字段转换和匹配；收益来自避免临时页和复制，并在结果已满时停止后续转换。提前停止意味着不会观察 limit 之后的 SQLite 步进错误，但已返回行与旧级联一致；首步转换错误仍返回空页。没有引入依赖或 unsafe；Rust 请求字节不含 SQLite C 堆或 RSS，分配收益不等同设备延迟改善。
