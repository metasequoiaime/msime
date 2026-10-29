# Agent Note: 主库、五笔库与 others 库的文件内版本

Status: implemented

## Problem

引擎要做成给其他输入法用的库（设计笔记 `.agents/notes/implemented/architecture/2026-10-29-embeddable-engine-library.md`，P5），采用方会拿到、甚至自己产出词库文件。三个语言词库从一开始就带 `metadata.format_version`，读端不认识就拒绝；但 `msime-pinyin.db`、`msime-wubi.db`、`msime-others.db` 没有任何文件内版本，版本只记在发布清单的 `engine_compatibility.dictionary_format` 里，而引擎根本不读清单。一个改了表结构的新词库放进旧引擎的资源目录，旧引擎会照旧按老规则去读，结果是查不到词或悄悄读错，而不是一个看得懂的错误。同时规格文档（代码里到处引用的 `data-formats.md`）不在仓库里，第三方无从知道这些文件长什么样。

## Decision

- 三个主库各带一张与语言词库同形的表：`CREATE TABLE IF NOT EXISTS metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID`，`format_version` 行的值是十进制文本，等于 `msime_engine::format::FORMAT_VERSION`（现在是 1，也就是清单里的 `dictionary_format`）。定义只在 `crates/engine/src/format.rs` 一处：`METADATA_SCHEMA`、`METADATA_FORMAT_VERSION`、`LEGACY_FORMAT_VERSION`、`check_format_version`。
- dict-builder 在拆出五笔库之后、冻结数据库之前写入（`product::stamp_format_versions`），产品检查用引擎的 `check_format_version` 核对三个文件都是当前版本，构建与读端是同一个契约。`msime-dict-build web` 保留拼音库的 `metadata`，跳过五笔库那份，裁出的网页库与拼音库同版本。
- 读端规则：没有 `metadata` 表按版本 0 照常读，所以 `dict-v2.0.39` 及以前的发布、各平台已经装好的词库、各处测试夹具都不受影响；0 到 `FORMAT_VERSION` 之间照常读；更大、不是整数、缺 `format_version` 行或 `metadata` 的列不对时拒绝，错误是 `Dictionary format version unsupported: <文件名> ..., this engine reads format_version <N> or older`，只带文件名不带路径；连 `sqlite_master` 都读不出的文件不在这里报错，读 `metadata` 时被锁、读写出错、文件损坏这类与格式无关的错误也一样，交还给原来的读写步骤，保持它们原有的诊断，不说成格式不受支持。
- 检查点：准备代次复制主库之前，重新打开已经就绪的代次时（查代次里的主库副本：更新的引擎准备的代次在宿主降级后仍可能被选中，客户端只在下载时按清单检查格式），合并 `msime-wubi.db` 之前（包括宿主下载的五笔库），只读布局（`dictionaries == resources`）建会话时，后端（`backend::execute`，不建会话、直接打开资源里的库）每次请求前（`convert` 除外）查主库、五笔库和 `msime-others.db` 并以 `resources_unavailable` 拒绝，打开 `msime-others.db` 的目录接口时（原样返回版本错误），以及按键路径上的本地模式连接缓存（版本不支持按读不了处理，因为那里没有别的报错出口）。准备代次时已经查过的布局，建会话不再重复打开文件。
- 改格式的规矩写进 `docs/data-formats.md` §4.3 和 `docs/ARCHITECTURE.md`：读端不认识的改动必须把 `FORMAT_VERSION` 加一并在同一个提交里让引擎学会新版本；只追加读端会忽略的表或索引不加版本。
- `docs/data-formats.md` 是运行时文件的规格，章节号沿用代码注释里已有的 `data-formats.md §N` 引用（§1.4、§3、§4.1、§5、§6.2、§7、§8、§10、§11、§14），新章节只往后加。
- 顺带删掉 `assets::GLOSS_CACHE`：整个仓库没有任何引用，用户磁盘上的释义缓存一直叫 `translation-glosses.db`，规格 §1.4 记下了这段历史。

## Alternatives considered

- **用 SQLite 的 `PRAGMA user_version` 当版本。** 最强的理由是不加表、读起来最便宜，`msime-english.db` 和学习日志已经在写它。不用：`msime-english.db` 的 `user_version=3` 和日志的 `user_version=4` 都是只写不读的历史值，含义各不相同；`user_version` 只有一个整数，放不下以后可能要的 `source_commit`、`license` 这类元数据；语言词库已经用 `metadata` 表，同一个引擎里两种做法只会让采用方困惑。
- **只靠发布清单的 `dictionary_format`。** 最强的理由是它已经存在，客户端的词库更新也只接受版本相同的清单。不用：引擎不读清单，第三方拿到的常常只是单个 `.db` 文件；清单和文件会分开搬运，只有文件里的版本跟着文件走。
- **严格要求 `format_version` 等于当前版本（像语言词库那样）。** 最强的理由是简单、没有歧义。不用：现有发布全都没有这张表，严格要求会让所有已装好的词库在升级引擎后失效；以后升到版本 2 时也希望新引擎还能读版本 1 的文件，所以规则是「不超过当前版本」。

## Consequences

- 收益：不兼容的新词库在旧引擎里得到带文件名的明确错误，而不是误读；第三方有了一份逐表、逐字段的规格和每个方案的最小数据集；构建端与读端共用一份定义，构建出来的文件过不了引擎检查就不出产品。
- 代价：准备代次、重新打开代次、合并五笔库、只读布局建会话和每次后端请求各多一到三次只读打开和一两条查询；三个发布文件的字节变了（多一张小表），下一次 `dict-v*` 发布之后 `resources/desktop-dictionary.lock.json` 照常提升。
- 回滚：引擎侧的检查对没有 `metadata` 的文件是空操作，撤掉检查不影响任何已发布文件；dict-builder 停写 `metadata` 也只会让文件回到版本 0。
- 验证：`msime-engine` 的 `format::tests`、`user_dictionary::generation::tests` 的四个版本用例（含重新打开就绪代次）、`backend::tests::a_newer_dictionary_format_makes_resources_unavailable`、`local::catalog::tests::the_catalog_checks_the_in_file_format_version`；`msime-dict-builder` 的 `product::tests::every_main_database_carries_the_format_version` 和 `tests/web.rs::versioned_inputs_keep_one_metadata_table`。没有在真实的完整发布上跑一次主构建：那要 msime-dictionary 检出和约 500 MB 的固定输入，留给下一次 `dict-v*` 发布的工作流。
