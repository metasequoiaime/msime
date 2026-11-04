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
- **尾段的行换算之后接在极点表之后**（`wubi86_table::tail`，`wubi86_table::rows` 是 `build_wubi` 和三个生成器共用的那一个入口），四条规则：
  - 有词频的行记为 10（`RIME_WUBI_WEIGHT`），即极点表名次的最低一档：同码时与极点最后一名同权重，按 rowid 排在它之后；又高于补充表占用的 0–9（编译期 `const` 断言）。极点表构建会写入的行并不都不低于 10，蔻 `apfc`、个位数 `wwov` 是 5；同码极点行有名次的最低权重低于 10 时取那个权重，免得尾段排到它前面（这两个编码上眼下没有尾段行）。新世纪表第一层沿用的也是换算后的权重。
  - 上游权重为 0 的保持 0。会写入的 12 行全是部首、部件（龴 `cny`、龺 `fjj`、龸 `ipb`、龹 `udi`、⺶ `udr`、龷 `aghg`、龶 `gggg`、⺌ `ihyt`、⺄ `nnll`、龵 `rtgt`、⺻ `vhgd`、龻 `xyxy`），上游用 0 表示它们排最后。起初一律记 10 时，⺻ 成了 `vhg` 补全的首位，龷 挤进 `agh` 前 5。0 权重的行不定补充表的同码上限（`wubi86_supplement::build` 去掉它们再交给 `rank`，否则 `gggg` 上的 56 个补充词、`rtgt` 上的 16 个都会被压成 0），`build_wubi` 把它们写在补充表和手工词条之后，同码同为 0 时也排在补充词后面。
  - 极点表已经收了的多字词不收，不论编码。词组编码由规则推出，一个词只有一个；极点表收了，rime-wubi 给的另一个编码就是错码。全表实测是 2 行：`版权所有 bqsy`（拼音首字母，原来是 `bqsy` 的首选，压在正确的 除皱术 前面；极点表有 `tsrd`）和 `T恤 ttnt`（极点表是 `tntl`）。单字不在此列：尾段 669 行单字的编码与极点表不同，多是极点表没有的简码（咪 `ko`、式 `aad`），是正常的候选。
  - 同一编码的行按上游权重从高到低稳定排序，换算成同一权重之后 rowid 的先后仍是词频的先后。文件顺序大体就是这个次序，只有 `qtwb`（犹他 626000 在 狗仔 5680000 之前）和 `zzxx`（θ 454 在 ι 463 之前）两处不是。
  旧布局分不出尾段，按原样读。
- **`rank()` 的同码最低权重和「四码唯一」只算构建会写入的行**（`msime::unsupported_wubi_value` 为假），三个生成器的候选词也按同一个判断筛。
- **`unsupported_wubi_value` 加上 U+F900–U+FAFF**，86、98、新世纪三张表都不写兼容汉字区的字，包括区内 12 个没有分解的字（只见于日文人名等场合）。
- 补充表与辅助码表的表头按实际读到的文件列出每个 86 文件的 SHA-256，新布局下三份补充表的「上游」一行还列出 `rime-wubi86-jidian` 和 `rime-wubi` 两个提交（`Files::references`）；旧布局下表头与拆分之前一字不差（`wubi86_table::described`）。

## 跨仓顺序

`Dictionary::open` 遇到 msime 锁文件里没有的上游名会失败，所以 msime 这一侧（两个 reference、`UPSTREAM_FILES`、布局切换）必须先合，msime-dictionary 拆文件的 PR 再在同一个 PR 里把 `builder.lock.json` 升到 msime 的合并提交。反方向安全：新构建器读没拆的 checkout 走旧布局。拆分 PR 合入时，权重映射、最低权重修正和兼容字过滤同时生效；之前用新构建器构建旧布局的数据，只有后两项生效。

## Alternatives considered

- **把首段改回与上游数据区逐字节一致。** 最强的理由是「上游文件逐字节锁定」的原则本身，以后换上游提交只是替换文件。不用：上游有 5 行没有权重，构建器的行解析会把它们当无效行丢掉，等于少了 5 个词；这次拆分要能用「产物里 wubi86 只因权重映射而变」来审，不能夹带别的变化。差异已经逐条登记，以后换上游提交时一并消掉。
- **尾段按 rime-wubi 词频在同码内排名，给 10 的倍数。** 能让权重本身表达尾段内部的词频次序，不依赖 rowid。不用：尾段 1195 个可写入的行里只有 295 行落在极点表也有的编码上，这些编码的极点最低权重都是 10，名次权重会顶到极点行前面或者还得截回 10。常数 10 加同码内按词频稳定排序得到同样的次序，规则也简单。
- **把 12 个部首、部件当作不支持的字，不写入五笔表。** 最省事，也不必管它们和补充词的先后。不用：rime-wubi 特意收了它们、只是排最后，极点表没有它们，删掉就打不出来；保持 0、写在补充词之后，已经不会抢任何候选。
- **为「版权所有 bqsy」加一张按「词＋五笔码」否决的 curation 表。** 更通用，以后能否决任意五笔错码。不用：`curation/pinyin-vetoes.txt` 只能整词否决或按全拼否决，整词否决会把 版权所有 从拼音库和正确的 `tsrd` 一起删掉；而这一行是「极点表已有这个词」的特例，规则本身有依据，全表只碰到 2 行，不需要再造一张表。
- **旧布局按已知的字节位置切出尾段，也做映射。** 合并文件自 #36 起没变过（SHA-256 `70afd847…`），按摘要识别再按偏移切开是确定的，好处是新构建器配旧数据也能立刻改善排序。不用：要在构建器里写死一份数据文件的摘要和偏移；拆分 PR 本来就和升构建器同时合，旧布局只是兼容路径。
- **在辅助码表里也去掉兼容汉字。** 和 wubi86 表保持一致。暂不做：现在的 `wubi86_helpcode.txt` 里有 7 个兼容汉字正好在「只来自 rime-wubi 的 16 个字」里，去掉要连同各平台许可说明的字数一起改；辅助码表本次不重新生成（两种布局生成的码相同），留给下次重新生成。

