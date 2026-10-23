# Agent Note: 词网格给句末语气词加分，叠字叹词不拿词组奖励

Status: implemented

## Problem

iOS 用户反馈九键「打快了吞键」。真机诊断日志显示按下的数字一个不少地进了引擎，问题在排序：`qiguaiya`（奇怪呀）出「是胎压」，`xiangguanba`（相关吧）出「相关啊啊」，`zhaiquanma`（债券吗）出「债券那」。

从锁定词库按权重靠前的两字词里抽 120 个，接上 啊、呀、吧、呢、吗、啦 在三种键盘上打（整理后成为 `resources/eval/sentences-particles-v1.tsv`），修改前 top-1 全键盘只有 0.524、14 键 0.282、九键 0.146。按失败原因分：全键盘 59 条失败里 33 条是词干对、句尾字错（标注亚、债券马）；九键 104 条失败里 30 条是同一类，10 条是词干后面跟叠字叹词（相关啊啊、基本啊啊），其余 64 条是整串数字被解成完全不同的读法。

前两类有两个原因。一是 `msime-bigram.bin` 只有句首记号 `SENTENCE_START`，没有句尾，又是从维基百科统计的，几乎见不到口语句尾，吗、呀、吧只能按单字词频和同音（亚、马）或同码（那、把）的字比。二是「啊啊」「呃呃」是词库里的两字词，`edge_log_prob` 给多音节词每个音节加 `PHRASE_LENGTH_BONUS`，单字边却按 `ln(权重 / 10⁶)` 计分，九宫格上 `22` 的「啊啊」凭这份奖励压过「吧」。

## Decision

`lattice::decode` 在束搜索里，整句最后一条边是 `SENTENCE_FINAL_PARTICLES` 里的单字、前面还有别的词时，给这条路径加 `SENTENCE_FINAL_PARTICLE_BONUS`（3.0 nats）。只看最后一条边，句中、句首的同一个字不加；单个音节本来就不进词网格。

由同一个叹词（`REDUPLICATED_INTERJECTIONS`：啊 呃 哦 嗯 呀 哇 噢 唉 哎 喔 诶 额 谔）叠成的词算一个音节计词组奖励（`bonus_syllables`），即按单字口径计分。爸爸、哈哈这类叠字词照旧拿奖励。

加分值在 120 条里 id 为偶数的一半上从 2 扫到 5：3 是现有八份评测集一项都不退步时最大的值，4 和 5 让九键长句集原本找得到的一条金标准掉出列表（found 0.728 → 0.716）。奇数一半没有参与调参，在它上面全键盘 0.583 → 0.767、14 键 top-5 0.617 → 0.867。

整理后的 103 条前后对照（top-1 / top-5）：全键盘 0.524 / 0.951 → 0.680 / 0.971，14 键 0.282 / 0.650 → 0.311 / 0.854，九键 0.146 / 0.641 → 0.194 / 0.728。叠字叹词这一项单独把九键长句集 top-1 从 0.617 提到 0.642；14 键长句集指标不变，两条原本失败的用例金标准排位往前挪（45 → 44、46 → 43）。全键盘三份整句集、词级集、九键和 14 键词级集逐条不变。

## Alternatives considered

- 在 n-gram 表里加句尾记号：语言模型的正统做法，但表从维基百科统计，口语句尾样本太少，统计出来的句尾概率帮不上这类输入；还要重新发布词库资源。
- 接 octagram `.gram` 模型的 `$` rear 项（[语法模型读取端](../feature/2026-10-21-octagram-grammar-model.md)）：解码端已经支持，但模型是桌面按需下载的 39 MB 到 380 MB 资源包，手机上没有，九键用户正是手机用户。
- 九键里不再让词库首行自动受信、交给重排模型或格子分加模型分比较：在词级集上把九键 top-1 从 0.571 拉到 0.279 到 0.510，模型对词库词的判断不如词频，与上游 `chinese-ime-lm` 文档说的一致，放弃。
- 统一词组边与单字边的计分口径：九键剩下的大头（一词全码、此案蛀牙）源于它，但[九宫格切分路径笔记](2026-10-08-nine-key-path-ranking.md)记过一次只扣实得奖励的尝试，整句集 top-1 从 0.233 掉到 0.150，牵动整个词网格，本次不做。

## Consequences

全键盘、九键、14 键的整句行都受影响：句尾的吗、呀、吧、呢、啦等更容易排在同音或同码字前面，句中这些字不受影响。真实聊天里「他吗」这类句尾本该是别的字的输入会多一点倾向语气词，评测集上没有观察到退步。

只改整句行。两个词库词条之间的先后仍按词库权重：九键 `7448242` 的「沏壶茶」（650）照旧排在「奇怪啊」（100）前面。九键里整串数字被解成完全不同读法的失败（`sentences-particles-v1.tsv` 九键约三分之二）不在本次范围。

`resources/eval/baseline-{sentences,harvested,neutral}.json` 与当前 develop 在同一环境下跑出来的数字已经不一致（harvested top-1 基线 0.324，实跑 0.503），与本次改动无关，本次没有更新它们。

## Verification

- `cargo test -p msime-engine --lib lattice::`：161 项通过，新增 `a_sentence_final_particle_beats_a_commoner_homophone`、`the_particle_bonus_is_only_for_the_last_word`、`reduplicated_interjections_take_no_phrase_bonus`；`cargo clippy -p msime-engine --all-targets -- -D warnings` 干净。
- `convert_eval` 在锁定资源上跑全部十一份集合；新增 `baseline-particles*.json` 三份，更新 `baseline-nine-key-sentences.json`、`baseline-fourteen-key-sentences.json`。
