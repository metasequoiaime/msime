# Agent Note: 候选窗不再画整句候选的逐词拆解

Status: implemented

## Problem

#2610 给 2 到 32 个汉字的整句候选在释义下面加了一行逐词拆解（`大 big · 肥 fat · 鱼 fish`），macOS 和 Windows 两个宿主都画。拆解没有单独的开关：打开候选翻译、目标语言里有英文就出现，所以默认偏好下几乎每个多字候选都多一行。用户在真实输入里看到的是每行候选都变成三行（候选、释义带音标、拆解），拆解和上面那行释义说的多半是同一件事（`答飞鱼` 的释义是 `Answered flying fish`，拆解是 `答 answer · 飞鱼 flying fish`），候选窗显得杂乱，用户明确要求去掉。

## Decision

两个桌面宿主都不再请求、也不再画逐词拆解，候选翻译那一行和它的音标不变。

- 共享层：`crates/host-api/src/ffi/providers.rs` 的 `msime_client_translation_query` 不再生成 `gloss_breakdown`，`resources` 只在离线释义或读音需要时带上。
- macOS：`InputController.mm` 去掉 `synchronizeGlossBreakdowns`、`breakdownForCandidate` 和它们的缓存，`CandidateChrome.h` 去掉 `breakdown` 属性，`CandidatePronunciation.h` 的 `MSIMECandidateGlossDisplay` 不再有拆解参数。
- Windows：`TranslationWorker.cpp` 不再调用拆解，也去掉了「这一页没有任何释义、只有拆解」时单独交回读音的分支（没有释义时读音一定为空，这个分支只为拆解存在）；`CandidateReading`、`PresentationCandidate` 不再带 `breakdown`，`candidate_gloss_display` 不再有拆解参数，`gloss_sentence_candidate` 随之删除。

`msime_client_gloss_breakdown_request`（`word_breakdown.rs`）和 Apple 侧的包装 `MSIMEClientSession glossBreakdownRequest:` 保留：它们是独立的能力，[两张释义表都不在时逐词拆解回答空列表](../bug-fix/2026-10-11-gloss-breakdown-needs-tables.md) 记的契约仍然成立。[英文释义三张表](../process/2026-10-11-ship-english-gloss-tables.md) 照旧随包发布，单字、长尾词的英文释义和音标还要用它们。

## Alternatives considered

- **加一个「显示逐词拆解」开关，默认关** — 留住这项能力，想看的人自己打开。但要在共享偏好里加字段，改设置页、各宿主的设置窗口和账号同步的字段表，用户要的是不再看到这一行，不是多一个选项；以后真有需要，从这次的 diff 反向恢复也比维护一个没人开的开关便宜。
- **只在宿主里不画，查询和请求照旧** — 改动最小。但每页候选仍要多读一次本机词表、缓存一份用不到的结果，Windows 还留着一条只为拆解存在的回传分支，删到一半最容易让后来的人以为它还在用。
- **连同共享层的 FFI 和 `word_breakdown.rs` 一起删** — 最彻底。但那是另一层的公开接口，删它要单独评估有没有别的调用方和释义表的去留，不和这次界面上的取舍绑在一起。

## Consequences

- **收益**：每个多字候选少一行，候选窗高度和信息量回到「候选 + 释义（带音标）」；每页候选不再多一次拆解查询。
- **代价**：不认识整句里某个词的用户失去了逐词对照，只能看整句释义。Windows 和 macOS 的行为照旧一致。
- 共享层的 FFI 现在没有宿主调用，[Windows 候选窗对齐 macOS](../feature/2026-10-10-windows-candidate-window-mac-parity.md) 里「读音与逐词拆解」一段描述的拆解部分已不再成立，那里的读音部分仍然有效。

## Verification

- `crates/host-api/src/tests.rs`：只开候选翻译时，查询里没有 `gloss_breakdown`，读音和离线释义都关着时也不带 `resources`。
- macOS `tests/candidate/CandidatePronunciationTest.mm`：释义显示只拼释义和读音。
- Windows `tests/candidate/candidate_gloss_readings.cpp`、`tests/candidate/korean_hanja_presentation.cpp`、`tests/ui/candidate_mailbox.cpp`：显示、次要文字、悬停提示和读音挂载里都不再有拆解。
