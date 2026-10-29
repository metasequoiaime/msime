# Agent Note: 引擎的 C ABI（msime-engine-capi）：会话句柄、借用字符串、版本协商与 cargo-c 构建

Status: implemented

## Problem

引擎要做成给其他输入法嵌入的库（PR #10167 合入的提案「msime-engine 作为可嵌入的输入引擎库」，`implemented/architecture/2026-10-29-embeddable-engine-library.md`，本分支的基点早于它合入），其中 P4 是新 crate `crates/engine-capi`：一个只包核心层 `msime_engine::Session` 的 C 库。仓库里唯一的 C ABI 是 host-api 的 `msime_client.h`，它是 GPL 的客户端门面，每次按键走 JSON，错误只有字符串，会话绑定在线程上，不能直接给第三方。草案定下了 33 个函数、`struct_size` 版本协商、`int32_t` 返回码、线程局部的错误说明、热路径上借用的字符串和 cargo-c 构建；P3 的两篇笔记（[核心会话收尾与日志连接的线程归属](2026-10-29-core-session-lifecycle-and-journal-threads.md)、[引擎库的一套默认值、错误码、日志出口与维护锁归属](2026-10-29-engine-library-defaults-errors-log-and-lock.md)）给出了它依赖的修订号、核心收尾、`Send` 的会话、错误码和搬进引擎的维护锁。落地时要定的是草案没写到或者写得和代码对不上的细节。

## Decision

### crate 形态

`crates/engine-capi`，包名 `msime-engine-capi`，`license = "MPL-2.0"`（不从工作区继承，目录里有 MPL-2.0 全文的 `LICENSE`，README 的「许可证」段写 Exhibit A），`crate-type = ["cdylib", "staticlib", "rlib"]`，`#![allow(unsafe_code)]` 写在 crate 根并说明理由。rlib 是给工作区里的 Rust 代码（ABI 测试、以后 msime 宿主自用）链接同一份实现。它在工作区 `members` 和 `default-members` 里。引擎为它多公开两样：类型 `FirstPageRows`（`Session::set_first_page_rows` 是公开方法，参数类型却没导出），以及 `Session::display_snapshot`。后者是 host 门面一直在用的宿主快照（全拼的 `preedit` 按音节分开，`ni'hao'shi'jie`），核心的 `snapshot()` 保留按键原文给参考回放比对；`msime_engine_state.preedit` 承诺的是给用户看的组字，所以 C 接口读宿主那一份。

构建用 cargo-c（维护者 2026-10-29 拍板）。`package.metadata.capi` 定下 C 侧的名字与版本：库名 `msime_engine`，头文件 `msime_engine.h` 直接装进 includedir，pkg-config 名 `msime-engine`；库版本取 ABI 版本 `1.0.0`、`version_suffix_components = 1`，所以 Linux 的 SONAME 是 `libmsime_engine.so.1`，macOS 的 install name 是 `<libdir>/libmsime_engine.1.dylib`、compatibility version 是 1。crate 自己的版本是 0.1.0，与 ABI 版本分开。`.pc` 的 `Version` 同样设成 ABI 版本 `1.0.0`（cargo-c 缺省用 crate 版本），前端可以写 `msime-engine >= 1.0` 按 ABI 做编译期开关；`tests::library_version_follows_the_abi` 核对库版本和 `.pc` 版本都等于 `MSIME_ENGINE_ABI_MAJOR.MINOR.0`，C 示例脚本再核对装出来的 `.pc`。草案要的「默认共享数据目录变量」没有做：cargo-c 0.10 生成的 `.pc` 只有 prefix、exec_prefix、libdir、includedir 四个变量，不能加自定义变量；库本身也没有默认数据目录，四个根目录总由前端传入，这个变量没有可指的地方。cargo-c 生成的版本号宏会是 crate 版本，关掉（`emit_version_constants = false`），免得采用方拿它当 ABI 版本。苹果平台上给 rustc 加 `-Cstrip=none`：工作区 dev 配置 `debug = 0` 时 rustc 默认剥调试信息，Xcode 27 的 `strip` 剥过的 dylib 被它自己的链接器拒绝（`mis-aligned LINKEDIT string pool`，在 Studio 上复现：同一个 cdylib 不剥就能链接）。

头文件由 cargo-c 调 cbindgen 从 `src/` 生成（`cbindgen.toml`：C、`MSIME_ENGINE_H`、C++ 兼容、按源码顺序、`size_t`），提交在 `include/msime_engine.h`；`tests/c-example.sh --update-header` 重新生成，CI 比对漂移。所有常量都是 `pub const`（cbindgen 出 `#define`），不用 C 的 `enum`：枚举的宽度在 C 里不固定。

### 函数面

