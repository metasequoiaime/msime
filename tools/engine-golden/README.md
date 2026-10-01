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

The `dict-v2.0.2` set pinned by `resources/desktop-dictionary.lock.json` (sha256 of every file checked against the lock), minus `dict_pinyin.dat` and `sentence-model*.safetensors`: `msime.db english.db bigram.bin trigram.bin others.db dict_japanese.dat dictionary-manifest.json mozc_dictionary_oss_README.txt`.

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
 "fixture": {"english_schema": false, "databases": {"msime.db": "SQL", "english.db": "SQL", "others.db": "SQL"},
             "files": {"helpcodes/helpcode.txt": "text"}},
 "options": {"scheme": "quanpin", "learning": true, "frequency": {"mode": "pin"}, "...": "..."},
 "steps": [{"op": "type", "arg": "nihao"}, {"op": "select", "arg": {"word": "拟好"}}, {"op": "reopen"}]}
```

- Fixture SQL is copied verbatim from the reference ctest named in `source`. The fixture directory becomes `resources`, staged with `prepare_runtime_paths(resources, user, cache, "v1")` like the product; an absent `english.db` gets `EnglishDictionary::ensure_schema`, and `english_schema: true` runs it before the fixture SQL. Tests that used `RuntimePaths::legacy()` get this staged layout instead, the one the product uses (`crates/engine-bridge/native/bridge.cpp:699`).
- Options: only fields of the reference `SessionOptions` (`include/metasequoia/session.h:12-54`); unset fields keep that struct's defaults, which is what the ctest's own session used. `learning_undo` is refused (feature dropped). `page_size` enables the recorder-side `page` / `select_on_page` view (the engine returns the whole list; paging is input-runtime's). The view returns to page 0 whenever the editing text or candidate list changes, as a host does.
- Ops mirror `metasequoia::Session`: `type` (one `character()` per byte), `char` (+`shift`), `command` (`Command` enum name), `candidate_key`, `punctuation`, `select` / `pin` / `remove` / `clear_position` / `select_edge` (+`edge` first|last) / `fix_position` (+`position`) with an index or `{"word": ...}`, `finish` (optional index), `switch_scheme`, `set_helpcode_schema`, `set_helpcode_enabled`, `set_dedicated_english`, `set_wubi_mixed_pinyin`, `set_chinese_punctuation_enabled`, `set_punctuation_lock`, `set_paired_punctuation_enabled`, `balance_paired_punctuation_after_auto_close`, `set_nine_key_enabled`, `choose_nine_key_spelling` (index or `{"spelling": ...}`), `set_personal_context_enabled`, `reset_cache`, `reset_context`, `expand_initial_candidates`, `apply_online_candidates` (+`source` cloud|ai, against the live query), personal dictionary `validate_entry` / `dict_edit` / `dict_list`. Control: `reopen` (new Session on the same user data; re-preparing replays the journal), `new_generation` (content id), `dump_journal`, `query` (read-only SQL on the generation's `msime.db` / `english.db`, or `msime_user.db`).
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
