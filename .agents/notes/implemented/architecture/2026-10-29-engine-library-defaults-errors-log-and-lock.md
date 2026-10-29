# Agent Note: 引擎库的一套默认值、错误码、日志出口与维护锁归属

Status: implemented

## Problem

引擎要做成给其他输入法嵌入的库（PR #10167 的提案 `proposed/architecture/2026-10-29-embeddable-engine-library.md`，P3「ABI 稳定之前引擎侧要修的硬点」；本分支的基点早于它合入）。[核心会话收尾与日志连接的线程归属](2026-10-29-core-session-lifecycle-and-journal-threads.md) 修完了生命周期和线程那几项，剩下四项与对外契约有关：

- 两套默认值。核心 `SessionOptions::new` 照参照实现默认辅助码开、表名 `lantian`、学习开；host 门面的 `prepare_options` 默认 `ziranma`、学习关，`session_options` 再从 `SessionOptions::new` 出发改写大部分字段，没写到的字段（`personal_context`）悄悄继承核心默认值。核心默认值一改，产品就跟着变。另外随仓库的七张辅助码表里只有 `wubi86` 写明了来源与许可，`lantian` 和 `ziranma` 都没有再分发授权（`resources/helpcodes/NOTICE.md`），引擎库默认去读它们不合适。
- 错误只有字符串。`EngineError` 四个变体都只给说明文字，`KeyResult.diagnostic` 是 `diagnostics.rs` 常量的自由文本，C ABI 要按数字返回。
- 日志直接写标准错误。嵌入方接不住引擎记的日志。
- 跨进程的词库维护锁在 GPL 的 `msime-client-services` 里（`dictionary/access.rs`，加上 `msime-client-base` 的 `file_lock.rs`、`storage.rs` 片段）。引擎层的维护接口（改个人词库、重置学习）自己不取锁，只在文档里写「调用方要先让所有会话停下」，直接用引擎库的采用方没有办法和 msime 自己的进程协调。

## Decision

### 一套默认值

`SessionOptions::new` 是引擎库唯一的默认值，文档写在它上面。大部分沿用参照实现；与参照实现不同的只有辅助码：`helpcode = false`，`helpcode_schema = "wubi86"`。表名也改，是因为会话建立时不论开没开辅助码都按表名读一次表，默认表名指向一张没有授权的表，引擎库就默认读它；嵌入方要辅助码就自己打开并选表。学习、个人上下文照旧默认开。

产品不依赖这套默认值。`host::options::session_options` 改成逐字段的结构体字面量，不再从 `SessionOptions::new` 出发：核心层加字段时门面不写就编译不过，`personal_context` 写明为 `true`（桥接层从没把它交给宿主，产品一直是开）。`prepare_options` 里的产品取值（`ziranma`、学习关等）不动，它们是 `EngineOptions` 的值，宿主照旧按偏好覆盖。engine-wasm 直接用 `SessionOptions::new`，原来就显式关辅助码，辅助码表由页面交来，表名只决定在没有文件系统的 wasm 上读一次必然失败的文件，行为不变。

golden 基准按参照实现录制，`tests/golden_support/replay.rs` 的 `reference_options()` 在库默认值上补回 `helpcode = true`、`lantian`；`session/tests.rs` 与 `session/learning.rs` 的测试夹具同样补回，测试内容不变。

### 错误码与诊断码

`ErrorCode`（`#[repr(u32)]`）是 `EngineError` 的稳定数字码，经 `EngineError::code()` 取得，说明文字照旧是 `Display`：`InvalidArgument = 1`、`Failed = 2`、`Storage = 3`（`Sqlite` 与 `Io`）、`Busy = 6`。编号与 C ABI 草案的返回码共用一个空间，0 是成功，4（不支持）、5（revision 不符）、7（会话已毒化）、8（空指针）留给 C ABI 自己的情形，引擎不产生它们。新增变体 `EngineError::Busy`，现在只有维护租约冲突用它（`DICTIONARY_MAINTENANCE_BUSY`，"Dictionary maintenance busy"）。

`diagnostics::DiagnosticCode`（`#[repr(u16)]`）给 `diagnostics.rs` 里「KeyResult diagnostics」「Local mode query diagnostics」两段的 24 个常量各一个号（1 到 24，按文件里的顺序），九键笔画筛选拿不到笔画词库时放进诊断的 `LANGUAGE_DICTIONARY_UNAVAILABLE` 是 25（它定义在宿主门面错误那段，因为同时是查语言词库接口的错误文字），映射表是 `DIAGNOSTIC_CODES`，`KeyResult::diagnostic_code()` 按文字查表。表外的文字是 `Other = 0xFFFF`：注音的选字和按键会把底层错误原文放进诊断。`KeyResult` 不加字段，免得改动所有构造处和快照；编号只追加、不改、不复用。测试有两条：两段里的常量逐条要有码；引擎源码里每个 `with_diagnostic(...)` 实参引用的常量也要有码，不管定义在哪一段，按分段数常量的写法就漏掉过 25 号。

