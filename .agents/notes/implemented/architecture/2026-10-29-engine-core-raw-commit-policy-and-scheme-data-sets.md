# Agent Note: 回车学词策略进核心，代次准备按方案集合要求词库

Status: implemented

## Problem

可嵌入引擎库的提案（[embeddable-engine-library](../../implemented/architecture/2026-10-29-embeddable-engine-library.md)）在 P3 硬点表里留了两件事，P7（[engine-host-dogfood](2026-10-29-engine-host-dogfood.md)）做完之后仍没动：

- 回车学英文词的产品策略和 `CommitRawWithoutLearning` 只在 `host::Session` 里。核心 `Session` 的 `CommitRaw` 只在专用英文里学词；临时模式里原样上屏的字母、全拼双拼里拼不成音节的拼音是否学成英文词，Rust 核心 API 和 C ABI 的使用者都拿不到，也没有「上屏但不学」的命令。
- `prepare_runtime_paths_for` 只放开了 `msime-pinyin.db`：不含全拼、双拼、五笔的方案集合照样要求 `msime-english.db`，只用韩语、越南语、藏文、日文、粤语、注音、笔画的采用方也得带 rime-ice-en 的 GPL-3.0 数据。重置（`reset_learned_data_with`）同样要求它；C ABI 1.0 的 `prepare_runtime`、`reset_learning` 不分方案，连 `msime-pinyin.db` 都要。

## Decision

### 回车学词的选项与命令

- `SessionOptions::learn_english_on_enter`（已有，缺省开）是总开关。只开它时 `CommitRaw` 只学专用英文里打的词，与参照实现相同。
- 新增 `SessionOptions::learn_typed_spellings_on_enter`，缺省关。打开后 `CommitRaw` 在专用英文之外还把临时模式里原样上屏的字母（去掉入口字母，临时日文的词前加 `R`）和全拼、双拼里拼不成完整音节的拼音学成英文词；九宫格组字、整句改字、生成文本的本地模式（计算、指令、名单）和网址不学；学不进去时上屏照旧，结果带 `ENGLISH_WORD_NOT_LEARNED`。规则原样搬自门面（bridge.cpp:1332-1359），实现在 `crates/engine/src/session/raw_commit.rs`。
- 核心 `Command` 追加 `CommitRawWithoutLearning = 14`：专用英文里取预编辑原文并结束组字，其余情况就是不带上面策略的 `CommitRaw`（拼写纠错「回车上屏原文就关掉这段输入的纠错」照常记）。`Session::command` 在分派之前处理它，组字的内部路径收不到；内部 `match` 把它与 `CommitRaw` 并列只为穷尽。
- 门面只传产品取值：`host::options::session_options` 把 `learn_typed_spellings_on_enter` 写成 `true`，`host::Session::command` 把宿主的 `CommitRaw`、`CommitRawWithoutLearning` 交给核心同名命令（宿主编号 11 按名字映射到核心的 14）。产品行为不变，`host/tests.rs` 的回车学词用例原样通过。
- C ABI：选项 JSON 的 `english` 加 `learn_on_enter`、`learn_typed_spellings_on_enter`，命令加 `MSIME_ENGINE_COMMAND_COMMIT_RAW_WITHOUT_LEARNING`（14，沿用核心序号）。

### 代次只要求方案集合读的词库