33 个函数与草案一一对应（ABI 1.1 为按方案准备追加了 `prepare_runtime_for`、`reset_learning_for`，见下文「不分方案」一条；又追加了 `candidate_key`、`user_dict_validate`、`set_log_callback`，以及选项 JSON 的调频、混排与本地模式开关和四个运行时选项编号，见 [引擎 C ABI 1.1 补上的缺口](2026-10-30-engine-capi-abi-1-1-additions.md)，现在共 38 个），按代码调整的地方：

- 四个函数不返回状态码：`abi_version` 返回 `(major << 16) | minor`，`version` 和 `last_error` 返回 `const char*`，`string_free` 无返回值。其余 29 个返回 `int32_t`。
- `prepare_runtime` 用 `char**` 交回代次目录，由 `string_free` 释放：采用方要把它原样放进选项的 `paths.dictionaries`，自己拼路径在 Windows 上容易错。所以 `string_free` 释放的是库交出的一切字符串，不只 JSON。
- `flush_learning(flags)` 带一个 `MSIME_ENGINE_FLUSH_STOP_THREAD` 位，置位时调 `msime_engine::shutdown()`（P3 加的写完并 join 冲刷线程），不置位时调 `flush_personal_learning()`。卸载前停线程的路径因此不必多一个函数。
- `preload_model(kind, path, bytes)` 现在只有 `MSIME_ENGINE_MODEL_JAPANESE`，对应 `preload_japanese_dictionary`；`bytes` 为 NULL 且长度为 0 时撤掉。别的种类是 `UNSUPPORTED`，以后加种类不加函数。
- 选项 JSON 比草案的子集多一个 `next_word_prediction`：没有它，`MSIME_ENGINE_OPTION_PREDICTION_ALLOWED` 这个门禁永远无事可管。枚举类的取值在 JSON 里用名字（`quanpin`、`xiaohe`、`z_zh`），整数编号只出现在 C 函数的参数里。
- 命令 11 在 ABI 里叫 `MSIME_ENGINE_COMMAND_OPEN_CANDIDATE_LIST`：核心层的 `ConvertHanja` 是历史名，它现在开关的是当前方案可打开的列表（韩文汉字、注音整句），编号不变。
- `prepare_runtime` 和 `reset_learning` 不分方案，用引擎的 `prepare_runtime_paths` 和 `reset_learned_data`，所以要 `msime-pinyin.db` 和 `msime-english.db`。按方案准备由后加的 `prepare_runtime_for`、`reset_learning_for` 承担（带方案位集合，见 [回车学词策略进核心，代次准备按方案集合要求词库](2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md)），README 的数据表按方案写明要求。`prepare_runtime` 不读 `paths.dictionaries`：调用时还没有代次目录，C 调用方很自然地只填前三项。
- 个人词典三个函数用引擎层自己取锁的版本（`edit_personal_dictionary`、`reset_learned_data`）和只读的 `personal_dictionary_entries`。引擎以后新加的词条种类在 JSON 里还没有名字，列表里先略过（`PersonalDictionaryKind` 是 `non_exhaustive`）。

### 会话句柄

`msime_engine_session` 是不透明的类型化指针，里面一把 `Mutex` 护着会话状态。每个入口只 `try_lock`，拿不到返回 `BUSY`，不等待：同一会话的调用由采用方串行，重叠就是用错了。核心 `Session` 是 `Send`，所以会话可以在任何线程上用、在调用之间换线程，不同会话可以并行；不再像 host-api 那样把会话绑在建它的线程上。入口外层的 `catch_unwind` 接住 panic；会话里的 panic 置位会话的 `poisoned`，之后这个会话的每次调用都返回 `PANIC`，只能释放。`session_free` 在别的线程正调着时返回 `BUSY`、不释放。

会话状态里除了引擎会话还有三样：

- **共享词库租约。** `session_new` 在建会话之前取 `user_data` 与代次目录上的共享租约（`DictionaryAccess::try_session`），持到会话结束；字段顺序保证会话先结束、写完学习，租约后放。维护（`user_dict_edit`、`reset_learning`）要独占租约，于是 C ABI 的会话和维护按 msime 各版本宿主用的同一把文件锁互斥，进行中的维护也让 `session_new` 返回 `BUSY`。这回答了上面那篇维护锁笔记留给 C ABI 的问题：核心 `Session` 仍不取租约（golden 和引擎测试在会话开着时改词库），取租约的是 C 包装层。
- **快照缓存。** `state_get` 和 `candidate_get` 第一次调用时取一份 `SessionSnapshot`，借出的字符串都指向它；每次改变状态的调用开始时清掉它。缓存不按修订号判断是否过期，因为有改变快照却不改修订号的调用：标点开关和锁不递增修订号，但空闲时 `/`、`@` 是否开模式跟着它们变，`spelling_symbols` 就变了。
- **上一次的结果。** `msime_engine_result` 的 `commit` 和 `diagnostic` 指向会话里存着的上一个 `KeyResult`，下一次改变状态的调用才换掉它。