## Consequences

- **收益**：86 原表两段都有锁、来源和许可分开登记；尾段的词频不再抢同码首选、不再占一码二码补全前列，部首、部件排在最后，`版权所有 bqsy` 这个错码不再进表；约 2 万个编码的补充词恢复正常权重；兼容字不再和正规字抢位。
- **代价**：尾段内部的次序靠 rowid，不再由权重表达；首段与上游数据区的 6 处差异仍然存在，只是登记了；`版权所有 bqsy` 是在构建时丢掉的，源文件里还在，上游报告草稿另写。msime-dictionary 的三份补充表快照与辅助码表的表头仍写着拆分前的摘要，它们只供审计，下次重新生成时刷新。

## Verification

- 构建器：在独占的 target 目录里经 `rbuild` 跑 `cargo clippy -p msime-dict-builder --all-targets --locked -- -D warnings`（无告警）和 `cargo test -p msime-dict-builder --locked`（253 个单元测试、14 个集成测试通过）；`cargo test -- --list` 确认 `wubi86_table`、`rank_ignores_rows_the_build_leaves_out`、`rime_wubi_rows_count_at_their_mapped_weight`、`the_split_86_layout_ranks_rime_wubi_rows_below_jidian_rows` 等新测试在二进制里。`python3 scripts/test-language-data-notices.py` 通过。
- 数据：拆分后两个文件首尾相接的 SHA-256 等于 `main` 上的 `wubi86-jidian.txt`（`70afd847…`）；对上游 `28f19e3a` 的数据区重放 NOTICE 写的 6 处改动得到的字节与首段逐字节相同（被删的那一行是 `<TAB>cokg<TAB>10<TAB>全角空格`，词那一列是空串）；按复现规则从 rime-wubi `152a0d3f` 生成的 50497 行与尾段逐字节相同。msime-dictionary 的 `python3 scripts/check_upstream.py` 通过。
- 产物：msime-dictionary 分支 `cleanse-data/wubi` 以 `msime_ref=cleanse/wubi` 试跑发布 workflow（publish 关闭）成功，与同一构建器基线（`main` 数据加 `feat/dict-cleanse-round1` 构建器的试跑）用 `scripts/dict_artifact_diff.py` 比较，只有 `msime-wubi.db`、三份补充表和 manifest 变了：`wubi86` 删 23 行（兼容汉字）、改权重 71197 行；`wubi98` 删 25 行（兼容汉字）、改权重 1 行；`wubixsj` 改权重 492 行；拼音、英文等其他库逐表相同。按引擎的前缀查询次序算，一码前缀补全前 5 里有 rime-wubi 行的从 25/25 降到 0/25，二码从 279/625 降到 9/625（剩下的 9 个是 rime-wubi 在该二码本身或其下有行，排在同码极点行之后，例如 `ko` 的 噗、咪）；极点表有行却由 rime-wubi 行当首选的编码从 286 个降到 2 个（`rtgt` 的 龵、`vhgd` 的 ⺻：这两个编码上的极点行都是构建不写入的扩展区字，rime-wubi 的部首成了唯一的原表行）；`wubi86-supplement.txt` 里权重为 0 的行从 136866 行（28742 个编码）降到 66864 行（9958 个编码），剩下的是同码补充词多于 10 个、从 9 往下排到底的。
- 审查后的修正（尾段 0 权重保持 0 并写在补充词之后、极点表已有的词组不收、同码按词频排、表头列出 86 上游）：经 `rbuild` 在独占的 target 目录里 clippy 无告警，257 个单元测试、14 个集成测试通过，`-- --list` 确认 `rime_wubi_rows_yield_to_the_jidian_table`、`wubi86_writes_zero_weight_rows_after_the_supplement`、`a_zero_weight_row_does_not_set_the_ceiling` 在二进制里。试跑 `41836044307` 与上一轮试跑 `41831735318` 比，只有 `msime-wubi.db` 的 `wubi86`、三份补充表的表头和 manifest 变了：`wubi86` 删 2 行（版权所有 `bqsy`、T恤 `ttnt`），12 个部首、部件从 10 回到 0，`qtwb`、`zzxx` 两处同码次序按词频调过来；补充词的权重一行没变。12 个部首、部件在各自编码上都排在最后（`gggg` 第 60/60、`rtgt` 第 17/17），`vhg` 补全前 5 从 [⺻ 盡 賮 那些天 九点五十] 变为 [盡 賮 那些天 九点五十 如此一来]，`agh` 前 5 不再有 龷；`bqsy` 的首选是 除皱术；尾段行排到同码极点行之前的编码为 0。
