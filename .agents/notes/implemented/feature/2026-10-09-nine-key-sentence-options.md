# Agent Note: 九键遵守句子联想设置，键盘模型跨切分统一重排一次

Status: implemented

## Problem

#6059：九键打长句时整句首选常常不对。原因有两层：一是九键按切分路径各自解码、再跨路径比较（见 [九宫格切分路径按音节频度截断](../bug-fix/2026-10-08-nine-key-path-ranking.md)），这是结构性上限；二是九键根本没接上句子联想设置。`Session::new` 只把笔画库和 emoji 选项交给 `NineKeySession`，九键自己用 `QuanpinDictionary::new` 打开词库，词库停在默认值：用户关掉「词网格整句」九键照样出整句，打开「键盘神经联想」九键也没有重排，整句备选（`sentence_alternatives`）和上屏上下文（`rescoring_context`）对九键都不起作用。

本篇只处理第二层：让九键与 26 键遵守同一组设置。数字层统一词网格和专门为九键训练模型不在范围内。

## Decision

- `Session::new` 把 `sentence_association`、`sentence_alternatives`、`rescoring_context` 交给 `NineKeySession::set_sentence_options` / `set_rescoring_context`；`Session::set_rescoring_context` 同时更新九键。九键打开词库统一走 `open_dictionary`，带上这些设置。
- 词网格开关、整句备选直接交给九键的词库，效果与 26 键相同：关掉词网格时九键没有词网格整句行，打开整句备选时每条切分交回全部整句读法。
- 键盘模型不交给词库。26 键在词库里对一串音节的几个最好整句重排；九键每次按键要查几十条切分，词库里有重排器时每条切分各重排一次。九键改为自己持有一个 `NeuralReranker`（第一次需要时加载），在各条切分的整句按 `comparable_weight` 排好之后，取最前的 `MAX_RERANK_PATHS` 条（交给模型的词网格分就是 `comparable_weight`）统一重排一次。放置规则沿用 `lattice::merge::reranked_block`：模型最看好的整句不是词网格最好的那一句时，改成 `NeuralKeyboard` 行，紧跟在词网格最好的整句后面；两者相同时只有打开 `show_next_on_duplicate` 才看模型的下一句（`place_keyboard_pick`）。
- 模型要重排的是词网格整句，所以模型打开而词网格关着时，词库照样解出整句（`dictionary_association` 把 `word_lattice` 设为两者之或），挑完模型行再把其余词网格整句去掉，只留模型那一行，与 26 键在这两个开关下的结果相同。这时词网格最好的整句不显示（`reranked_block` 的 `include_lattice_best` 为假），所以模型最看好的整句即使就是词网格那一句，也照样改成模型行占它的位置，`show_next_on_duplicate` 只在词网格那一行显示时才起作用；模型读不到或重排失败时不留任何整句行。
- 模型给出的顺序只取决于上屏上下文、交给它的整句和词网格分，九键按这三者缓存最近 `RERANK_CACHE_ENTRIES`（16）次重排的结果：退格回到前一串数字、选词或筛选后刷新时不再跑模型。26 键缓存的是整张重排过的列表（按 series 槽），九键的列表每次由各条切分合成，只缓存模型这一步。关掉模型时清空。

## Alternatives considered

- **把 `neural_keyboard` 原样交给九键的词库**：改动最小，九键和 26 键走完全相同的代码。代价是每次按键要对每条有整句的切分各跑一次模型：长数字串一次刷新有几十条切分（每个位置最多 `PATH_LIMIT` 条），音节完整的每条都解一次整句，模型开销随条数线性增长，而 26 键的按键延迟预算（`rerank_latency`，p95 16 ms）是按一次重排定的。跨切分统一重排一次，开销与 26 键同一个量级，所以不用这个做法。
- **只在列表最前几条切分上让词库重排**：仍然是逐条切分重排，还要引入一个凭经验调的条数；而且各条切分里被模型挑中的句子之间还得再比一次，问题没有消失。

## Consequences

- **收益**：九键与 26 键遵守同一组句子联想设置；打开键盘模型后九键也有模型行，多出的开销是一次最多 12 句的重排。
- **代价**：关掉了「词网格整句」的用户在九键上不再看到整句行（与 26 键一致，但行为变了）。九键的模型只在各条切分的最好整句之间重排，不像 26 键那样能挑同一串音节里的次优切词；整句首选的准确率仍受切分路径解码的结构性上限约束，那一层要靠数字层统一词网格。
- 本机进程 CPU 时间的粗测（Mac Studio，同时跑着 CI，未入库的临时探针在出货资源上用 `host::Session` 逐键输入，九轮取中位数，词库缓存热）：`9436364782662` 十三键合计 91 ms → 218 ms，单键最多 12 ms → 32 ms；`943426943426` 104 ms → 207 ms；`64426` 2 ms → 7 ms。数字只作量级参考，没有在移动设备上测过；上线前应在安卓真机上打开「增强」量一次九键的按键延迟。宿主的运行时重排器照旧在引擎之外再跑一次，九键自己的模型和 26 键的各有一份上下文缓存。

## Verification

- `cargo test -p msime-engine`：`nine_key::tests::sentence_rows_follow_the_sentence_association_options`（词网格开关、整句备选、词库打开前设好的设置）、`a_missing_keyboard_model_leaves_the_lattice_rows_to_their_switch`、`the_keyboard_pick_follows_the_best_lattice_sentence`（放置规则、`show_next_on_duplicate`、词网格关闭时模型行即使与词网格那一句相同也照样出）、`the_rescoring_context_keeps_the_characters_the_model_reads`，以及 `session::tests::nine_key_follows_the_sentence_options_like_the_full_keyboard`（经 `SessionOptions` 传入，26 键和九宫格结果一致）。
- `crates/engine/tests/golden` 不受影响：九键场景里只有 `bridge_nine_key_spelling_choices` 打开了整句备选，它只打一位数字，没有整句行；其余场景用默认设置，交给词库的设置与改动前相同。`cargo test -p msime-engine --test golden` 通过，基准未改。
