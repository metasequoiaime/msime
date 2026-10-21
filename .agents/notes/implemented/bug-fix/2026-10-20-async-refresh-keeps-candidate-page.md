# Agent Note: 组字不变时异步刷新保留已翻开的候选页

Status: implemented

## Problem

macOS 上组字停住后按 `+` 翻到下一页，云联想回包到达后候选页回到第一页。关掉云联想后，这次回弹消失。页码来自 `highlighted / page_size`。云回包走 `Runtime::refresh`。组字没变、只有候选被插入或重排时，`refresh` 仍把 `highlighted` 置 0。macOS 每次 `apply:` 还会在约 150ms 后跑 `rerank_settled`；顺序有变化时它同样把高亮置 0。

## Decision

组字字段保持不变时，`refresh` 按下面三条放高亮。比较的字段是 `editing_text`、`scheme`、`local_mode`、`reading`、`dedicated_english`、`conversion`、`conversion_focus_start`。

- 候选列表也没变：高亮留在原下标，并夹在可见范围内。
- 候选变了且高亮原来就在第一条：留在第一条。云词仍插在第二格，空格选中的是新的第一条。
- 候选变了且高亮原来不在第一条：按文本和编码，在当前页或更后的页找回同一条。该条只出现在更前的页时不跟回去，停在原页开头。找不到也停在原页开头。

任一组字字段变化时，高亮回到第一条。`rerank_settled` 在顺序变化且高亮原来不在第一条时用同一条找回规则；原来在第一条时仍停在第一条。

## Alternatives considered

- 只在 macOS 候选窗自己记住页码。页码的来源是 runtime 的 `highlighted`，云回包和静止重排都会再走 `refresh`。窗口里记住的页会被下一次 `apply:` 盖掉，其它宿主也还是会弹回第一页。
- 云回包期间一直冻住高亮下标，直到下一次按键。高亮停在第一条时，用户要的是空格选中新插进来的第一条；冻住旧下标还会在列表变短时越界。
- 词被排到更前的页时，把已翻开的页拉回去跟着那条词。用户按 `+` 是要留在下一页，跟回去就回到第一页。

## Verification

库测试用夹具，不加载真实词库。`a_late_cloud_candidate_keeps_the_page_the_user_opened` 覆盖迟到的云词，`a_promoted_candidate_does_not_pull_an_open_page_back` 覆盖 `refresh` 里被提前的词。`a_settled_rerank_does_not_pull_an_open_page_back` 覆盖静止重排把高亮那一条提到更前的页。三条都要求已翻开的页留在原地。macOS App 没有编译、没有安装、没有启动。

## Consequences

云联想、AI 候选和静止重排可以在组字不变时改列表。已经翻开的页留在原页。高亮还在第一条时，迟到的云词不抢走空格。被排到更前页的那条词，已翻页的用户在当前页看不到它，按上一页可以见到。新的按键仍从第一页开始。不改页码计算、云词座位或选词提交。

## 历史笔记检索

按 `highlighted`、`refresh`、云联想检索活跃笔记。没有一篇持有「异步改候选时高亮放哪」。[在线座位暂存](../../implemented/testing/2026-10-16-online-seat-scratch-buffers.md) 只减少 `normalize_online_slots` 的分配。[Android 回车长按翻译](../../implemented/feature/2026-10-12-android-ctrl-enter-translation.md) 让释义页在组字不变时跟上在线刷新的代次，不决定候选页高亮。两篇都保留原决定。