### 日志出口

`msime_engine::logging::set_log_sink(Option<LogSink>)`，`LogSink = Arc<dyn Fn(&str) + Send + Sync>`，进程级，`None` 回到缺省。缺省写标准错误并加 `msime: ` 前缀，与原来的 `eprintln!` 逐字节相同。出口在读锁外调用，出口里再换出口不会死锁。非测试代码里原来只有四处 `eprintln!`（日志不是数据库被挪开、五笔反查索引建不了、下载的五笔码表并不进代次、辅助码表读不到），全部改走 `logging::warn`，每一行都是引擎兜住了的警告，所以出口不分级别。`lib.rs` 对非测试代码 `deny(clippy::print_stderr)`，以后的日志只能走出口；测试和基准测量照旧直接打印。

### 维护锁归属

`DictionaryAccess` 搬进引擎（`crates/engine/src/access.rs`，`msime_engine::DictionaryAccess`、`msime_engine::access::LOCK_FILE`），连同它要的目录句柄、锁文件打开和文件锁片段。锁文件名 `.msime-dictionary-access.lock`、两个根目录（`user_data` 与代次目录）先规范化排序去重再逐个取锁、取不到就放掉已取的、会话共享维护独占、只试不等、Unix 上经目录句柄打开并显式解锁，全部与搬之前相同，新旧版本的进程照样互斥（测试 `interoperates_with_a_lock_taken_on_the_same_file_names` 直接对同名文件取锁核对）。引擎因此在 Unix 上多依赖工作区已有的 `rustix`（`fs` 特性）。

引擎层的维护接口自己取独占租约：`edit_personal_dictionary`、`edit_bundled_dictionary_entry` 先做不碰磁盘的校验，再取租约；`reset_learned_data` 在建好两个目录之后、动任何文件之前取租约。取不到返回 `EngineError::Busy`，打不开目录或锁文件时改词库报原来的「存储不可用」，重置报 `Io`。`host` 门面的 `dictionary_edit`、`dictionary_edit_batch`、`reset_learned_data` 调用的 `_with` 版本不取锁，它们的调用方 host-api 先取租约再进来，批量导入、个人词库同步还要把一个租约跨多次编辑持住。

`msime-client-services` 的 `DictionaryAccess` 改为包一层引擎的租约，`try_session`、`try_maintenance`、`directory` 的签名和语义不变，host-api 一行不改。`directory` 在 Unix 上用引擎交出的、取锁时打开的根目录句柄（`msime_client_base::file_lock::PrivateDirectory::from_opened`），仍绑定在取锁时的那个目录上；非 Unix 的目录句柄本来就是确认过的路径。`msime-client-base` 的 `file_lock.rs`、`storage.rs` 其余调用方很多，原样保留，引擎里是同样规则的一份。`msime-client-services` 因此依赖 `msime-engine`：host-api、桌面设置应用、mcp-server 本来就链接引擎，只有 pack-tool 多编一份。

核心 `Session` 不取共享租约。golden 的 `dict_edit` 和不少引擎测试在会话开着时改个人词库，核心会话一取共享租约它们就全部报忙；采用方的会话与维护由 C ABI 的包装层互斥：C ABI 的会话持共享租约，见 [引擎的 C ABI](2026-10-29-engine-capi.md)。

## Alternatives considered

- **门面继续从 `SessionOptions::new` 出发，只把核心默认值改成产品值** — 改动最小，一套默认值也成立。否掉是因为产品值（`ziranma`、学习关）恰好是引擎库不该有的默认值：`ziranma` 没有再分发授权，学习关会让嵌入方以为学习坏了；而且门面继承默认值的写法下次改核心默认值还会悄悄改到产品。
- **辅助码默认关，表名保留 `lantian`** — 和参照实现差得最少。否掉是因为表名决定建会话时读哪张表，关着也读；默认读一张没有授权的表，和数据许可证的结论相悖。
- **`KeyResult` 加一个 `diagnostic_code` 字段** — 取码不用查表。否掉是因为所有构造 `KeyResult` 的地方和 golden 快照都要改，按文字查 25 行的表只在出诊断时发生。
- **日志出口带级别（`Fn(Level, &str)`）** — C ABI 的回调通常带级别。现在四条日志都是警告，加一个只有一个取值的枚举没有用处；以后真有别的级别时 Rust 侧加一个新的设置函数。C ABI 1.1 的回调同样不带级别，取舍见 [引擎 C ABI 1.1 补上的缺口](2026-10-30-engine-capi-abi-1-1-additions.md)。
- **只在 ABI 里写明「有会话打开时不调用维护接口」，锁留在 client-services** — 草案给的另一条路，不用搬代码。否掉是因为采用方和 msime 自己的宿主可能用同一份数据目录（同一台机器上的另一个前端），只有同一把文件锁能让它们互斥。
- **`client-services` 直接 `pub use msime_engine::DictionaryAccess`，去掉 `directory`** — 少一层包装。`directory` 现在只有它自己的测试在用，但它是这个类型对外的一部分，返回的 `PrivateDirectory` 属于 client-base，引擎不能依赖 client-base；包一层能保住签名，不必顺手删接口。
- **核心层的编辑接口也不取锁，只给采用方一个 `DictionaryAccess` 自己取** — 和门面的做法一致。否掉是因为采用方最容易忘的就是这一步，任务本身也要求引擎层的维护接口自己取锁；需要跨多次编辑持锁的调用方（host-api）用门面的 `_with` 版本。

