# Agent Note: 86 五笔原表拆成极点表与 rime-wubi 两个文件，rime-wubi 的行按最低一档权重写入

Status: implemented

## Problem

msime-dictionary 的 `sources/wubi/wubi86-jidian.txt` 是两样东西接在一起：前 89272 行是 KyleBing/rime-wubi86-jidian 的码表，权重是同码名次（10、20、30……）；后 50497 行是 msime-dictionary #36 从 rime/rime-wubi `152a0d3f` 补进来、极点表没有的行，权重是词频（0 到 444000000）。这带来四个问题（数据见 `dict-cleanse-plan.md` §1、§3 H6）：

- 这个文件不在 `upstream.lock.json`，也不在构建器的 `UPSTREAM_FILES` 里，没有任何锁保护，许可也分不开（Apache-2.0 与 LGPL-3.0 混在一个文件里）。
- 尾段的词频原样进表：284 个编码的极点首选被尾段的行压到后面（`ko` 下 咪 61900000），一码 25 个、二码 279 个编码的补全前 5 被尾段占满；`版权所有` 带着错码 `bqsy` 和 444000000 的权重。新世纪表的第一层沿用「极点表里的权重」，也把这些词频带了过去。
- 补充表生成器算「同码最低权重」时把扩展区字的行也算进去，这些行几乎都是 0 权重，构建又根本不写它们，结果 20538 个编码、83073 行补充词的权重被压成 0。
- 兼容汉字区（U+F900–U+FAFF）的字与正规字共用编码，有时排在正规字前面（兀 U+FA0C 在 `gqv` 上带着 2370000）。

## Decision

- **拆文件，不改字节。** msime-dictionary 把原文件在第 89272 行之后切开：`wubi86-jidian.txt` 留前段，新文件 `wubi86-rime-wubi.txt` 是后段，两个文件首尾相接与拆分前逐字节相同（SHA-256 `70afd847…`）。`upstream.lock.json` 分别记在新上游 `rime-wubi86-jidian`（`28f19e3affdabb37d11d38f457c7a08a4694b226`）和 `rime-wubi`（`152a0d3f3efe40cae216d1e3b338242446848d07`）下；msime 的 `resources/dictionary-sources.lock.json` 加上同名 reference，`sources.rs` 的 `UPSTREAM_FILES` 从 17 项变 19 项。
- **首段与上游数据区不是逐字节一致的，照实登记。** 按上游各提交的 `wubi86_jidian.dict.yaml` 逐个比对，`28f19e3a`（2025-11-19）的 `...` 之后的数据区与 msime-dictionary 最早的 `cn/Wubi86.txt`（`f453885`）只差末尾一个换行；之后 msime-dictionary `923ca17` 删了 `cokg` 下词那一列为空、第 4 列写着「全角空格」的一行（上游这一行本来就没有那个字符，词是空串），给 5 行无权重的词补了 10，#36 给末行补了 CRLF。首段照这个状态锁进记录，6 处改动写进两边的 NOTICE，不为了「逐字节等于上游」去改它：还原会让构建少 5 个词（没有权重的行被行解析当成无效行），空词那一行也只会被当成无效行跳过，这些变化混进一次本应不改产物的拆分里。
- **尾段可复现。** 尾段等于 rime-wubi `152a0d3f` 的 `wubi86.dict.yaml` 数据行里「词、编码」不在首段的行，按文件顺序取首次出现的、只留三列、去掉唯一一行非整数权重（`χ zzxx 450.5`），已逐行比对一致。
- **按「`wubi86-rime-wubi.txt` 在不在」切换布局**（`crates/dict-builder/src/wubi86_table.rs` 的 `Files::locate`）。在：两个文件都经 `Sources::pinned` 按记录校验。不在：旧布局，合并的 `wubi86-jidian.txt` 经 `Sources::optional` 不经记录读取，拆分之前的提交（包括已发布的 `dict-v*`）照常构建。记录已经列出 `wubi86-jidian.txt` 却没有 rime-wubi 文件时报错，不读出一张少了尾段的表。Wubi 阶段、三个补充表生成器、`wubi86-helpcode` 和 `word-vetoes` 的过期检查都经 `wubi86_table` 读。
- **尾段的行一律按权重 10 写入**（`RIME_WUBI_WEIGHT`，`wubi86_table::rows` 是 `build_wubi` 和三个生成器共用的那一个映射）。10 是极点表有名次的行的最低一档：同码时与极点最后一名同权重，按 rowid 排在它之后；尾段内部保持文件顺序，也就是 rime-wubi 权重从高到低；又高于补充表占用的 0–9（编译期 `const` 断言）。新世纪表第一层沿用的也是映射后的 10。旧布局分不出尾段，按原样读。
- **`rank()` 的同码最低权重和「四码唯一」只算构建会写入的行**（`msime::unsupported_wubi_value` 为假），三个生成器的候选词也按同一个判断筛。
- **`unsupported_wubi_value` 加上 U+F900–U+FAFF**，86、98、新世纪三张表都不写兼容汉字区的字，包括区内 12 个没有分解的字（只见于日文人名等场合）。
- 补充表与辅助码表的表头按实际读到的文件列出每个 86 文件的 SHA-256；旧布局下表头与拆分之前一字不差（`wubi86_table::described`）。

## 跨仓顺序