- `SchemeSet::requires_english_dictionary()`：集合含全拼、双拼或五笔时为真。这三个方案自己的功能读英文库（英文混输、九键英文、临时英文、回车学词）；其余方案只在宿主打开专用英文时读它。读主库的集合一定要求英文库，`types.rs` 的测试钉住这一点。
- `prepare_runtime_paths_for`：要求英文库的集合与以前完全相同。不要求的集合在资源目录带了 `msime-english.db` 时照样复制、只回放英文行；没带时代次里两个库都没有，日志一行也不回放、原样留着（以后带英文库的代次照样回放）。就绪代次里不要求的英文库在不在，以准备时有没有复制为准，后来才放进资源目录的英文库要换 `content_id`。
- `reset_learned_data_for(paths, schemes)` 按同一规则换回词库；`reset_learned_data_with(paths, false)`（门面）不再要求资源目录有英文库。`prepare_runtime_paths_for` 与 `reset_learned_data_for` 从 crate 根导出。
- 没有英文库时不建空库：会话只在方案集合不要求英文库、代次里也没有它时跳过回车学词（`InputSession::can_learn_english_words`），不报诊断；要求它的集合照旧由学词时的建表补出（没经过准备的目录，测试夹具就是这样）。个人词典编辑在没有主库也没有英文库时以 `NO_ENGLISH_DICTIONARY` 失败，不在代次里建库。读英文库的地方（`EnglishDictionary::open`、九键英文、多语言、释义）本来就把缺文件当作没有行。
- 不分方案集合的根入口 `edit_personal_dictionary`、`personal_dictionary_entries`、`edit_bundled_dictionary_entry` 按代次实际有没有 `msime-pinyin.db` 决定能编辑、列出什么（以前写死「有主库」，在不读主库的代次上编辑任何词都以「Cannot open dictionary」失败）。
- C ABI 追加 `msime_engine_prepare_runtime_for(paths, content_id, len, schemes, out)` 与 `msime_engine_reset_learning_for(paths, schemes)`，`schemes` 是方案位集合（第 n 位是序号 n 的方案），0 和不认识的位是 `INVALID_ARGUMENT`；个人词典的列表与编辑经上面的根入口按代次内容走。msime-engine-capi 0.1.0、0.1.1 已经以 ABI 1.0 发布到 crates.io，这些追加按「每次追加加一」把次版本升到 1.1：`MSIME_ENGINE_ABI_MINOR`、`.pc` 的 Version 和库文件版本（`libmsime_engine.so.1.1.0`、`libmsime_engine.1.1.0.dylib`，SONAME 不变）一起改，打包脚本与发布工作流里的库文件名跟着改。

最小数据集写在 `docs/data-formats.md` §20、`crates/engine/README.md` 和 `crates/engine-capi/README.md`：韩语、越南语、藏文什么也不用带，日文只要 `msime-japanese.dat`（缺了只出假名），粤语、注音、笔画只要各自的语言库，`msime-english.db` 在这些方案里只是专用英文的可选项。

## Alternatives considered

- **门面策略保持在门面，只给核心加一个「学所有原样上屏」的布尔** —— 改动更小。不行：规则的细节（九宫格、整句改字、生成文本的模式、网址、临时日文的 `R`）正是产品打磨出来的部分，第三方要的就是这些；只给一个粗开关等于让他们各自重写一遍。
- **`CommitRawWithoutLearning` 做成 `Session` 的方法而不是 `Command` 变体** —— 不用碰组字内部的三处穷尽 `match`。但 C ABI 的命令沿用核心序号，golden 场景按名字找命令，方法要在 C ABI 里另开一个特例编号；变体让三边都只是多了一个名字。
- **英文库对所有方案保持必需，只把 `msime-pinyin.db` 放开（现状）** —— 专用英文在任何方案里都开箱可用。代价是只做韩语或越南语的采用方必须带 GPL-3.0 数据，提案待拍板 2 的宽松许可数据集以去掉这个要求为前提。
- **不要求英文库的集合一律不复制它** —— 规则最简单。但 msime 自己的日文、越南文、藏文版本随包带着英文库、靠它给专用英文出补全和存英文个人词，一律不复制会改掉这些版本的行为；按「带了就复制」两边都不变。
- **没有英文库时一律跳过学词（不看方案集合）** —— 判断只看文件。但 `host/tests.rs` 等夹具的代次没有英文库、靠学词时建表，产品契约在这些用例里会跟着变；限定在不要求英文库的集合上，要求它的集合与以前逐字节相同。
- **C ABI 的 `reset_learning` 按代次里有没有主库自己推断，不加 `_for`** —— 少一个函数。不行：重置途中被杀时主库副本可能还挪在备份里，按文件推断会把它当成不读主库的代次；准备与重置都显式带集合更不容易出错。个人词典的列表与编辑只在准备好的代次上运行，按代次内容推断没有这个问题。
- **不升 C ABI 次版本，直接加进 1.0** —— 少改 `.pc`、库文件名和发布工作流。不行：1.0 已经随 crates.io 上的 0.1.0、0.1.1 发布，前端按 `msime_engine_abi_version()` 判断能不能调后加的函数，不升版它们就分不出旧库。

