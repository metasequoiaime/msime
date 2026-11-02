# 引擎运行时数据格式

这份文档规定 `msime-engine`（`crates/engine`）在运行时读写的每一个文件：放在哪个根目录下、文件格式（SQLite 的建表语句、二进制布局、文本格式）、引擎怎么检查版本、谁产出它、每个方案最少需要哪些文件，以及每个文件的数据许可证。它面向两类读者：要给引擎换一套数据或者自己产出数据的采用方，以及改动 `crates/dict-builder` 或引擎读端的人。

代码注释里的 `data-formats.md §N` 指的就是本文的章节号。章节号沿用移植时那份规格的编号，所以中间有空号；新增章节只往后加，不重排。建表语句均取自代码：SQLite 的 `sqlite_master` 保存的是原文，dict-builder 为了与旧流水线逐字节一致保留了原来的空白和注释，下文为了可读把它们排成多行，列名、类型、约束与代码一致。

**许可证。** 引擎代码目前按 GPL-3.0-only 发布，计划改为 MPL-2.0（见可嵌入引擎库设计笔记的 P2）；无论引擎用哪个许可证，数据都不随它改许可证，每个数据文件按它自己的许可证发布，见 [§21](#21-数据许可证)。采用方自己选择带哪些数据。

## 1. 运行时根目录与文件总表

### 1.1 四个根目录

会话的所有路径来自 `RuntimePaths`（`crates/engine/src/paths.rs`）的四个根：

| 字段 | 含义 | 引擎是否写入 |
| --- | --- | --- |
| `resources` | 随包、只读的资源目录，放一次数据发布的全部文件（[§2](#2-资源目录与只读布局)） | 不写 |
| `user_data` | 用户数据：学习日志 `msime_user.db`、学到的释义 `translation-glosses.db`、用户手写的 `custom_translations.txt`，以及 `dictionaries/` 下的代次目录 | 写 |
| `cache` | 可以随时清掉的缓存目录。引擎只负责建它，不在里面放必须保留的东西 | 只建目录 |
| `dictionaries` | 当前代次目录，即 `user_data/dictionaries/<content_id>`（[§3](#3-代次目录)）；只读布局下等于 `resources` | 写（学习回放、反查索引、n-gram 副本） |

四个根都必须是绝对路径（`RuntimePaths::validate`，否则 `Runtime directories must be absolute`）；`prepare_runtime_paths` 还要求 `resources`、`user_data`、`cache` 两两不重叠（`Resource, user-data and cache directories must not overlap`），因为清缓存不能删到用户数据，准备代次不能写进只读资源。路径在建会话时取定，之后改进程环境不会让活着的会话换目录。

`RuntimePaths::resource(name)` 和 `RuntimePaths::dictionary(name)` 在现名文件不存在、旧名文件存在时返回旧名路径（[§1.3](#13-旧文件名)）。空的根、空的名字或绝对路径的名字一律得到空路径，读端把空路径当作文件缺失。

### 1.2 文件总表

| 文件 | 所在根 | 格式 | 读端版本检查 | 谁需要 | 规格 |
| --- | --- | --- | --- | --- | --- |
| `msime-pinyin.db` | `resources`，工作副本在代次里 | SQLite | `metadata.format_version`（没有表按 0） | 全拼、双拼、五笔、九键 | [§4](#4-msime-pinyindb) |
| `msime-wubi.db` | `resources` | SQLite | `metadata.format_version`（没有表按 0） | 五笔 | [§9](#9-msime-wubidb) |
| `msime-english.db` | `resources`，工作副本在代次里 | SQLite | 写 `user_version=3`，读端不查 | 全拼、双拼、五笔（英文混输、九键英文、临时英文、回车学词）；其余方案只有专用英文用它，可缺 | [§5](#5-msime-englishdb) |
| `msime-bigram.bin`、`msime-trigram.bin` | `resources`，副本在代次里 | `MSNG` 二进制 | 魔数、版本、长度、键有序 | 整句打分，可缺 | [§7](#7-msime-bigrambin-与-msime-trigrambinmsng) |
| `msime-others.db` | `resources` | SQLite | `metadata.format_version`（没有表按 0） | emoji、颜文字、符号模式与目录，可缺 | [§12](#12-msime-othersdb) |
| `msime-japanese.dat` | `resources`，或 `SessionOptions::japanese_dictionary` | `MSJPDT1` 二进制 | 魔数、版本、全部边界、读音有序 | 日文，可缺（缺了只出假名） | [§8](#8-msime-japanesedatmsjpdt1) |
| `msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db` | 由 `SessionOptions` 的 `cantonese_dictionary`、`zhuyin_dictionary`、`stroke_dictionary` 给出 | SQLite | `metadata.format_version == "1"` | 对应方案，缺了激活失败 | [§13](#13-语言词库msime-cantonesedbmsime-zhuyindbmsime-strokedb) |
| `sentence-model.safetensors` | `resources` | safetensors | 由 `chinese-ime-lm` 校验几何，64 MiB 上限 | 神经重排，默认关 | [§15](#15-sentence-modelsafetensors) |
| octagram `.gram` | `SentenceAssociationOptions::collocation_model` 给出的路径 | Rime `Rime::Grammar/` 头加 Darts 双数组 | 只看格式前缀和数组边界，不验校验和 | 搭配打分，可缺 | [§16](#16-octagram-gram-语法模型) |
| `handwriting-zh_CN.model` | 宿主传给 `handwriting_recognize` 的路径 | Zinnia 二进制 SVM | 魔数异或文件大小、版本、无尾随字节 | 手写 | [§14](#14-zinnia-手写模型) |
| `helpcodes/*.txt`、`helpcodes/custom/*.txt` | `resources` | 文本 `字=码` | 无 | 辅助码 | [§17](#17-辅助码表) |
| `custom_translations.txt` | `user_data` 优先，否则 `resources`；宿主复制一份到代次 | 文本 `原文<TAB>释义` | 无 | 释义 | [§10](#10-custom_translationstxt) |
| `translation-glosses.db` | `user_data` | SQLite | 无 | 学到的释义 | [§11](#11-translation-glossesdb) |
| `msime_user.db` | `user_data` | SQLite（iOS 以外为 WAL） | `CREATE IF NOT EXISTS`，`user_version=4` 只写不查 | 学习、个人词典、个人 n-gram | [§6](#6-msime_userdb) |
| `.ready` | 代次目录 | 文本 | 必须是普通文件 | 代次完成标记 | [§3](#3-代次目录) |

### 1.3 旧文件名

统一 `msime-` 前缀之前的文件名仍然被认：现名缺席而旧名在场时 `RuntimePaths` 返回旧名（`assets::LEGACY_NAMES`）。对照：`msime-pinyin.db` ↔ `msime.db`，`msime-english.db` ↔ `english.db`，`msime-bigram.bin` ↔ `bigram.bin`，`msime-trigram.bin` ↔ `trigram.bin`，`msime-others.db` ↔ `others.db`，`msime-japanese.dat` ↔ `dict_japanese.dat`，`msime-mozc_dictionary_oss_README.txt` ↔ `mozc_dictionary_oss_README.txt`，`msime-dictionary-manifest.json` ↔ `dictionary-manifest.json`。新产出的数据一律用现名。

### 1.4 用户目录里的文件

`user_data` 下引擎会碰的文件只有三个，加上代次目录：

- `msime_user.db`：学习日志，学习数据的权威来源（[§6](#6-msime_userdb)）。
- `translation-glosses.db`：宿主经 `save_candidate_gloss` 写入的释义缓存，查释义时先读它（[§11](#11-translation-glossesdb)）。移植前的资源契约把这个文件叫 `gloss_cache.db`，但宿主写的、用户磁盘上有的一直是 `translation-glosses.db`，引擎只认后者；`gloss_cache.db` 这个名字不再使用。
- `custom_translations.txt`：用户手写的释义（[§10](#10-custom_translationstxt)）。
- `dictionaries/<content_id>/`：代次目录（[§3](#3-代次目录)）。

日志不是数据库时（例如被覆盖成全零），准备代次之前会把它连同 `-wal`、`-shm`、`-journal` 改名为 `msime_user.db.corrupt-<unix 秒>` 挪到旁边，从空日志开始（`quarantine_corrupt_journal`）。

## 2. 资源目录与只读布局

`resources` 放一次数据发布的文件，引擎只读不写。打开数据库时不跟随末级符号链接（`SQLITE_OPEN_NOFOLLOW`），只读打开的文件在所在目录只有 root 或当前用户能写时允许多个硬链接（Nix store 去重、ostree 部署）；二进制模型经 `open_file_no_follow` 打开，必须是普通文件。

**只读布局**是 `dictionaries == resources` 的 `RuntimePaths`：不调用 `prepare_runtime_paths`，引擎直接在资源目录里读主库。网页引擎（`crates/engine-wasm`，主库放在 `/res`）和后端（`backend::execute`）这样用。这种布局下：

- 没有代次准备这一步，所以建会话时（`Session::new`）检查 `msime-pinyin.db`、`msime-wubi.db` 的文件内版本（[§4.3](#43-文件内版本)），比引擎新的格式拒绝建会话。后端不建会话，在每次请求前自己查这两个文件和 `msime-others.db`，比引擎新的格式让 `convert` 以外的请求都报 `resources_unavailable`（资源目录整体与这个后端不匹配）。
- 五笔从资源目录的 `msime-wubi.db` 读（`ime::registry::wubi_database` 依次找代次的 `msime-wubi.db`、代次的 `msime-pinyin.db`、资源的 `msime-wubi.db`，最后退回资源的 `msime-pinyin.db`）；五笔的 14 键排序另读 `msime-pinyin.db` 的词频。
- 学习、删词和个人词典编辑写的是 `dictionaries` 下的文件，在只读布局下就是资源文件本身；引擎不拦这些写入，所以只读布局要么用可写、可丢弃的资源副本（网页引擎的内存 VFS），要么由宿主不开学习。重置学习数据拒绝在只读布局上执行（`Cannot reset packaged dictionaries in place`），合并下载的五笔库也跳过。
- 日志仍在 `user_data/msime_user.db`。

## 3. 代次目录

`prepare_runtime_paths(resources, user_data, cache, content_id)`（`crates/engine/src/user_dictionary/generation.rs`）准备一个代次，返回 `dictionaries = user_data/dictionaries/<content_id>` 的 `RuntimePaths`。`prepare_runtime_paths_for` 多一个 `SchemeSet`，资源目录只要有这些方案读的词库：方案集合不读主库时（`SchemeSet::reads_main_dictionary` 为假，即不含全拼、双拼、五笔），资源目录不必有 `msime-pinyin.db`，代次里也没有它；这样的集合也不要求 `msime-english.db`（`SchemeSet::requires_english_dictionary` 为假），资源目录带了就复制进代次、日志只回放英文行，没带时代次里两个库都没有、日志一行也不回放（记录留在日志里）。代次里有没有不要求的英文库，以准备它时资源目录里有没有为准；之后才放进资源目录的英文库要换一个 `content_id` 才进代次。`reset_learned_data_for(paths, schemes)` 按同样的规则换回词库。

**`content_id`** 是 1 到 128 个 `[0-9A-Za-z_-]`，由宿主给出，通常取自数据发布的版本或摘要；同一个 id 对应同一份资源内容，换了资源必须换 id。

**目录布局。**

```text
user_data/
  msime_user.db
  dictionaries/
    <content_id>/                      完成的代次
      msime-pinyin.db                  主库工作副本（不读主库的方案集合没有它）
      msime-english.db                 英文库工作副本（不要求它的方案集合在资源目录没带它时没有）
      msime-bigram.bin                 n-gram 表副本（资源里有才有）
      msime-trigram.bin
      custom_translations.txt          宿主复制的释义 sidecar（可选）
      .ready                           完成标记，内容是 "<content_id>\n"
    <content_id>.<随机串>.incoming/    正在准备的暂存目录
    <content_id>.<随机串>.removing/    被认领、正在删除的暂存目录
```

**准备一个新代次**的顺序：

1. 校验 id 和三个根，建 `user_data` 与 `cache`，恢复中途被杀的重置留下的备份，把不是数据库的日志挪开。
2. 清扫被丢下的暂存目录：`<id>.<字母数字>.incoming`（以及旧版本固定名字的 `<id>.incoming`）本身和里面每一项都超过一小时没被修改，才先改名为 `.removing` 认领再删；已经是 `.removing` 的直接删。
3. 代次目录里已有普通文件 `.ready` 时直接打开它（见下文）；`.ready` 是目录、设备或符号链接时报 `Incomplete runtime dictionary generation`。
4. 否则在 `tempdir` 新建的 `<id>.<随机串>.incoming` 里暂存：检查资源 `msime-pinyin.db` 的文件内版本，经 SQLite backup API 复制 `msime-pinyin.db` 与 `msime-english.db`（按方案集合，见上文；backup 会带上已提交的 WAL 内容），把资源里的 `msime-wubi.db` 的 `wubi86`、`wubi98` 连同索引按 rowid 并进主库副本（先检查 `msime-wubi.db` 的文件内版本），复制 n-gram 表，回放日志，补建五笔反查索引，最后写 `.ready`（内容 `<content_id>\n`，独占新建、`fsync`），再把整个暂存目录改名为 `<content_id>`。任何一步失败只删这次自己的暂存目录。
5. 改名失败而同名代次已经就绪（另一个进程抢先发布）时，改用那一份。

**打开已就绪的代次**：放回重置途中被杀时挪开的词库，确认方案集合要求的词库副本都是普通文件，五笔表不在时并入，重新回放当前日志（在新代次上学到的东西切回旧代次时不丢），补反查索引，补上后来才随资源发布的 n-gram 表（已有的不替换，会话可能正映射着它）。

**n-gram 表的复制**：每张表先写进本次独有的 `<表名>.<随机串>.incoming`（独占新建），`fsync` 后改名就位，从不就地改写，因为会话以只读内存映射读它。一小时没被修改的 `<表名>.incoming`、`<表名>.<字母数字>.incoming` 在下次准备时删掉。

**工作副本**是用户可写的持久数据，不是缓存：学习、删词、个人词典编辑直接写它，日志同时记一份；换代次时由日志回放补齐。工作副本保持回滚日志模式（不切 WAL），因为重置学习数据把它们挪作备份时不带旁路文件。

**五笔反查索引**：`CREATE INDEX IF NOT EXISTS idx_wubi86_value ON wubi86("value")` 和 `idx_wubi98_value`，只在代次副本里补建；失败只记日志，反查照常正确，只是慢。

## 4. `msime-pinyin.db`

主词库：全拼各表、快捷短语表 `quick_parases`（拼写如此，是冻结的表名）和文件内版本表。全拼、双拼、九键和五笔的 14 键排序都读它；准备代次时五笔码表并进它的工作副本（[§9](#9-msime-wubidb)）。发布的文件是回滚日志模式（`journal_mode=DELETE`）、没有空闲页、只带 `sqlite_stat1` 统计表的独立数据库。

### 4.1 拼音表

表名规则在 `crates/engine/src/format.rs`：`tbl_<n>_<首字母>`，`n` 是音节数 1 到 7，超过 7 个音节进 `tbl_others_<首字母>`；首字母取第一个音节的第一个字母，只有 `abcdefghjklmnopqrstwxyz` 这 23 个字母有表（`i`、`u`、`v` 不开头任何音节）。一共 8 × 23 = 184 张表。读端把不存在的表当作没有行。

每张表（以 `tbl_2_n` 为例）：

```sql
create table if not exists tbl_2_n (
   "key" text, -- 全拼拼音
   "jp" text, -- 全拼简拼
   "value" text, -- 对应的汉字或者词组
   "weight" integer default 0 -- 权重
);
create index idx_key_2_n on tbl_2_n(key);
create index idx_jp_2_n on tbl_2_n(jp);
```

- `key`：小写全拼音节，音节间用 `'` 分隔，`ü` 写作 `v`（`lv`、`nve`），例如 `ni'hao`。
- `jp`：每个音节的首字母连在一起，例如 `nh`。
- `value`：词。
- `weight`：非负整数，越大越靠前；引擎按 `i64` 读。

同一表里同一 `(key, value)` 只有一行（构建时去重，保留较高权重），但表上没有唯一约束，工作副本里的学习写入按「先更新、没有行再插入」维护这一点。

### 4.2 快捷短语表

```sql
CREATE TABLE quick_parases (
    "key" TEXT NOT NULL,
    "value" TEXT NOT NULL,
    "weight" INTEGER NOT NULL DEFAULT 0,
    UNIQUE("key", "value")
);
CREATE INDEX idx_quick_parases_key_weight ON quick_parases("key", "weight" DESC);
```

`key` 是小写 ASCII 字母码，`value` 是展开的文本。

### 4.3 文件内版本

`msime-pinyin.db`、`msime-wubi.db`、`msime-others.db` 各带一张与语言词库同形的表（`format::METADATA_SCHEMA`）：

```sql
CREATE TABLE IF NOT EXISTS metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
INSERT OR REPLACE INTO metadata(name, value) VALUES('format_version', '1');
```

`format_version` 是十进制文本，等于 `format::FORMAT_VERSION`，也就是发布清单的 `format_version` 与 `engine_compatibility.dictionary_format`。dict-builder 在拆出五笔库之后、冻结数据库之前写入（`product::stamp_format_versions`），产品检查用引擎的读端核对三个文件都是当前版本。

读端规则（`format::check_format_version`）：

| 文件里 | 引擎的处理 |
| --- | --- |
| 没有 `metadata` 表 | 版本 0（`LEGACY_FORMAT_VERSION`），照常读。`dict-v2.0.39` 及以前的发布都是这样 |
| `format_version` 是 0 到 `FORMAT_VERSION` 之间的整数 | 照常读 |
| 更大的整数、不是整数、没有 `format_version` 行、或 `metadata` 的列不对 | 拒绝，报 `Dictionary format version unsupported: <文件名> ..., this engine reads format_version <N> or older`，错误里只有文件名，没有路径 |
| 连 `sqlite_master` 都读不出（不是数据库、被锁），或读 `metadata` 时被锁、读写出错、文件损坏 | 版本检查不报错，交给原来的读写步骤按「读不了」处理 |

检查发生在：准备代次复制主库之前、重新打开已经就绪的代次时（查代次里的主库副本，它带着来源文件的 `metadata`；更新的引擎准备的代次在降级后不会被照旧格式读）、合并 `msime-wubi.db` 之前（包括合并宿主下载的五笔库），只读布局下建会话时（主库与五笔库），后端每次请求前（主库、五笔库与 `msime-others.db`），以及打开 `msime-others.db` 时。符号与 emoji 目录接口（`read_emoji_catalog_slice`、`emoji_catalog_groups`、`emoji_symbol_groups`）把版本错误原样返回；按键路径上的本地模式查询（emoji、颜文字、快捷短语、超级简拼）没有报告别的错误的出口，版本不支持的文件按读不了处理，出它们原有的「不可用」诊断。

**改格式的规矩。** 读端不认识的改动（换列、改列的含义、改表名规则）必须把 `FORMAT_VERSION` 加一，新文件写新版本，引擎同一个提交里学会读新版本；只追加读端会忽略的东西（新表、新索引）不必加版本。旧引擎遇到新版本的文件会清楚地拒绝，而不是误读。

### 4.4 工作副本里的变化

代次里的 `msime-pinyin.db` 是发布文件的副本加上：

- 并入的 `wubi86`、`wubi98` 及其索引（[§9](#9-msime-wubidb)），外加反查索引 `idx_wubi86_value`、`idx_wubi98_value`；
- 日志回放写进来的行：用户自造词、调频后的权重、删除（[§6.2](#62-user_dictionary_operations)）。

`metadata` 随 backup 原样进副本，引擎不改它。

## 5. `msime-english.db`

英文词表和中英互译释义。方案集合含全拼、双拼或五笔时每个代次都复制一份，资源目录必须有它；只用日文、韩语、越南语、藏文、粤语、注音、笔画的集合不要求它，带了才复制进代次，供专用英文用（见 [§20](#20-每个方案的最小数据集)）。

```sql
CREATE TABLE english_words (
    word TEXT COLLATE BINARY NOT NULL,
    display TEXT NOT NULL,
    weight INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (word, display)
) WITHOUT ROWID;
CREATE TABLE en_zh_glosses (
    english TEXT COLLATE BINARY PRIMARY KEY,
    chinese_gloss TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE zh_en_glosses (
    chinese TEXT COLLATE BINARY PRIMARY KEY,
    english_gloss TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE source_notices (source TEXT NOT NULL PRIMARY KEY, notice TEXT NOT NULL) WITHOUT ROWID;
PRAGMA user_version=3;
```

- `english_words.word` 是小写查找键，`display` 是展示的大小写形式（`china` 与 `China` 各一行），`weight` 可能超过 `i32`，引擎按 `i64` 读。同一个词的几种大小写里，领头的那种带词频，其余比它低 1。
- 释义表的值可以有多个义项，宿主门面在候选旁显示时只取前两个。
- `source_notices` 保存来源要求随数据保留的声明（目前是 SCOWL 的版权声明），引擎不读。
- `user_version=3` 由构建和 `ensure_english_schema` 写入，读端不检查；`ensure_english_schema` 会把没有 `weight` 列或只以 `word` 为键的旧表就地改建成上面的形状。

工作副本里，日志的英文行（[§6.2](#62-user_dictionary_operations) 的 `english`）回放成 `english_words` 的插入、改权重或删除。

## 6. `msime_user.db`

学习日志，放在 `user_data`，是用户学到的一切的权威来源：工作副本可以随时由它重建。iOS 以外的连接把它切到 WAL、`synchronous=NORMAL`、`journal_size_limit=4194304`（iOS 的 App 与键盘扩展共用 App Group 容器，WAL 的共享内存锁会让挂起的进程被系统杀掉，所以保持回滚日志）。

### 6.1 模式与版本

每次打开都执行全部 `CREATE TABLE IF NOT EXISTS`（`user_dictionary::journal::SCHEMA_SQL`），检查 `user_dictionary_operations` 有没有 `user_inserted` 列、没有就 `ALTER TABLE ... ADD COLUMN`，最后写 `PRAGMA user_version=4`。版本号只写不读：老引擎写回自己的版本号也只是忽略它不认识的表，新旧引擎可以交替打开同一个日志。现有用户多是只有前四张表的 v3 日志。一共 13 张表。

### 6.2 `user_dictionary_operations`

```sql
CREATE TABLE IF NOT EXISTS user_dictionary_operations(
    dictionary TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    operation TEXT NOT NULL CHECK(operation IN ('upsert','delete')),
    weight INTEGER NOT NULL DEFAULT 0,
    display TEXT NOT NULL DEFAULT '',
    user_inserted INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()),
    PRIMARY KEY(dictionary,key,value));
```

`dictionary` 的取值冻结，回放时决定写到哪张表：

| `dictionary` | 回放目标 | `key` |
| --- | --- | --- |
| `pinyin` | 主库的全拼表（按 [§4.1](#41-拼音表) 的规则选表，同时写 `jp`） | `'` 分隔的全拼 |
| `wubi` | 主库的 `wubi86` | 1 到 4 个 `a`-`y` |
| `wubi98` | 主库的 `wubi98` | 同上 |
| `quick` | 主库的 `quick_parases` | 1 到 32 个字母或数字 |
| `english` | 英文库的 `english_words`，`display` 列是展示形式 | 1 到 64 个字母 |

`operation='delete'` 是墓碑，回放时删除对应行；`user_inserted=1` 表示用户自己加的词（删除后仍记着它曾是用户的）。权重低于 1 的 `pinyin` upsert 是旧版调频留下的，回放时跳过，`ensure_user_database` 会把它们删掉。代次里没有对应表的行（只读英文库的代次里的非英文行，还没有五笔表时的五笔行）计为跳过，留在日志里。**未知的 `dictionary` 值和存不进去的 `key` 算失败行，任何一行失败整次回放回滚、代次准备被拒**，所以新增取值必须先让引擎认得它，再往日志里写。

### 6.3 其余表

```sql
CREATE TABLE IF NOT EXISTS personal_dictionary_receipts( request_id TEXT PRIMARY KEY, payload TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS candidate_selection_state(
    context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,
    selection_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(context_key,entry_key,value));
CREATE TABLE IF NOT EXISTS fixed_candidate_positions(
    context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,
    position INTEGER NOT NULL CHECK(position BETWEEN 1 AND 5),
    PRIMARY KEY(context_key,entry_key,value),UNIQUE(context_key,position));
CREATE TABLE IF NOT EXISTS pick_transitions(
    previous_key TEXT NOT NULL,previous_value TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()),
    PRIMARY KEY(previous_key,previous_value,key,value));
CREATE INDEX IF NOT EXISTS pick_transitions_updated_at ON pick_transitions(updated_at);
CREATE TABLE IF NOT EXISTS pinyin_typo_counts(
    typed TEXT NOT NULL,intended TEXT NOT NULL,
    accepted INTEGER NOT NULL CHECK(accepted BETWEEN 0 AND 1000),
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()),
    PRIMARY KEY(typed,intended));
CREATE TABLE IF NOT EXISTS pinyin_autocorrect_suppressions(
    input TEXT PRIMARY KEY,
    commits INTEGER NOT NULL CHECK(commits >= 1),
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()));
CREATE TABLE IF NOT EXISTS shuangpin_typo_counts(
    profile TEXT NOT NULL,typed TEXT NOT NULL,intended TEXT NOT NULL,
    accepted INTEGER NOT NULL DEFAULT 0 CHECK(accepted BETWEEN 0 AND 1000),
    rejected INTEGER NOT NULL DEFAULT 0 CHECK(rejected BETWEEN 0 AND 1000),
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()),
    PRIMARY KEY(profile,typed,intended));
CREATE TABLE IF NOT EXISTS shuangpin_autocorrect_suppressions(
    profile TEXT NOT NULL,input TEXT NOT NULL,
    commits INTEGER NOT NULL CHECK(commits >= 1),
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()),
    PRIMARY KEY(profile,input));
CREATE TABLE IF NOT EXISTS pinned_candidates(
    context_key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT(unixepoch()));
CREATE TABLE IF NOT EXISTS personal_bigram(
    previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),
    PRIMARY KEY(previous,word)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS personal_bigram_word ON personal_bigram(word);
CREATE TABLE IF NOT EXISTS personal_trigram(
    earlier TEXT NOT NULL,previous TEXT NOT NULL,word TEXT NOT NULL,
    count INTEGER NOT NULL CHECK(count>0),
    PRIMARY KEY(earlier,previous,word)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS personal_trigram_word ON personal_trigram(word);
CREATE TABLE IF NOT EXISTS quick_phrase_slots(
    code TEXT PRIMARY KEY,slot INTEGER NOT NULL CHECK(slot>=0));
```

| 表 | 内容 |
| --- | --- |
| `personal_dictionary_receipts` | 个人词典编辑请求的回执，按 `request_id` 去重 |
| `candidate_selection_state` | 调频的选中计数 |
| `fixed_candidate_positions` | 固定位置（1 到 5） |
| `pick_transitions` | 连续选中的两个词，够多次就成词 |
| `pinyin_typo_counts`、`pinyin_autocorrect_suppressions` | 全拼纠错的学习与按输入关闭纠错 |
| `shuangpin_typo_counts`、`shuangpin_autocorrect_suppressions` | 双拼的同上，按 `profile` 区分 |
| `pinned_candidates` | 置顶的候选 |
| `personal_bigram`、`personal_trigram` | 个人 n-gram 计数 |
| `quick_phrase_slots` | 快捷短语的位置 |

## 7. `msime-bigram.bin` 与 `msime-trigram.bin`（MSNG）

整句词格的二元、三元加成表。小端序，布局：

| 偏移 | 长度 | 内容 |
| --- | --- | --- |
| 0 | 4 | 魔数 `MSNG` |
| 4 | 4 | `u32` 版本，必须是 1 |
| 8 | 4 | `u32` 条目数 `count`，不超过 40,000,000 |
| 12 | 4 | 保留，写 0 |
| 16 | `8 × count` | `u64` 键，升序 |
| `16 + 8 × count` | `4 × count` | `f32` 值，与键一一对应 |

- 键是把词按 UTF-8 字节、词与词之间插一个 `0x00` 字节拼起来后的 64 位 FNV-1a（偏移基 `0xCBF29CE484222325`，质数 `0x100000001B3`）。二元表的键是 `[前词, 后词]`，三元表是 `[前前词, 前词, 后词]`；句首用 `U+0001` 作为一个「词」。
- 值是对数加成：二元 `ln(P(后|前) / P(后))`，三元 `ln(P(后|前前,前) / P(后|前))`，按 `count / (count + 5)` 收缩并截到 ±3。查不到的键贡献 0，所以两张表可以直接加到一元分上。
- 读端只映射 `16 + 12 × count` 字节，允许文件尾部有多余字节；文件短于这个长度、魔数或版本不对、键不是非降序时整张表当作没有，词格照常工作。
- 文件只读映射，所以只能以改名替换，绝不能就地改写（[§3](#3-代次目录)）。

发布的两张表各 1,000,000 条。

## 8. `msime-japanese.dat`（MSJPDT1）

日文整句解码器的词条、连接矩阵与字符串，从 Mozc 开源词库打包而来。小端序，各字段按偏移非对齐读取。

**文件头（56 字节）：**

| 偏移 | 类型 | 内容 |
| --- | --- | --- |
| 0 | 8 字节 | 魔数 `MSJPDT1\0` |
| 8 | `u32` | 版本，必须是 1 |
| 12 | `u32` | 词条数 `token_count`，1 到 2,000,000 |
| 16 | `u32` | 连接矩阵边长 `size`，非 0，`size²` 不超过 20,000,000 |
| 20 | `u32` | 保留，写 0 |
| 24 | `u64` | 词条区偏移 |
| 32 | `u64` | 连接矩阵偏移 |
| 40 | `u64` | 字符串区偏移 |
| 48 | `u64` | 字符串区字节数，不超过 2³² |

**词条（每条 20 字节）：** 读音偏移 `u32`、读音长度 `u16`、表层偏移 `u32`、表层长度 `u16`、左上下文 id `u16`、右上下文 id `u16`、词条成本 `i32`。偏移相对字符串区起点。

**连接矩阵：** `size × size` 个 `i16`，下标 `右id × size + 左id`。

**字符串区：** UTF-8，相同的串只存一次。

读端检查：三个区都落在文件内且不早于文件头末尾；每个词条的读音、表层在字符串区内且落在字符边界上；上下文 id 小于 `size`；词条按读音升序（文件顺序就是查找索引）。任何一项不满足整个文件当作没有，日文只出假名行。文件只读映射，替换只能改名。

网页引擎用的是 `msime-dict-build web --japanese` 裁出的小模型，格式相同。

## 9. `msime-wubi.db`

单独发布的只读五笔码表，从拼音构建产物里拆出来：

```sql
CREATE TABLE wubi86 ("key" TEXT NOT NULL, "value" TEXT NOT NULL, "weight" INTEGER NOT NULL DEFAULT 0, UNIQUE("key", "value"));
CREATE INDEX idx_wubi86_key_weight ON wubi86("key", "weight" DESC);
CREATE TABLE wubi98 ("key" TEXT NOT NULL, "value" TEXT NOT NULL, "weight" INTEGER NOT NULL DEFAULT 0, UNIQUE("key", "value"));
CREATE INDEX idx_wubi98_key_weight ON wubi98("key", "weight" DESC);
CREATE TABLE IF NOT EXISTS metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
```

- `key` 是 1 到 4 个 `a`-`y` 字母的编码，`value` 是字或词，`weight` 越大越靠前。
- 行的 rowid 顺序有意义：同码同权重的候选按 rowid 排，反查编码也以 rowid 作最后的排序键，所以拆分和合并都按 rowid 复制。
- 只收基本区汉字，CJK 扩展区和私用区字符不进表。
- `metadata` 见 [§4.3](#43-文件内版本)；合并进主库时只搬 `wubi86`、`wubi98`，不搬 `metadata`。

准备代次时这两张表并进工作主库，五笔查询、学习和个人词典都在工作主库上进行。旧的合并发布（五笔表还在 `msime-pinyin.db` 里）照常可用。

## 10. `custom_translations.txt`

用户手写的释义，优先于数据库里的释义。UTF-8，可带 BOM，任何换行风格，上限 1 MiB（超了当作没有）。每行去掉首尾空白后：空行和 `#` 开头的行跳过；其余按第一个制表符分成 `原文<TAB>释义`，两边都不能为空。原文含任何非 ASCII 字节就算中文（进中译英表），否则算英文（进英译中表）。同一个原文后出现的行覆盖前面的。

宿主在建会话前把 `user_data/custom_translations.txt`（没有就用资源目录里的）复制到代次目录（`host::options::prepare_translation_sidecar`）。会话的英文混输读资源目录那份；宿主门面的候选释义直接读 `user_data` 里那份。

## 11. `translation-glosses.db`

宿主保存的释义缓存，放在 `user_data`，表结构与 `msime-english.db` 相同（`ensure_english_schema` 建表，同样写 `user_version=3`），实际只用两张释义表，以 `INSERT OR REPLACE` 写入。查释义的顺序是：`custom_translations.txt`，这个文件，最后是 `msime-english.db`。它是用户文件而不是代次文件，因为代次副本每换一次代次就被替换。

## 12. `msime-others.db`

emoji、颜文字和符号目录，emoji 面板与 E、M 等本地模式读它，只读，可缺（缺了这些模式出「不可用」诊断）。

```sql
CREATE TABLE emoji (
    emoji TEXT NOT NULL,
    category TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    keywords TEXT NOT NULL,
    pinyin TEXT NOT NULL,
    PRIMARY KEY (emoji)
) WITHOUT ROWID;
CREATE INDEX idx_emoji_category_order ON emoji(category, sort_order);
CREATE TABLE emoji_pinyin (
    key TEXT NOT NULL,
    emoji TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    PRIMARY KEY (key, emoji)
) WITHOUT ROWID;
CREATE TABLE kaomoji (
    pinyin TEXT NOT NULL,
    jianpin TEXT NOT NULL,
    kaomoji TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    PRIMARY KEY (pinyin, jianpin, kaomoji)
) WITHOUT ROWID;
CREATE INDEX idx_kaomoji_jianpin ON kaomoji(jianpin);
CREATE TABLE kaomoji_catalog (
    kaomoji TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    keywords TEXT NOT NULL,
    PRIMARY KEY (kaomoji)
) WITHOUT ROWID;
CREATE TABLE symbol_catalog (
    symbol TEXT NOT NULL,
    category TEXT NOT NULL,
    parent_category TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    keywords TEXT NOT NULL,
    PRIMARY KEY (symbol, category)
) WITHOUT ROWID;
CREATE INDEX idx_symbol_catalog_parent_order ON symbol_catalog(parent_category, sort_order);
CREATE TABLE IF NOT EXISTS metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
```

- `emoji.keywords` 是空格分隔的中英文关键词，`emoji.pinyin` 是其中中文关键词的连写全拼，空格分隔。
- `emoji_pinyin` 每个搜索键一行：中文关键词的连写全拼和首字母简拼，英文关键词拆出的小写单词；输入的码按前缀匹配这些键。
- `kaomoji.pinyin`、`jianpin` 是关键词的全拼和简拼（英文关键词只有 `pinyin`，`jianpin` 为空）；`kaomoji_catalog` 是面板浏览用的目录，`keywords` 空格分隔。
- `sort_order` 是目录里的展示顺序。
- `metadata` 见 [§4.3](#43-文件内版本)。

## 13. 语言词库（`msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db`）

粤语、注音、笔画方案各自的只读词库，随资源集另行发布，只在对应方案激活时打开；路径由宿主经 `SessionOptions` 给出。结构由 `crates/engine/src/language_dictionary.rs` 的 `SCHEMA` 唯一定义：

```sql
CREATE TABLE metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE syllables(syllable TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE entries(key TEXT NOT NULL, text TEXT NOT NULL, weight INTEGER NOT NULL, PRIMARY KEY(key, text)) WITHOUT ROWID;
CREATE INDEX entries_by_key_weight ON entries(key, weight DESC);
```

- `metadata` 有三行：`format_version`（必须恰好是 `"1"`，否则 `Language dictionary version unsupported`）、`source_commit`（源数据的上游提交）、`license`（库里数据所有来源的许可证合成的 SPDX 表达式，每个来源一项：粤语 `CC-BY-4.0`，注音 `LGPL-2.1-or-later AND BSD-3-Clause AND MIT`，笔画 `LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0`；`dict-v2.0.40` 及以前发布的注音与笔画库还是构建器改正前写的 `LGPL-2.1-or-later`、`LGPL-3.0-only AND GPL-3.0-only`，`dict-v2.0.41` 起是新值）。
- `syllables` 是方案的音节表；笔画库里是出现过的笔画码。
- `entries.key` 是音节以单个空格连接（`nei hou`）；笔画库里是 `hspnz` 五个字母组成的笔画码，没有空格。
- 文件缺失、打不开或不是数据库报 `Language dictionary unavailable`，方案激活失败。

## 14. Zinnia 手写模型

`handwriting-zh_CN.model`（Tegaki zh_CN 0.3）是 Zinnia 的二进制模型，每个字一个线性一对多 SVM，引擎原样读（移植自 Zinnia `581faa8f`，BSD-3-Clause）。小端序：

| 内容 | 类型 |
| --- | --- |
| 魔数：`0x0EF71821` 异或文件字节数 | `u32` |
| 版本，必须是 1 | `u32` |
| 类数 | `u32` |
| 每类：标签（UTF-8，补 `0x00` 到 16 字节）、偏置（`f32`）、若干 `(特征下标 i32, 权重 f32)` 对，以下标 `-1` 的一对结束 | |

文件必须恰好在最后一类的结束对之后结束，否则拒绝。模型只读映射。

## 15. `sentence-model.safetensors`

键盘神经重排模型，由 `chinese-ime-lm` 0.2 读取（`SentenceModel::load`）。文件超过 64 MiB 不读。格式是 safetensors：8 字节小端 `u64` 头长度，JSON 头，张量数据。

- `__metadata__` 必须有 `config`（JSON 字符串，字段 `vocab`、`n_layer`、`n_head`、`n_embd`、`context`）和 `vocab`（JSON 字符串数组，长度等于 `config.vocab`，单字符的项按字索引，`<pad>` 一类保留项按编号引用）；可选 `attribution`、`license`。发布的模型 `license` 为 `Apache-2.0`，`attribution` 记训练语料，再分发时必须保留整个 `__metadata__`。
- 张量：`tok.weight`、`pos.weight`、`ln_f.weight`、`ln_f.bias`，以及每层 `blocks.<i>.` 下的 `ln1.weight`、`ln1.bias`、`qkv.weight`、`qkv.bias`、`proj.weight`、`proj.bias`、`ln2.weight`、`ln2.bias`、`fc.weight`、`fc.bias`、`out.weight`、`out.bias`。数据类型 `F32`、`F16`，或 `I8` 加同名 `.scale`（每行一个 `f32`）。形状由 `config` 推出并逐一校验。

加载失败、文件缺失或过大都只是没有神经候选，不影响会话。

## 16. octagram `.gram` 语法模型

Rime 上游的 octagram 语法模型格式（[lotem/librime-octagram](https://github.com/lotem/librime-octagram) `57d18b9f`，BSD-3-Clause），引擎只读，不产出这种文件。小端序：

| 偏移 | 内容 |
| --- | --- |
| 0 | 32 字节格式串，以 `Rime::Grammar/` 开头 |
| 32 | `u32` 校验和，引擎不验 |
| 36 | `u32` 双数组单元数（单位个数，不是字节） |
| 40 | `i32` 偏移，相对这个字段自身的位置指向双数组 |
| 之后 | Darts 双数组，每单元 4 字节 |

键是「上下文尾部字 + 词首字」经 octagram 的 `encode` 压缩的字节串（ASCII 一字节，`U+4000`-`U+9FFF` 两字节，其余变长），值是 `int(ln(共现频次) × 10000)`。句尾项是「词 + `$`」。只校验格式前缀、偏移非负、单元数不超过 400,000,000 且数组在文件内；不合格时没有搭配打分。文件以只读映射加载，替换只能改名。

## 17. 辅助码表

内置方案和文件（`assets::HELPCODES`，相对 `resources`）：`lantian` → `helpcodes/helpcode.txt`，`ziranma` → `helpcodes/zrm_helpcode_big_unique.txt`，`shouyou2_0` → `helpcodes/shouyou2_0_helpcode.txt`，`shouyouplus` → `helpcodes/shouyouplus_helpcode.txt`，`xiaohe` → `helpcodes/xiaohe_helpcode.txt`，`jiajia` → `helpcodes/jiajia_helpcode.txt`，`wubi86` → `helpcodes/wubi86_helpcode.txt`。自定义方案 `custom/<名字>` 读 `helpcodes/custom/<名字>.txt`（扩展名大小写不限，名字不能含路径分隔符、不能以 `.` 开头）。

格式：UTF-8，首行可带 BOM，按 `\n` 分行并去掉行尾 `\r`；`#` 开头的行跳过；每行在第一个 `=` 处分成 `字=码`，`字` 不能为空，码取 `=` 后的头一到两个字节且必须都是 `a`-`z`，否则跳过这行；同一个字后面的行覆盖前面的。文件上限 1 MiB。内置表缺失或读不了时是空表（会话照常建立），自定义表缺失报 `Unknown helpcode schema`。

## 18. 发布清单

主构建写 `msime-dictionary-manifest.json` 和 `msime-SHA256SUMS.txt`。清单字段：`manifest_version`（1）、`profile`（`desktop`）、`format_version` 与 `engine_compatibility.dictionary_format`（都等于 `format::FORMAT_VERSION`）、`engine_compatibility.japanese_model_magic`（`MSJPDT1`）、`source`（构建所在的 msime 提交与是否有未提交改动）、`sqlite_journal_mode`（`delete`）、`format_contract_commit`、`custom_dictionary_commit`、`custom_dictionary_repository`、`custom_dictionary_path`、`references`（各上游的仓库与提交）、`mozc_revision`、`features`、`licensing`（是否含无授权输入、排除了哪些）、`files`（每个产物的 `sha256` 与 `size`）。客户端的词库更新只接受 `dictionary_format` 等于自己的 `FORMAT_VERSION` 且日文魔数为 `MSJPDT1` 的清单。引擎本身不读清单。

## 19. 产出方

| 产物 | 产出方 |
| --- | --- |
| `msime-pinyin.db`、`msime-wubi.db`、`msime-english.db`、`msime-others.db`、`msime-japanese.dat`、`msime-bigram.bin`、`msime-trigram.bin`，以及 `msime-mozc_dictionary_oss_README.txt`、`msime-mozc_LICENSE.txt`、`msime-scowl_Copyright.txt`、清单与 `msime-SHA256SUMS.txt` | `msime-dict-build` 主构建（不带子命令，阶段见 `msime-dict-build --list`），输入是 `resources/dictionary-sources.lock.json` 固定的第三方文件和 msime-dictionary 的检出（`--dictionary`）。发布由 [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 的 `release-built-dictionaries.yml` 构建并以 `dict-vX.Y.Z` 发布，本仓库经 `resources/desktop-dictionary.lock.json` 固定。用法见 `crates/dict-builder/README.md` |
| `msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db` 及其许可证文本、`msime-language-dictionaries-SHA256SUMS` | `msime-dict-build languages`，同一个 `dict-v*` 发布，经 `resources/language-dictionaries.lock.json` 固定 |
| 网页引擎的 `msime-pinyin.db`、`msime-wubi86.db`、小号 `msime-japanese.dat` | `msime-dict-build web`，从 `dict-v*` 发布的文件裁出；保留拼音库的 `metadata`，`msime-wubi.db` 的 `metadata` 不复制 |
| `sentence-model.safetensors` | [metasequoiaime/chinese-ime-lm](https://github.com/metasequoiaime/chinese-ime-lm) 的 `model-v1` 发布，经 `desktop-dictionary.lock.json` 与 `resources/neural-model.lock.json` 固定 |
| `rerank-model/`、`rerank-model-small/`（`lm_step.ort`、`model.json`、`vocab.json`，只在宿主打开 `onnx-rerank` 时读） | GitHub Release [`metasequoiaime/chinese-ime-lm` 的 `rerank-v1`](https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/rerank-v1)（文件名带尺寸前缀，如 `rerank-8x448.ort`），同一批文件也在 Hugging Face：[`msime/pinyin-ime-keystroke-reranker-25M`](https://huggingface.co/msime/pinyin-ime-keystroke-reranker-25M)（8x448）和 [`msime/pinyin-ime-keystroke-reranker-8M`](https://huggingface.co/msime/pinyin-ime-keystroke-reranker-8M)（6x256），那里是原名（`lm_step.ort`、`model.json`、`vocab.json`）并附 `LICENSE` 与 `NOTICE`。还没有锁文件，也还没有随任何安装包分发 |
| octagram `.gram` | 上游发布：amzxyz/rime-wanxiang-data、lotem/rime-octagram-data、gaboolic/rime-frost，经 `resources/grammar-models.lock.json` 固定 |
| `handwriting-zh_CN.model` | metasequoiaime/msime-engine `a9b9f09`，经 `resources/handwriting-model.lock.json` 固定 |
| `helpcodes/*.txt` | 提交在本仓库 `resources/helpcodes/`；`wubi86_helpcode.txt` 由 `msime-dict-build wubi86-helpcode` 生成 |
| `msime_user.db`、`translation-glosses.db`、代次目录 | 引擎和宿主在运行时写 |
| `custom_translations.txt` | 用户或宿主 |

dict-builder 的其他子命令（`places`、`hanja`、`syllable-frequency`）产出的是编进引擎的表（`include_str!`），不是运行时文件；它们的来源与许可证见 `docs/third-party.md`。

## 20. 每个方案的最小数据集

按 `prepare_runtime_paths_for` 只为用到的方案准备代次时，资源目录只要有下表的文件；`prepare_runtime_paths`（全部方案）要全拼那一行。几个方案一起用时取各行的并集。「产品同等」指与 msime 自己的产品排序相同还需要的文件。

| 方案 | 必需 | 产品同等再加 | 可选 |
| --- | --- | --- | --- |
| 全拼、双拼、九键 | `msime-pinyin.db`、`msime-english.db` | `msime-bigram.bin`、`msime-trigram.bin` | `msime-others.db`（本地模式、表情混输）、`sentence-model.safetensors`、`.gram`、辅助码表 |
| 五笔 | 上一行加 `msime-wubi.db` | 同上 | 同上 |
| 日文 | 无（`msime-japanese.dat` 缺了只出假名） | `msime-japanese.dat`、`msime-mozc_dictionary_oss_README.txt`、`msime-mozc_LICENSE.txt` | `msime-english.db`（专用英文） |
| 韩语、越南语、藏文 | 无 | — | `msime-english.db`（专用英文） |
| 粤语、注音、笔画 | 对应的语言词库及其许可证文本（缺了激活失败） | — | `msime-english.db`（专用英文） |
| 手写 | `handwriting-zh_CN.model` 及其许可证文本 | — | — |

全拼、双拼、五笔自己的功能读 `msime-english.db`：英文混输、九键英文、临时英文（Shift+Y）和回车学英文词，所以它在这几行是必需的。其余方案只有宿主打开专用英文（`Session::set_dedicated_english`）时才读它：带了就有英文补全，回车学词也写进它；没带时专用英文只出键入的字母、回车照常上屏而不学词，个人词典也存不下英文词（编辑以 `This input method has no English dictionary` 失败），引擎不会在代次里建一个空的英文库。带 `msime-english.db` 时同时带 `msime-scowl_Copyright.txt`（SCOWL 的条款要求）。

## 21. 数据许可证

**引擎代码目前按 GPL-3.0-only 发布，计划改为 MPL-2.0；数据按各自的许可证发布，不随引擎改许可证，由采用方自行选择带哪些数据。** 下表按文件列出数据里各个来源的许可证，未经律师审阅。每个来源文件的上游、固定的提交与条款以 msime-dictionary 的 `NOTICE.md` 为准，与下表不一致时以它为准；本仓库内的来源说明在 `docs/third-party.md`。发布构建不读 `sources/unlicensed/` 下没有再分发授权的输入（`crates/dict-builder/src/licensing.rs`，`--include-unlicensed` 只用于本地评测），所以下表不列它们。

| 文件 | 数据许可证 |
| --- | --- |
| `msime-pinyin.db` | GPL-3.0（rime-ice 及其补充）；与万象共有词条的权重和字频为 CC-BY-4.0（rime-wanxiang）；单字读音按 MIT 的 pinyin-data 校正；行政区划地名为 WTFPL |
| `msime-wubi.db` | Apache-2.0（极点 86）、LGPL-3.0（rime-wubi 补入的词条）、GPL-3.0-or-later（fcitx 98）、Unlicense（98 五笔小组）；本项目生成的 86、98 补充表为 GPL-3.0，词条依据 GPL-3.0 的 rime-ice、人工词条与 WTFPL 的区划名称 |
| `msime-english.db` | GPL-3.0（rime-ice-en 及补充）；含 SCOWL 的词，分发时必须附 `msime-scowl_Copyright.txt`；Google 词频以来源页面说明为准 |
| `msime-bigram.bin`、`msime-trigram.bin` | 计数来自中文维基百科（CC-BY-SA-4.0），词表是 GPL-3.0 的 `msime-pinyin.db` |
| `msime-others.db` | emoji 数据与颜文字（`resources/dictionary-sources/kaomoji/kaomoji.txt`）的来源都未记录，PiliApp 符号未声明许可，再分发前需另行确认 |
| `msime-japanese.dat` | Mozc 开源词库：Google 三条款 BSD 加词典目录的 IPAdic、ICOT、冲绳辞書条款；分发时必须附 `msime-mozc_dictionary_oss_README.txt` 与 `msime-mozc_LICENSE.txt` |
| `msime-cantonese.db` | CC-BY-4.0（rime-cantonese、HKCanCor，以及 msime-dictionary 的人工词表 `custom/cantonese.txt`，本项目以 CC BY 4.0 提供） |
| `msime-zhuyin.db` | LGPL-2.1-or-later（libchewing-data）；补充词表（McBopomofo 的 `BPMFMappings.txt`）源自 libtabe 的 `tsi.src`，为 BSD-3-Clause，McBopomofo 的改动与词频 `phrase.occ` 为 MIT（文件内记为 `LGPL-2.1-or-later AND BSD-3-Clause AND MIT`，`dict-v2.0.40` 及以前的发布只记了 `LGPL-2.1-or-later`） |
| `msime-stroke.db` | LGPL-3.0（rime-stroke），权重取自 GPL-3.0 的 rime-ice 字频，其中与万象共有读音的字频为 CC-BY-4.0（rime-wanxiang；文件内记为 `LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0`，`dict-v2.0.40` 及以前的发布只记了 `LGPL-3.0-only AND GPL-3.0-only`） |
| `sentence-model.safetensors` | Apache-2.0（权重）；训练语料 C4 中文部分 ODC-BY、LCCC MIT，署名在 `__metadata__` 里 |
| octagram `.gram` | 按模型：rime-wanxiang-data CC-BY-4.0、rime-octagram-data LGPL-3.0、rime-frost GPL-3.0 |
| `handwriting-zh_CN.model` | LGPL-2.1，附 `HandwritingModel-LICENSE.txt` |
| `helpcodes/wubi86_helpcode.txt` | 见 `resources/helpcodes/NOTICE-wubi86.md` |
| 其余六张辅助码表 | 见下 |
| `custom_translations.txt`（随包那份） | 未核实 |

**辅助码表。** `lantian`、`ziranma`、`shouyou2_0`、`shouyouplus`、`xiaohe`、`jiajia` 六张表都没有明确的再分发授权（`resources/helpcodes/NOTICE.md`），**没有再分发授权的辅助码表不属于任何数据发布**：它们只随 msime 自己的产品分发，任何面向第三方的数据发布都不包含它们，采用方需要时自备。只有 `wubi86` 有自己的来源与许可说明。

## 22. 不在本规格内的文件

- 编进引擎二进制的表（`include_str!`，如 `places.tsv`、`hanja.tsv`、`chaizi*.tsv`、`symbol_groups.tsv`、`syllable_frequency.tsv`）不是运行时文件，它们的来源与许可证见 `docs/third-party.md`。
- 宿主门面（`host::*`）另外读的离线释义包、英文音标库（`pronunciations/en-phonetic.db`，`user_version=1` 加 `meta.kind='en_phonetic'`）和法日西俄德韩的读音库（`target-pronunciations/<语言>-phonetic.db`，表 `<语言>_phonetics(word, phonetic)`，`user_version=1` 加 `meta.kind='<语言>_phonetic'`，键是维基词典的写法，读音是带斜线或方括号的 IPA，日语是赫本式罗马字，韩语是文化观光部式罗马字）属于产品功能，不在引擎库对外承诺的范围内。
- 宿主侧的资源包、皮肤、插件、账号与同步数据由各自的模块规定。