所以借用字符串的有效期只有一条规则：到同一会话下一次改变状态的调用为止。改变状态的调用是按键、命令、选择与逐候选学习、`set_*`、`switch_scheme`、`set_caret`、`reset_context`、`expand_candidates`；`state_get`、`candidate_get`、`state_json` 不算。选择类函数和 `candidate_get` 先核对修订号，不对就返回 `STALE`，什么也不清，之前借出的字符串照样有效。

### 结构体与错误

每个进出的结构体以 `uint32_t struct_size` 开头。输出时库写 min(调用方大小, 库内大小) 个字节，把 `struct_size` 改写成写入的大小，调用方多出的部分清零；比 1.0 的布局还小是 `INVALID_ARGUMENT`。改变状态的调用（按键、标点、命令、七个选择类函数）在动会话之前就核对结果结构体的大小，被拒绝的调用什么也没做；如果等到写结果时才核对，键已经进了组字、整句已经上屏并学过，调用方却拿到失败和丢掉的上屏文字。输出结构体按字节整块复制，所以设计成没有填充，编译期断言字段大小之和等于 `size_of`；输入结构体（`msime_engine_paths`）先复制进一块清零的内存再读，读不到的字段是零。`Sized1::V1_SIZE` 现在等于 `size_of`，追加字段时要把它固定成 1.0 的大小。

返回码的 1、2、3、6 与引擎 `ErrorCode` 相同，`lib.rs` 里有编译期断言；4、5、7、8 是包装层自己的情形。`last_error` 存在线程局部的 `Option<CString>` 里，每个入口先清掉它，成功的调用不分配；`last_error` 自己不清。`msime_engine_result.diagnostic_code` 是引擎的 `DiagnosticCode`，0 表示没有诊断。

### 测试与 CI

`src/tests.rs` 经 C 函数本身测：版本与库版本、`.pc` 版本一致、空指针、选项 JSON 拒绝未知键与错版本、打一句全拼后核对状态（含按音节分开的 `preedit`）和候选、过期修订号被拒且不改状态、选词上屏、冲刷后个人词典里有这句、`struct_size` 的三种情形、结果结构体太小的按键和选择不改会话、没处理的数字键按它交回的修订号选词（README 按键路由第 4 条）、`prepare_runtime` 不读 `paths.dictionaries` 并拒绝带点的 id、并发调用报 `BUSY`、会话跨线程使用、panic 只作废自己的会话、会话开着时维护报 `BUSY` 而释放后成功、维护进行中建不了会话、各种不认识的编号、标点与命令的路由、`last_error` 按线程。夹具是 `tests/fixture/*.sql`，与 C 示例共用；建表方式同 `session/tests.rs`，另外有空的三、四音节表，因为选中整句时引擎把它当用户词写进对应长度的表，表不在就报 `SENTENCE_NOT_PERSISTED`。

`tests/c-example.sh` 用 `cargo cinstall --debug` 装进 `target/engine-capi/stage`，核对头文件没有漂移、共享库的 SONAME 或 install name 带主版本、`platforms/linux/collect-notices.py` 按 `msime-engine-capi` 的依赖图生成得出第三方声明（并且收进了 `libsqlite3-sys`，不带许可证文件的 crate 仍只有 README 交代过的 `ewts`、`exmex`、`rink-core`）、`.pc` 的 `Version` 是 ABI 版本，再用 CMake 把 `examples/c/example.c` 按动态库、静态库、静态库加系统 SQLite（Windows 上没有这一个）各编一遍并运行：准备代次、建会话、打 `nihaoshijie`、读状态与候选、过期修订号被拒、选第一行上屏「你好世界」、冲刷、释放、从个人词典读回这句。CMake 有 pkg-config 时用它（静态链接直接给出归档路径再接 `Libs.private`，因为前缀里同时有共享库），没有时（Windows）自己读同一个 `.pc` 的 `Libs.private`。

CI 的 `engine-capi` 作业在 `ci-platforms.yml` 里，Linux、macOS、Windows 三个 runner 各跑 `cargo test -p msime-engine-capi` 和这个脚本。`changes` 作业新增 `engine_capi` 输出，只在 `crates/engine*`、`crates/path-trust/`、工作区清单与工具链、`collect-notices.py` 或这个 workflow 改动时为真；这个作业不是必需检查，所以在作业级跳过，不为不相干的改动占一台稀缺的 macOS runner。cargo-c 按版本、系统和工具链缓存装好的可执行文件，缓存不中才 `cargo install cargo-c --locked --version 0.10.26`。

`tests/golden_replay.rs` 把引擎的 golden 场景经 C 函数回放，每一步与核心 API 的回放对照，同一条 `cargo test -p msime-engine-capi` 跑它；怎么比、跳过哪些、回放下限见 [引擎 C ABI 回放 golden 场景](../testing/2026-10-29-engine-capi-golden-replay.md)。

