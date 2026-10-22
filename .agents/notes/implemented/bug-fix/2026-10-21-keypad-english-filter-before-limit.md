# Agent Note: 分组键盘英文先匹配完整码再限额

Status: implemented

## Problem

九键和十四键只在 64 个字母前缀的预算内展开输入，各前缀先取 5 个词，再检查完整组码。高权重但后续编码不同的词会占满查询页，导致词库内存在的目标词消失，见 #9104。

## Decision

保留索引前缀范围与原有查询预算，在 SQL 中按完整组码对应的字母集合过滤查询键，再取每前缀的 5 行。查询排序与 `rank_english` 的正权重精确展示长度、权重及展示字节长度一致。匹配针对查询键，展示可含标点；普通二十六键查询不改变。会话复用模式字符串，空结果不预留候选页。

## Historical audit

[前缀展开](../../implemented/testing/2026-10-18-nine-key-english-prefix-generation.md)、[数字借用](../../implemented/testing/2026-10-16-nine-key-english-digits-borrow.md)、[空页缓冲](../../implemented/testing/2026-10-10-nine-key-english-lazy-buffer.md)部分重叠，保留其预算及分配契约并互链。[英文空页](../../implemented/testing/2026-10-10-english-empty-prefix-storage.md)约束普通查询容量，继续保留。[无中文答案时英文门控](../../implemented/bug-fix/2026-10-08-nine-key-path-ranking.md)约束调用时机，不改变。本次没有完全吸收或过时提案。

## Alternatives considered

提高每前缀扫描上限最容易实施，但任何固定扫描限额仍会被不匹配后续编码的高权重词占满，不能保证目标词被检索。扫描整个前缀范围并在 Rust 过滤可复用现有谓词，但把无关词条带入按键路径，增加字符串分配和跨 SQLite 边界的开销。

## Verification

拥挤前缀回归在旧实现的真实 `NineKeySession` 上失败；修复后九键、十四键与混输、专用英文均能检索并选择目标词。另覆盖限额前的最终排序、带标点展示、大小写查询键、无效词和空展示不占限额；既有空页容量及热刷新分配测试保持通过。引擎库 2387 项通过、50 项忽略（新增 1 项发布资源验收），golden 31 项通过，严格 all-targets clippy 通过。

发布资源验收通过 `MSIME_TEST_ENGLISH_DICTIONARY` 显式指定已校验词库，不在普通单测中联网。使用 `dict-v2.0.24` 的 `msime-english.db`（SHA-256 `d216f66cab7edb9072ba7a07f3fe35623e76870438b76d360a89cc2a4795d3cd`）验证四词在两种分组键盘、两种英文模式下均可出现并选中。Android 现有 `NineKeyEnglishDeviceSmoke` 同时覆盖报告中的四个完整数字码和实际编辑器提交。

## Consequences

不匹配后续组码的高权重词不再占候选限额；每前缀 5 行、总计 64 个前缀及最终 5 条候选预算不变。普通二十六键前缀查询与分组键盘查询共用行解码，继续保留空页延迟预留、热查询复用及查询失败时丢弃整页的契约。模式字符串由会话复用；完整码过滤仍需扫描索引前缀范围，不能从返回行数预算推导扫描成本。全局展示去重仍沿用原有每前缀预算，不承诺补满五个不同展示。模拟器与用户 Android 16 真机验收需区分。

[九键英文前缀页直接追加](../testing/2026-10-23-nine-key-english-page-append.md) 消除分组查询的临时候选页；本篇原有的查询、复用及容量契约继续保留。