## Consequences

- **收益**：第三方经核心 API 或 C ABI 就能得到 msime 的回车学词策略和「上屏但不学」；只用非中文方案的采用方可以不带 `msime-pinyin.db` 和 `msime-english.db`，也就不带它们的 GPL-3.0 数据，提案的宽松许可数据集不再被这条挡住。
- **行为变化**：只限于方案集合不读主库的情况——资源目录没带英文库时准备和重置不再失败；这样的代次里专用英文的回车不学词、不建库，个人词典编辑报 `NO_ENGLISH_DICTIONARY`。根入口的个人词典函数在没有主库的代次上能编辑英文词（以前一律失败）。msime 产品（全部方案、随包带全部文件）和带英文库的单语版本不变。
- **代价**：核心 `Command` 多一个变体，组字内部三处 `match` 为穷尽并列它；不要求英文库的就绪代次不会自动吸收之后放进资源目录的英文库，要换 `content_id`。C ABI 从 33 个函数变成 35 个，次版本升到 1.1，库文件名随之变化（打包与发布工作流已同步）。
- **审计**：提案的硬点表、运行时文件表、最小数据集与缺口清单就地更新事实；[engine-host-dogfood](2026-10-29-engine-host-dogfood.md) 的「没做的」、[engine-library-defaults-errors-log-and-lock](2026-10-29-engine-library-defaults-errors-log-and-lock.md) 的剩余项和 [engine-capi](2026-10-29-engine-capi.md) 的「不分方案」就地改成现状并链到这里（部分重叠）；[Esc 保留已选词与回车学英文词改为可配](../feature/2026-10-21-esc-keeps-selection-and-enter-learns-english.md) 讲产品总开关的由来，它的 Problem 记的是当时门面里的位置，部分重叠、互不改写；[核心会话收尾](2026-10-29-core-session-lifecycle-and-journal-threads.md)、[主库文件内版本](2026-10-29-main-dictionary-in-file-version.md) 无关。

## Verification

- `cargo test -p msime-engine`（lib、golden 31 例、`tests/scheme_data_sets.rs`、`tests/session_threads.rs`）、`cargo test -p msime-engine-capi`、`cargo test -p msime-input-runtime`、`cargo test -p msime-host-api --lib`，以及 `cargo clippy` 覆盖这几个 crate 的全部目标。在 Mac Studio 上跑，SSH 会话的 `ulimit -n` 是 256，引擎的 lib 测试并行开 SQLite 时会撞上它（夹具报 `unable to open database file`），所以带 `ulimit -n 10240` 跑。
- `session/tests/raw_commit.rs` 钉住引擎库默认值（只学专用英文）、`learn_typed_spellings_on_enter`、`CommitRawWithoutLearning` 和「不要求英文库的集合不建库」；产品取值下的完整策略仍由 `host/tests.rs` 经门面钉住。
- `tests/scheme_data_sets.rs` 只用公开 API：空资源目录上准备、建韩语会话打字、专用英文回车、个人词典编辑报错、重置；日文、越南语、藏文在空目录上建会话打字；带英文库时专用英文补全、回车学词、个人词典与重置换回。
- C ABI：`prepare_runtime_for`、`reset_learning_for`、新选项与命令有 ABI 测试；`crates/engine-capi/tests/c-example.sh --update-header` 在 Studio 上重新生成头文件，并按动态、静态、静态加系统 SQLite 编译运行 C 示例。