`Dictionary::open` 遇到 msime 锁文件里没有的上游名会失败，所以 msime 这一侧（两个 reference、`UPSTREAM_FILES`、布局切换）必须先合，msime-dictionary 拆文件的 PR 再在同一个 PR 里把 `builder.lock.json` 升到 msime 的合并提交。反方向安全：新构建器读没拆的 checkout 走旧布局。拆分 PR 合入时，权重映射、最低权重修正和兼容字过滤同时生效；之前用新构建器构建旧布局的数据，只有后两项生效。

## Alternatives considered

- **把首段改回与上游数据区逐字节一致。** 最强的理由是「上游文件逐字节锁定」的原则本身，以后换上游提交只是替换文件。不用：上游有 5 行没有权重，构建器的行解析会把它们当无效行丢掉，等于少了 5 个词；这次拆分要能用「产物里 wubi86 只因权重映射而变」来审，不能夹带别的变化。差异已经逐条登记，以后换上游提交时一并消掉。
- **尾段按 rime-wubi 词频在同码内排名，给 10 的倍数。** 能保住尾段内部的词频次序，而且不依赖 rowid。不用：尾段 1202 个可写入的行里只有 295 行落在极点表也有的编码上，这些编码的极点最低权重都是 10，名次权重会顶到极点行前面或者还得截回 10；尾段的 550 个编码里只有 2 个的文件顺序不是词频从高到低。常数 10 加 rowid 得到同样的次序，规则也简单。
- **旧布局按已知的字节位置切出尾段，也做映射。** 合并文件自 #36 起没变过（SHA-256 `70afd847…`），按摘要识别再按偏移切开是确定的，好处是新构建器配旧数据也能立刻改善排序。不用：要在构建器里写死一份数据文件的摘要和偏移；拆分 PR 本来就和升构建器同时合，旧布局只是兼容路径。
- **在辅助码表里也去掉兼容汉字。** 和 wubi86 表保持一致。暂不做：现在的 `wubi86_helpcode.txt` 里有 7 个兼容汉字正好在「只来自 rime-wubi 的 16 个字」里，去掉要连同各平台许可说明的字数一起改；辅助码表本次不重新生成（两种布局生成的码相同），留给下次重新生成。

## Consequences

- **收益**：86 原表两段都有锁、来源和许可分开登记；尾段的词频不再抢同码首选、不再占一码二码补全前列，`版权所有 bqsy` 降到最低一档；约 2 万个编码的补充词恢复正常权重；兼容字不再和正规字抢位。
- **代价**：尾段内部的次序靠 rowid，不再由权重表达；首段与上游数据区的 6 处差异仍然存在，只是登记了；`版权所有` 的错码还在（只能报给上游或另用否决表处理）。msime-dictionary 的三份补充表快照与辅助码表的表头仍写着拆分前的摘要，它们只供审计，下次重新生成时刷新。

## Verification

- 构建器：在独占的 target 目录里经 `rbuild` 跑 `cargo clippy -p msime-dict-builder --all-targets --locked -- -D warnings`（无告警）和 `cargo test -p msime-dict-builder --locked`（253 个单元测试、14 个集成测试通过）；`cargo test -- --list` 确认 `wubi86_table`、`rank_ignores_rows_the_build_leaves_out`、`rime_wubi_rows_count_at_their_mapped_weight`、`the_split_86_layout_ranks_rime_wubi_rows_below_jidian_rows` 等新测试在二进制里。`python3 scripts/test-language-data-notices.py` 通过。
- 数据：拆分后两个文件首尾相接的 SHA-256 等于 `main` 上的 `wubi86-jidian.txt`（`70afd847…`）；对上游 `28f19e3a` 的数据区重放 NOTICE 写的 6 处改动得到的字节与首段逐字节相同（被删的那一行是 `<TAB>cokg<TAB>10<TAB>全角空格`，词那一列是空串）；按复现规则从 rime-wubi `152a0d3f` 生成的 50497 行与尾段逐字节相同。msime-dictionary 的 `python3 scripts/check_upstream.py` 通过。
- 产物：msime-dictionary 分支 `cleanse-data/wubi` 以 `msime_ref=cleanse/wubi` 试跑发布 workflow（publish 关闭）成功，与同一构建器基线（`main` 数据加 `feat/dict-cleanse-round1` 构建器的试跑）用 `scripts/dict_artifact_diff.py` 比较，只有 `msime-wubi.db`、三份补充表和 manifest 变了：`wubi86` 删 23 行（兼容汉字）、改权重 71197 行；`wubi98` 删 25 行（兼容汉字）、改权重 1 行；`wubixsj` 改权重 492 行；拼音、英文等其他库逐表相同。按引擎的前缀查询次序算，一码前缀补全前 5 里有 rime-wubi 行的从 25/25 降到 0/25，二码从 279/625 降到 9/625（剩下的 9 个是 rime-wubi 在该二码本身或其下有行，排在同码极点行之后，例如 `ko` 的 噗、咪）；极点表有行却由 rime-wubi 行当首选的编码从 286 个降到 2 个（`rtgt` 的 龵、`vhgd` 的 ⺻：这两个编码上的极点行都是构建不写入的扩展区字，rime-wubi 的部首成了唯一的原表行）；`wubi86-supplement.txt` 里权重为 0 的行从 136866 行（28742 个编码）降到 66864 行（9958 个编码），剩下的是同码补充词多于 10 个、从 9 往下排到底的。
