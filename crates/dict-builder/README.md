# msime-dict-builder

`msime-dict-build` builds the dictionary release msime ships (the artifacts `resources/desktop-dictionary.lock.json` pins): `msime.db`, `english.db`, `others.db`, `dict_japanese.dat`, `bigram.bin`, `trigram.bin`, `mozc_dictionary_oss_README.txt`, then `dictionary-manifest.json` and `SHA256SUMS.txt`. It replaces the Python pipeline in MSIME-Engine `dictionary/` (`build_all.py`, `build_profile.py`, `makecikudb/`) and reproduces its desktop output row for row.

```sh
cargo build --release -p msime-dict-builder
target/release/msime-dict-build --cache <sources-cache> --out <output>                # every stage, about a minute
target/release/msime-dict-build --cache <sources-cache> --out <output> --skip ngram   # without the zhwiki pass
target/release/msime-dict-build --list
target/release/msime-dict-build places --cache <sources-cache>                        # the @ place table the engine embeds
target/release/msime-dict-build hanja --cache <sources-cache>                         # the Korean Hanja table the engine embeds
```

## Inputs

- `resources/dictionary-sources.lock.json` pins every third-party or large input by URL, size and SHA-256: the lexicons and the custom dictionary as assets of one msime-dictionary `sources-v*` release, ECDICT, Mozc's OSS Japanese dictionary sources (mirrored into msime-dictionary) and the zhwiki dump part the n-gram tables count. They are downloaded into `--cache` on first use (about 500 MB) and reused while they still match; `--offline` refuses to download.
- The base lexicons (`cn/`, `en/`) and the custom words, translations and English words (`custom/words.txt`, `custom/translations.txt`, `custom/english.txt`) live in the dictionary source repository [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary), which msime and MSIME-Windows both consume through the dictionary release. The lock pins every file to an asset of one `sources-vX.Y.Z` release (assets are flat, so only the basename appears in the URL) and records the tagged commit as the `msime-dictionary` reference; the lock `path` keeps the repository layout. `cn/RimeIceSupplementV1.txt` is the GPL-3.0 rime-ice delta merged into the licensed Quanpin build, while `cn/Wubi98Fcitx.txt` is the GPL-3.0-or-later Fcitx5 98 Wubi supplement merged into the Wubi 98 table. The Cantonese and Zhuyin sources under `yue/` and `tw/` (rime-cantonese and libchewing-data files that msime-dictionary keeps byte for byte) are read only by `msime-dict-build languages`, so their entries move on their own and may be at a later release than the rest; their upstream commits are the `rime-cantonese` and `libchewing-data` references, which `languages` records in each database as `source_commit` and `scripts/test-language-data-notices.py` checks against the licence files.
- The Stroke scheme's `stroke.db` is built by `languages` from rime-stroke's `stroke.dict.yaml` (LGPL-3.0, `resources/licenses/rime-stroke-LGPL-3.0.txt`), ranked by the per-character sum of the pinned `cn/SingleCharsAllV1.txt` weights. Until msime-dictionary publishes it under `stroke/` in a `sources-v*` release and the lock pins it with a `rime-stroke` reference, `languages` reads it only from `<cache>/stroke/stroke.dict.yaml`, where it has to be placed by hand (`curl -fL -o <cache>/stroke/stroke.dict.yaml https://github.com/rime/rime-stroke/raw/1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48/stroke.dict.yaml`), and checks it against the commit, size and SHA-256 that `src/stroke.rs` records; nothing is downloaded for it.
- `resources/dictionary-sources/` holds the other hand-maintained inputs: quick phrases, the emoji, kaomoji and symbol tables, the single-character whitelist additions, and `pinyin-overrides.txt`.

Inputs without a redistribution grant (`src/licensing.rs`) are left out unless `--include-unlicensed` is given; such a build is for local evaluation and must not be released.

## Changing the data

- Edit the files under `resources/dictionary-sources/` and rebuild.
- Base lexicons, custom words, translations and English words are changed in msime-dictionary. To take its new state, wait for msime-dictionary to publish a new `sources-vX.Y.Z` release, move every entry fetched from it in the lock to that release's assets (URL, size and SHA-256, which `SHA256SUMS.txt` in the release lists), set the `msime-dictionary` reference to the tagged commit, rebuild, and compare. Do not pin raw files at an untagged commit.
- Emoji, kaomoji and symbol keywords get their pinyin from the `pinyin` crate one character at a time. When a new polyphone keyword needs its phrase reading, add `keyword<TAB>item<TAB>item...` to `pinyin-overrides.txt`.
- To move a pinned input, change its URL, size and SHA-256 in the lock in the same commit, rebuild, and compare the result with the previous release before publishing.

## The `@` place table

`@` mode can offer Chinese administrative divisions after the user's own mention list. Unlike the dictionary artifacts, that table is small enough to ship inside the engine: `msime-dict-build places --cache <sources-cache>` reads `places/provinces.csv`, `places/cities.csv` and `places/areas.csv` from [modood/Administrative-divisions-of-China](https://github.com/modood/Administrative-divisions-of-China) (WTFPL, see `resources/licenses/Administrative-divisions-of-China-WTFPL.txt`) at the commit the lock pins, and rewrites `crates/engine/src/local/places.tsv`, which is committed. Place names whose per-character pinyin is wrong (重庆, 六安, 蚌埠, ...) are listed in `READINGS` in `src/places.rs`; the command fails when an entry there no longer matches any name. To take newer data, move the three `places/` entries in the lock to the new commit, rerun the command, and review the diff of `places.tsv`.

