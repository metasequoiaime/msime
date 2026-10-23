# engine-golden

Records the C++ reference engine's behaviour once, as JSON under `crates/engine/tests/golden/`, so the Rust engine (`crates/engine`) is tested against committed data and the C++ engine can be deleted. After the C++ is gone this recipe only matters for re-recording from an archived reference build.

The recipe below is historical. `engine-lock.json`, `scripts/fetch_engine.py` and the `scripts/apply_engine_*.py` overlays it runs were removed with the C++ engine, and `crates/engine-bridge` (cited for the product options) went in `addfbd4ab`; to re-record, take them from a commit before their removal, such as `e8056a24e`.

## Oracle

- Engine: `metasequoiaime/msime-engine` commit `a9b9f092219505166c927843762b650d1e0e501d`, archive sha256 `9510e03f761e94c44c9dcba74f84de23db661251b0eb67766aba0991a7e5445e`, plus the `dependencies` of `engine-lock.json` (googlepinyinime-rev, utfcpp, ...) extracted at their paths, exactly as `scripts/fetch_engine.py` stages them.
- 23 overlays applied, in `engine-lock.json` order: alternative_segmentation_page, double_helpcode_cache, english_display, expand_initial_candidates, frequency_comparison_set, google_umlaut, jiajia_helpcode, lattice_reading, standalone_sentence_learning, quanpin_autocorrect_parity, manual_segmentation_cloud, wubi_prefix_learning, shuangpin_yo, xuan_single_character, punctuation_alternation, bundled_dictionary_entries, paired_punctuation_ipc, local_mode_fallback, custom_helpcode, personal_learning, online_candidate_dedup, wubi_mixed_candidates, shuangpin_sentence_score (each `scripts/apply_engine_<name>.py`).
- 3 overlays NOT applied (they fail to apply to this archive): `wubi_mixed_routing`, `neural_association`, `caret_prefix`. Nothing they add has a golden; the Rust side needs hand-written tests for them.
- Not engine behaviour, so not recorded: the Google decoder (`dict_pinyin.dat`, dropped) and the sentence reranker (`sentence-model*.safetensors`).

## 1. Reference tree and build

```sh
REF=/path/to/engine-ref          # holds MSIME-Engine/ (pristine archive + dependencies)
cd <msime checkout>
for s in alternative_segmentation_page double_helpcode_cache english_display expand_initial_candidates \
         frequency_comparison_set google_umlaut jiajia_helpcode lattice_reading standalone_sentence_learning \
         quanpin_autocorrect_parity manual_segmentation_cloud wubi_prefix_learning shuangpin_yo \
         xuan_single_character punctuation_alternation bundled_dictionary_entries paired_punctuation_ipc \
         local_mode_fallback custom_helpcode personal_learning online_candidate_dedup wubi_mixed_candidates \
         shuangpin_sentence_score; do
  python3 scripts/apply_engine_$s.py "$REF/MSIME-Engine"
done
cmake -S "$REF/MSIME-Engine" -B "$REF/build-ref" -DCMAKE_BUILD_TYPE=Release   # Unix Makefiles, AppleClang 21, Homebrew boost/fmt/spdlog/sqlite
cmake --build "$REF/build-ref" --target MetasequoiaImeEngine -j8
```

The recorded goldens came from macOS arm64 with that exact configuration (`-O3 -DNDEBUG -std=c++17`, `FMT_HEADER_ONLY=1`).

## 2. Recorder

```sh
cmake -S tools/engine-golden -B "$WORK/build" \
  -DMSIME_ENGINE_REF_SRC="$REF/MSIME-Engine" -DMSIME_ENGINE_REF_BUILD="$REF/build-ref"
cmake --build "$WORK/build"
```

Links `libMetasequoiaImeEngine.a` the way the reference test targets do (Boost headers, SQLite3, Threads) and uses Homebrew's header-only `nlohmann_json` for JSON (tooling only, nothing ships). `$WORK` must be a scratch directory, never inside the repo.

## 3. Resource directory (real-dictionary goldens)