## Alternatives considered

- **cbindgen 加自写脚本生成 `.pc`、设 SONAME** — 最强的理由是少一个构建工具，cbindgen 只是一个库或一个小命令。维护者选了 cargo-c：它一步给出头文件、`.pc`、按平台正确的库名和 SONAME/install name、Windows 的导入库，这些自己写要按三个平台分别处理，正是容易写错又没人测的部分。
- **手写头文件，像 host-api 的 `msime_client.h`** — 最强的理由是注释、分组和顺序完全由人控制，读起来更顺。否掉是因为手写头文件和 Rust 签名之间没有任何机械约束，漂移只能靠人发现；生成加 CI 漂移检查把它变成编译期的事实，注释也从 Rust 文档注释原样带过去。
- **会话固定在建它的线程（host-api 的句柄注册表）** — 最强的理由是与 msime 自己的宿主一致，按线程缓存的连接不会被别的线程碰到。P3 已经让核心 `Session` 成为 `Send`，日志连接改成可跨线程关闭，绑定线程只会把线程管理推给每个采用方（工作线程池、异步运行时）。
- **快照缓存按候选修订号判断过期** — 最强的理由是只读调用之间可以少取几次快照。否掉是因为修订号只承诺候选列表，标点开关这类不递增修订号的调用也会改快照里的 `spelling_symbols`；按「改变状态的调用」清缓存不依赖这个细节。
- **会话不取租约，只在文档里写「会话开着时不调维护函数」** — 草案给的另一条路，C 包装层更简单。否掉是因为采用方和 msime 自己的输入法可能用同一份数据目录，只有同一把文件锁能让它们互斥；而且最容易忘的正是这条文档约定。
- **库版本直接用 crate 版本** — 最强的理由是只有一个版本号。否掉是因为 cargo-c 按库版本算 SONAME：0.x 的 crate 版本会让 SONAME 跟着每个次版本变，和「同一个 ABI 主版本内只追加」的承诺对不上。

## Consequences

- **收益**：第三方前端拿到一个带版本、带头文件和 `.pc` 的 C 库，按键路径上没有 JSON 和额外分配，候选按下标随取，选择有修订号保护；会话可以在任何线程上用；会话与维护按 msime 宿主的同一把文件锁互斥；头文件、SONAME、第三方声明和三平台上的打字流程都有 CI 守着。
- **代价**：cargo-c 总是按显式的目标三元组构建，产物在 `target/<三元组>/` 下，是与 `target/debug` 分开的第二棵构建树（CI 和本机跑脚本时都多编一遍引擎）。苹果平台的 dylib 不剥调试信息，体积大一些。CI 在引擎相关的 PR 上多占三台 runner，其中一台是 macOS。快照缓存在只读调用之间可能多取一次快照。
- **没做的**：ABI 1.0 没有日志回调，1.1 加上了（`msime_engine_set_log_callback`，见 [引擎 C ABI 1.1 补上的缺口](2026-10-30-engine-capi-abi-1-1-additions.md)）；事件通知仍没有；二进制发布见 [msime-engine-capi 的预编译归档、最小数据包与快速上手](../process/2026-10-29-engine-capi-prebuilt-release-and-data-pack.md)，crates.io 等 P6，msime 自己的宿主改用公开 API 是 P7；九键、触点、神经重排等留到 v1.x。数据格式的规格文档（P5）还没写，README 的数据表以提案为准，许可证一列按仓库自己的声明（`resources/licenses/msime-engine-dictionary-NOTICE.md`、`docs/third-party.md`）改正过。Linux 和 Windows 上的 C 示例只在 CI 里跑过，本机只在 macOS（Studio，arm64）上跑过三种链接方式。

## 相关笔记

按 engine-capi、C ABI、cbindgen、cargo-c 检索活跃笔记：[引擎 C ABI 1.1 补上的缺口](2026-10-30-engine-capi-abi-1-1-additions.md) 在本篇的函数面上追加调频、混排、本地模式开关、数字选词、词条检查和日志回调，部分重叠，互链；[核心会话收尾与日志连接的线程归属](2026-10-29-core-session-lifecycle-and-journal-threads.md) 和 [引擎库的一套默认值、错误码、日志出口与维护锁归属](2026-10-29-engine-library-defaults-errors-log-and-lock.md) 是本篇依赖的 P3，部分重叠，互链（后者「没做的」一条由本篇回答）；[脚本插件工具链](../feature/2026-10-26-script-plugin-toolchain.md) 是按 crate 单独定许可证的先例，部分重叠。其余命中的是 host-api 的 `msime_client.h`，与这个 ABI 无关。没有被本篇吸收或取代的笔记。