## The Korean Hanja table

Korean mode converts the composing syllable to Hanja from a table that also ships inside the engine: `msime-dict-build hanja --cache <sources-cache>` reads `ko/hanja.txt` from [libhangul](https://github.com/libhangul/libhangul) (`data/hanja/hanja.txt`, BSD-3-Clause, see `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`) at the commit the lock pins, and rewrites `crates/engine/src/korean/hanja.tsv`, which is committed. Only single-syllable readings are kept: one precomposed Hangul syllable mapped to one Hanja in CJK Unified Ideographs, Extension A, or one of the twelve unified ideographs of the CJK Compatibility Ideographs block (U+FA0E, U+FA11 and so on, which NFC leaves alone). True compatibility ideographs, characters outside the BMP and multi-character values are dropped, each syllable keeps the source's order, and the source's 훈음 comment becomes the third column. `scripts/test-korean-hanja-table.py` checks the committed table against these rules, and against the generator's output when the pinned source is in `target/dictionary-sources`. To take newer data, move the `ko/` entry in the lock to the new commit, rerun the command, and review the diff of `hanja.tsv`.

## 网页词库

网页内置输入法（`crates/engine-wasm`）不带完整的 `msime.db`，而是带从它裁出来的两个库，由 `web` 子命令生成：

```sh
cargo run --locked --release -p msime-dict-builder --bin msime-dict-build -- web --input <msime.db> --out-dir target/web-dict [--keep-multi 200000]
```

- `msime-pinyin.db`：全拼和双拼共用。保留全部单字表 `tbl_1_*`；多字表 `tbl_{2..7,others}_*` 合在一起按 `weight DESC, key, value, 表名, rowid` 排序，只保留前 `--keep-multi` 行（默认 200000，即评测里的 d200000）；`wubi86`、`wubi98` 和 `quick_parases` 清空。
- `msime-wubi86.db`：只保留 `wubi86`，全拼表、`wubi98` 和 `quick_parases` 清空。

两个库都保留输入库的全部表结构和索引，被清空的表查询时返回空结果而不是报错；随后 `ANALYZE`（只留 `sqlite_stat1`），再用 `VACUUM INTO` 写出不带空闲页的回滚日志模式数据库。输入只读打开，不会被修改。输出逐字节可复现：同一个输入跑两次得到相同的 sha256，`tests/web.rs` 会检查这一点。输入库里出现不认识的表时命令直接失败。

Releases are cut by the manually dispatched `.github/workflows/release-dictionary.yml`: it builds every artifact from the pinned sources without `--include-unlicensed`, checks `SHA256SUMS.txt`, and uploads the result as a workflow artifact; only with `publish` set does it create the `dict-vX.Y.Z` release on metasequoiaime/msime with the artifacts, `dictionary-manifest.json` and `SHA256SUMS.txt` as the builder wrote them. After a release is published, bump `resources/desktop-dictionary.lock.json` to the new files.

## Checking contributed words

msime-dictionary's CI gates changes to `custom/words.txt`, `custom/translations.txt` and `custom/english.txt` with `check-words`. Pass a base/head pair for each file to check (at least one); the words pair keeps its original `--base`/`--head` names:

```sh
msime-dict-build check-words \
  [--base <old words.txt> --head <new words.txt>] \
  [--translations-base <old translations.txt> --translations-head <new translations.txt>] \
  [--english-base <old english.txt> --english-head <new english.txt>] \
  [--msime-db <shipped msime.db>] [--english-db <shipped english.db>] \
  [--json report.json] [--markdown summary.md]
```

For every file a change may only append lines, and each appended entry goes through the parser the build uses and must not repeat another appended line or an entry already in that file. Per file:

- `words.txt`: word, `'`-separated quanpin that maps to a table, integer weight. The weight must lie within the range the existing entries use; with `--msime-db`, a row with the same word and pinyin in the shipped quanpin tables is rejected.
- `translations.txt`: `source<TAB>gloss`. A new gloss for an existing source is accepted (the build's last line wins), but the same source and gloss again is a duplicate.
- `english.txt`: lowercase ASCII word, non-empty display, integer weight of at least 1. An entry is its word and display; the weight must lie within the range the existing entries use; with `--english-db`, a word and display already in the shipped `english_words` is rejected.

Appended blank and `#` lines are skipped. The JSON report lists `added` and `rejected` lines, each with its `file` (for example `custom/english.txt`) and 1-based line number; added lines carry the parsed fields, rejected ones the text and reason. It also has `accepted`, a `files` array with each checked file's line counts and `weight_range`, and, at the top level as before, `words.txt`'s line counts and `weight_range` (null when `words.txt` was not checked). The Markdown summary is the same in table form, with an Added table per file, and is also printed to stderr. The exit status is 0 when nothing is rejected, 1 when anything is (the reports are still written), and 2 when the check cannot run: an unreadable file, a base file the build itself would reject, or a database without the expected table.