四份真实词库金样在提交 `a42356724c597e051f360b41e308e51b4d12dda7` 录制，使用该提交的 [`resources/desktop-dictionary.lock.json`](https://github.com/metasequoiaime/msime/blob/a42356724c597e051f360b41e308e51b4d12dda7/resources/desktop-dictionary.lock.json) 固定的 `dict-v2.0.1`。按这份历史锁校验每个文件的大小和 SHA-256；当前工作树的资源锁已随产品升级，不能用于重建这批录制输入。后来的 `dict-v2.0.5` 文件名迁移没有重新录制这四份 JSONL。

参考引擎的历史录制目录保留原文件名，包含下面八项资源。当前 Rust 回放器按现行资产名查找文件，给 `MSIME_EVAL_RESOURCES` 准备目录时只按右列改名，文件字节仍须匹配历史锁；不加入 `dict_pinyin.dat` 或 `sentence-model*.safetensors`。

| `dict-v2.0.1` 原文件名 | 当前 Rust 回放目录中的文件名 |
| --- | --- |
| `msime.db` | `msime-pinyin.db` |
| `english.db` | `msime-english.db` |
| `bigram.bin` | `msime-bigram.bin` |
| `trigram.bin` | `msime-trigram.bin` |
| `others.db` | `msime-others.db` |
| `dict_japanese.dat` | `msime-japanese.dat` |
| `dictionary-manifest.json` | `msime-dictionary-manifest.json` |
| `mozc_dictionary_oss_README.txt` | `msime-mozc_dictionary_oss_README.txt` |

资源匹配只排除词库版本差异。录制后有意改变的引擎行为仍可能产生回放差异，需要对照对应的行为决定逐项核实。

## 4. Record

```sh
tools/engine-golden/record.sh "$WORK/build/engine_golden_recorder" "$WORK/run"            # every scenario
tools/engine-golden/record.sh "$WORK/build/engine_golden_recorder" "$WORK/run" <name> ...   # some

R="$WORK/build/engine_golden_recorder"; OUT=crates/engine/tests/golden/real
for s in sentences-v1 sentences-neutral-v1 sentences-v2; do
  $R real "$RES" resources/eval/$s.tsv $OUT/$s.jsonl "$WORK/run"
done
$R real "$RES" resources/eval/quanpin-words-v1.tsv $OUT/quanpin-words-v1.subset3000.jsonl "$WORK/run" --subset 3000
```

Record twice and `diff -r`; both outputs must be identical.

## Scripted scenarios (`golden/scenarios/<name>.json` -> `golden/expected/<name>.json`)

```json
{"name": "...", "source": "tests/src/test_x.cpp:lines", "note": "optional",
 "fixture": {"english_schema": false, "databases": {"msime-pinyin.db": "SQL", "msime-english.db": "SQL", "msime-others.db": "SQL"},
             "files": {"helpcodes/helpcode.txt": "text"}},
 "options": {"scheme": "quanpin", "learning": true, "frequency": {"mode": "pin"}, "...": "..."},
 "steps": [{"op": "type", "arg": "nihao"}, {"op": "select", "arg": {"word": "拟好"}}, {"op": "reopen"}]}
```

- Fixture SQL is copied verbatim from the reference ctest named in `source`. The fixture directory becomes `resources`, staged with `prepare_runtime_paths(resources, user, cache, "v1")` like the product; an absent `msime-english.db` gets `EnglishDictionary::ensure_schema`, and `english_schema: true` runs it before the fixture SQL. Tests that used `RuntimePaths::legacy()` get this staged layout instead, the one the product uses (`crates/engine-bridge/native/bridge.cpp:699`).
- Options: only fields of the reference `SessionOptions` (`include/metasequoia/session.h:12-54`); unset fields keep that struct's defaults, which is what the ctest's own session used. `learning_undo` is refused (feature dropped). `page_size` enables the recorder-side `page` / `select_on_page` view (the engine returns the whole list; paging is input-runtime's). The view returns to page 0 whenever the editing text or candidate list changes, as a host does.
- Ops mirror `metasequoia::Session`: `type` (one `character()` per byte), `char` (+`shift`), `command` (`Command` enum name), `candidate_key`, `punctuation`, `select` / `pin` / `remove` / `clear_position` / `select_edge` (+`edge` first|last) / `fix_position` (+`position`) with an index or `{"word": ...}`, `finish` (optional index), `switch_scheme`, `set_helpcode_schema`, `set_helpcode_enabled`, `set_dedicated_english`, `set_wubi_mixed_pinyin`, `set_chinese_punctuation_enabled`, `set_punctuation_lock`, `set_paired_punctuation_enabled`, `balance_paired_punctuation_after_auto_close`, `set_nine_key_enabled`, `set_key_grid` (`none` / `nine_key` / `fourteen_key`; Rust-only, the C++ reference has no 14-key grid), `grid_key` (one `grid_key()` per byte, reported like `type`), `choose_nine_key_spelling` (index or `{"spelling": ...}`), `set_personal_context_enabled`, `reset_cache`, `reset_context`, `expand_initial_candidates`, `apply_online_candidates` (+`source` cloud|ai, against the live query), personal dictionary `validate_entry` / `dict_edit` / `dict_list`. Control: `reopen` (new Session on the same user data; re-preparing replays the journal; an optional `options` object, in the scenario `options` shape, is applied before the new Session opens, which is how options with no live setter such as the Vietnamese tone style change mid-scenario), `new_generation` (content id), `dump_journal`, `query` (read-only SQL on the generation's `msime-pinyin.db` / `msime-english.db`, or `msime_user.db`).
- Scheme names for `options.scheme` / `switch_scheme` include `vietnamese`, which takes `vietnamese_input_method` (`telex` | `vni`) and `vietnamese_tone_style` (`modern` | `classic`) in `options`, and `cantonese` and `zhuyin`, which read a `msime-cantonese.db` or `msime-zhuyin.db` staged through `fixture.databases` (the `language_dictionary` schema with `metadata` `format_version` 1); the harness hands that file to the session as a host would, since the files ship beside the resource set. A `switch_scheme` the engine refuses (a scheme whose dictionary cannot be opened) records `{"error": ...}` as its result and leaves the session in its scheme.
- Output: step 0 is the snapshot after construction; every step records its `result` (KeyResult or op result) and, unless it is a read-only op, the full snapshot: every `SessionSnapshot` field, `segment_raw_boundaries()`, every candidate (word, pinyin, canonical_pinyin, numeric `CandidateSource`, scheme, weight, fixed_position, fuzzy, corrected_from, sentence_association, sentence_words, annotation, answers_key) and the online query minus its per-process `generation` / `identity` / `session_id`.
- Journal dumps list every table of `msime_user.db` with every column except `updated_at`, ordered by all columns, after `PersonalNgramStore::flush_all()` (its writes are otherwise delayed 2 s).
- Paths in diagnostics are replaced by `$ROOT`.
- `weight` of `Generated` rows is a lattice score and of learned rows the adjusted weight; replayers may compare it or ignore it.
- `qp_generated_sentence` records `Unable to persist the selected sentence.` on selecting 特乐好: the fixture has no `tbl_3_t` for standalone sentence learning to write into. The source ctest (`test_input_session.cpp:507-530`) does not check the diagnostic.
- Adding a scenario: write the JSON (copy the fixture from the ctest, cite it in `source`), run `record.sh ... <name>` twice, diff, commit both files.

## Real-dictionary goldens (`golden/real/*.jsonl`)

One line per eval row: `{"id","input","gold","n": total candidate count, "c": [[word, CandidateSource], ...]}`, a third element is `corrected_from` when set. Rows come from the TSV's `input` (2nd) and `gold` (3rd) columns, skipping `#` lines and the `id` header; `quanpin-words-v1` uses rows `floor(i * 25119 / 3000)`. One Session types every row in file order and sends `Cancel` + `reset_context()` between rows. Rows are independent of order: recording the three sentence sets reversed gives the same lines.

Options are the product's: `prepare_options` defaults (`crates/engine-bridge/native/bridge.cpp:698-733`) mapped by `options_for` (`:354-406`) - quanpin, helpcode on (ziranma), frequency promote, mixed English on with minimum prefix 5, emoji/kaomoji mixing off, all local modes on, autocorrect and fuzzy off - plus `sentence_alternatives = true` (`crates/host-api/src/lib.rs:365`) and `learning = false` so every row is a cold start (the same choice `convert_eval` and `eval_sentences` make).

Rows: sentences-v1 60, sentences-neutral-v1 1105, sentences-v2 312 (the earlier `.2col` eval copy had 310), quanpin-words subset 3000. Top-1/top-5 of candidate 0..4: 60.0/75.0, 48.6/88.1, 46.5/88.8, 70.8/84.6. The sentence sets match the lattice-only eval in the migration notes; the words top-5 does not (94.6 there) because that eval ran with `sentence_alternatives` off, and with it on the raw list leads with alternative Generated readings that the host demotes behind the first (`demote_runner_up_readings`, `crates/input-runtime/src/runtime.rs:1387`, keyed on `LATTICE_SOURCE = 8`, `:119`).

Trimmed: only the first 9 candidates per row are kept (the runtime's page size cap, `crates/input-runtime/src/runtime.rs:478`); annotations are not kept. Total 1.2 MB.

## Not recorded, and why

- Date/time candidates (`Shift+T rq`, `sj`, ...): they read the wall clock and the public Session has no provider hook (`InputSession::set_local_date_time_provider`, `core/input_session.h:109`, is internal). Only the clock-independent unmatched-input path is recorded.
- Personal-context / pick-pair chain breaks that depend on elapsed time (`+3 s` keep, `+30 s` break): same reason (`set_steady_clock_provider`, `core/input_session.h:111`). Every scripted step runs well inside the 3 s window.
- Learning undo: dropped feature.
- Personal-dictionary entries with invalid UTF-8 or an out-of-range kind: not expressible in JSON; port as Rust unit tests.

## Overlay behaviour that the pinned ctests reject

Four reference ctests fail against this build because an overlay changed what the pinned test asserts. The overlay behaviour is the product and is what the goldens record (each such scenario has a `note`):

- `wubi_input_session` -> `wubi_prefix_codes`: `wq` gives [你, 父子, 爷, 你] (apply_engine_wubi_prefix_learning.py ordering, no dedup).
- `wubi_mixed_input_session` -> `wubi_mixed_*`: the tail `wq` after selecting 你好 gets wubi rows (apply_engine_wubi_mixed_candidates.py).
- `nine_key_session` -> `nine_key_*`: `64426` gives [你好, 米好 (Generated), 你, 米, ogham] (apply_engine_lattice_reading.py, 2-syllable lattice).
- `personal_dictionary` -> `personal_dictionary_entries`: `{English, hello, different}` is accepted (apply_engine_english_display.py).

One later product change is recorded the same way: since #5640 the nine-key grid also reads every digit as one syllable's initial, so `64` adds 你好 (n'h) after the syllable rows in `nine_key_*` and `personal_context_public_session`, and the frequency adjustments those scenarios record are computed over the longer list.

Since #7106, Quanpin and Shuangpin keep a shifted uppercase letter in an existing composition even with helpcode off. The six `bridge_helpcode_*_off` scenarios, `qp_helpcode_disabled_uppercase`, and the helpcode-toggle steps in `qp_helpcode_filter` and `ri_finish_helpcode_dedicated_english` record this deliberate product behavior instead of the older C++ rejection. Unchanged steps still match the pinned reference.

Since #5667 / #5907 the mixed emoji and kaomoji rows no longer take the priority slot after the leading row: a row whose catalog keywords (`emoji.keywords`, `kaomoji_catalog.keywords`) include a candidate's word follows the first such candidate, and the rest go to the end of the list, emoji before kaomoji, at most three of each. The `mixed_expressive_*` fixtures have no catalog tables, so every row there goes to the end; their expected lists were reordered accordingly (weights and everything else unchanged). English rows keep the reference's priority slot, with one later exception: when the Quanpin or Shuangpin input does not split into complete syllables and the first English row left after deduplication spells the whole input exactly, that row takes index 0 (`.agents/notes/implemented/feature/2026-10-12-mixed-english-leads-unsegmentable-input.md`). No scripted scenario or real-dictionary input reaches that case, so no expected file changed.

#7583 的共享词网格为每个多字词扣一份插入代价；15 个脚本场景的 Generated 权重因此按每词 20,000 更新。`pick_pair_ineligible` 的重复音节同分路径由 `好 / 好好` 变为 `好好 / 好`，对应的个人二元、三元分词记录随同更新；候选文字、上屏、失败诊断及其余场景保持原断言。完整词条已成为词网格最优路径时，双拼只保留词库行；展示整句备选时其他读法仍跟在完整词条后面。

#7616 之后本地模式下回车（`CommitRaw`）原样上屏屏幕上的预编辑，两个临时模式的入口字母也在内：只有入口字母时上屏它本身（以前上屏空串），有内容时上屏 `Rka` / `Yhe`（以前去掉入口字母记 `ka` / `he`）。`temp_japanese_enter` 与 `temp_english_enter_and_learning` 只在这一处与参考不同；学进词库的词没有变（临时日文仍是带 `r` 前缀的 `rkk`，临时英文仍是 `he`），入口字母在写词库前照旧去掉。

#6888 之后引擎把 AI 行放在候选列表最后（`replace_online_candidate_batch` 和固定位置归位都这样排），第一条显示在第一页最后一格由 input-runtime 安排；英文行的优先位置也不再因为有 AI 行往后让。`online_cloud_and_ai_slots`、`online_learning_rules`、`online_mixed_english_ai_first`、`online_mixed_english_slots`、`online_quanpin_slots`、`online_rows_across_compositions` 和 `online_shuangpin` 里 AI 行和它后面那几行的先后据此更新，字段值和其余步骤保持参考断言（`.agents/notes/implemented/feature/2026-10-16-ai-candidate-last-slot.md`）。

#8043 修复本地模式静默吞键：`local_date_time_incomplete`、`local_emoji_edges`、`local_quick_phrase`、`local_quick_phrase_edges`、`local_super_jianpin_queries`、`local_unicode_edges` 和 `personal_dictionary_entries` 中不被模式接受的字符改为先提交所见原文（含入口字母），再以未处理结果交还当前键。对应步骤及随后空组字的取消／退格期望据此更新，其他步骤保留固定参考断言；临时英文、临时日文和网址的独立拒绝分支保持原有行为。

#9098 之后，全拼、双拼的空 `J` 入口遇到下一枚大写转为普通字面组字，避免 `JIT`、`JNH` 被当作超级简拼查询中文。`local_super_jianpin_queries` 中 `JNH` 的候选改为原文，模式、分词和逐字编辑边界相应更新；后接小写的查询、Unicode 合法大写以及其他步骤继续保留参考断言。
