# Agent Note: 两张释义表都不在时逐词拆解回答空列表

Status: implemented

## Problem

`msime_client_gloss_breakdown_request` 把整句候选按最长匹配切成词，每个词取一条英文：先查 `word_breakdown.rs` 的 `LEARNER_PHRASES`（130 个高频词指定的学习者义项），再查 resources 旁边的 `character-glosses/zh-en.db`（Unihan）和 `word-glosses/zh-en.db`（CC-CEDICT）。`render` 只要求至少两段有英文、不超过八段。

两张表都不在时，只剩学习者短语能对上，句子里大半的字没有英文，却因为凑够两段照样显示：`别问我` 画成 `别 do not · 问 to ask · 我`，`别问握` 画成 `别 do not · 问 to ask · 握`。发布包一直没有带这两张表，所以 macOS 和 Windows 的正式版在每个含两个学习者短语的句子候选下都是这种半截的行。宿主两侧本来都按「表没装时回答空列表」来写：Windows `TranslationWorker.cpp` 的注释这么说，引入拆解的 #2610 也这么说，只是实现没有做到。

## Decision

`word_breakdown::known_pieces` 返回 `Option`：`character-glosses/zh-en.db` 和 `word-glosses/zh-en.db` 都不在 resources 旁边时返回 `None`，请求回答空的 `breakdowns`。只要有一张表在，行为不变，学习者短语照旧优先于两张表。宿主不需要改：macOS 的 `synchronizeGlossBreakdowns` 和 Windows 的 `TranslationWorker` 把空回答当成没有拆解。

## Alternatives considered

- **`render` 要求每一段都有英文** — 不看表在不在，直接挡住任何带光秃汉字的拆解。但表齐全时，没有释义的多半是生僻字，`我 I · 疼 · 你 you` 这种拆解仍有用，`an_unknown_character_stays_without_a_gloss` 测的就是这种情形；表缺失则是另一回事，整行几乎都对不上。按表在不在来判，正好挡住后者而不影响前者。
- **宿主侧在请求前检查表在不在** — macOS 和 Windows 各写一遍，Windows 那边的注释已经认定这件事归共享层管。

## Consequences

- **收益**：表缺失时不再出现半截的拆解行，和宿主一侧认定的契约一致。
- **代价与已知上限**：表缺失的包完全没有逐词拆解，学习者短语恰好覆盖整句的少数拆解（比如 `别问` 的 `别 do not · 问 to ask`）也一起没了。让正式包真正有拆解，靠的是把两张表打进包。

## Verification

`crates/host-api/src/tests.rs` 的 `gloss_breakdown_request_lines_sentences_up_with_english_words` 在什么都没装时请求 `别问我`，修复前回答 `别 do not · 问 to ask · 我`，修复后回答空列表；装上两张夹具表后的各种情形不变。`cargo test -p msime-host-api --lib breakdown` 10 项通过，`cargo clippy -p msime-host-api --all-targets -- -D warnings` 无警告。