## Consequences

- **收益**：引擎库的默认值有一处文档、不读没有授权的辅助码表；产品取值逐字段写在门面里，以后改库默认值不会改到产品。宿主和 C ABI 能按数字处理错误和诊断。嵌入方能接住引擎的日志。引擎层的维护接口与 msime 各版本的宿主进程按同一把文件锁互斥。
- **代价**：辅助码默认关是对直接用 `SessionOptions::new` 的调用方的行为变化（仓库里只有 engine-wasm，它原来就关；golden 与测试夹具已补回参照值）。`EngineError` 多一个变体，按变体穷举匹配的外部代码要补一支（仓库里没有）。引擎多一份与 client-base 同规则的目录与锁代码，两边改规则要一起改。pack-tool 的编译图多了引擎。
- **没做的**：核心会话不取共享租约（C ABI 的会话在包装层取）；字节与标量单位的统一、辅助码注释规则下放到核心层仍在 P3 的剩余项里；CommitRaw 学习策略已由 [回车学词策略进核心](2026-10-29-engine-core-raw-commit-policy-and-scheme-data-sets.md) 下放。

## Verification

- 错误码：`error.rs` 的 `error_codes_are_frozen` 按变体核对 1、2、3、3、6，`codes_keep_the_message` 核对说明文字不变。诊断码：`diagnostics.rs` 的 `diagnostic_codes_are_frozen_and_round_trip` 核对编号连续且来回一致、表外文字是 `Other`，`every_key_result_diagnostic_constant_has_a_code` 逐条核对两段常量，`every_constant_passed_to_with_diagnostic_has_a_code` 扫引擎源码里 `with_diagnostic` 的实参（把 25 号的文字临时改掉，它报出 `nine_key.rs` 那一处）；九键到达按键上限的测试多核对一次 `diagnostic_code()`。
- 默认值：`host/tests.rs` 的 `session_options_do_not_inherit_the_library_defaults` 核对库默认值（辅助码关、`wubi86`、学习与个人上下文开）不漏进门面映射；原有的 `session_options_map_every_host_field` 照旧通过；golden 全部场景在 `reference_options()` 下不改基准通过。
- 日志：`logging.rs` 的测试换上出口后收到不带前缀的整行，换回后不再收到。
- 维护锁：`access.rs` 搬来了原 client-services 的全部锁测试（共享与独占互斥、描述符副本不延长租约、第二个根目录忙或锁文件不对时放掉第一把、拒绝符号链接根目录、目录句柄在根目录被替换后仍指向原目录），加上与旧版本同名文件互斥、相对路径、硬链接锁文件；client-services 保留经包装层取锁与 `directory` 的测试。引擎、client-base、client-services、host-api、input-runtime 的测试和 clippy 经 `rbuild` 在 Studio 上跑，命令与结果写在 PR 里；`cargo clippy -p msime-engine-wasm --target wasm32-unknown-unknown` 覆盖非 Unix 分支的编译。引擎层三个维护接口（`personal.rs`、`bundled.rs`、`reset.rs`）各有一条测试：会话持共享租约时报 `Busy`、不写数据，非法输入仍先报校验错误。Windows 分支由 CI 的 Windows 作业覆盖。Android 分支（`O_PATH` 逐级打开、`flock`）原来靠 client-services 的锁测试在设备上跑，搬进引擎后 `platforms/android/tests/device/build-core-test.sh` 加跑引擎的 `access::` 与 `maintenance_lease` 两组单测（`run-core-test.sh` 因此接受测试名过滤参数），合并前在 API 28 专用 AVD 上跑过，结果写在 PR 里；用标准库取锁的同名文件互斥测试不在 Android 上编译，那里新旧两边本来是同一个 `flock`。文件锁的标准库调用按函数分到 `cfg(not(target_os = "android"))`，`scripts/test-file-locking-goes-through-client-core.py` 照常扫引擎，靠的是它对 Android 不编译的函数的既有豁免。
