# Agent Note: 诊断包把遥测崩溃目录当崩溃日志来源

Status: implemented

## Problem

#5539：Android「开发者选项」里「导出诊断包」点了只弹「诊断包生成失败，请重试」，MCP 上传也因为同一个诊断包而失败。Android 宿主把 `files/telemetry/telemetry-crashes`（`Telemetry` 的崩溃目录）作为 `sources.crash_logs` 传给 `msime_client_diagnostic_bundle`，而 `crates/client-core/src/diagnostics.rs` 的 `read_section` 只接受普通文件，遇到目录返回 `diagnostics_invalid`。键盘每次开会话时 `TelemetryStore::begin_session` 都会建好这个目录，所以只要键盘用过一次，诊断包就一定失败；设计、测试里用的都是一行一条 `{at, message, stack}` 的文件，没人覆盖过宿主真正传的东西。

同一页上 MCP 上传开关点开后要先在「确认上传」里确认，等待确认时开关仍显示为关闭，再点也只是又进一次等待确认，看起来毫无反应；读云端快照状态的 HTTP 请求还放在设置页存储的串行线程上，网络慢时开关的保存都排在它后面。

## Decision

- `sources.crash_logs` 可以是目录：`read_crash_logs` 发现来源是真实目录（`symlink_metadata`，不跟随链接）时交给 `read_crash_directory`，否则仍按行读文件。目录里只读扩展名为 `crash` 的普通文件（与遥测的 `CRASH_EXTENSION` 同一个常量），每个文件最多读 `MAX_CRASH_RECORD_BYTES`（与遥测相同的 64 KiB），格式与遥测写的相同：第一行是异常摘要，其余是栈帧；`at` 取文件修改时间（UTC，秒）。按修改时间保留最近的 `MAX_CRASH_LOGS` 条，多出来的计入 `truncated`，读不出的计入 `dropped`，不让整个诊断包失败。
- 清洗与按行读的记录共用 `sanitized_crash`：`message` 只留异常类型，`stack` 里栈帧原样保留、说明行只留异常类型，路径只留文件名。目录里的记录不套按行记录的长度上限（摘要本来就只留类型，栈由 `clean_stack` 截断），空摘要记成 `unknown crash`，与遥测上报时一样。
- Android 的 MCP 开关由 `McpUploadSwitchPolicy` 决定显示与动作：已上传或等待确认时显示为打开；等待确认时关闭就是取消，已上传时关闭是删除。`DeveloperPage` 读云端状态改用 `HostTask.runNetwork`。

## Alternatives considered

- **宿主不再把目录传进来（传 null）** — 一行就能让诊断包不再失败，也不用碰 client-core；但这样崩溃日志一类永远是空的，开发者选项里「崩溃日志」开关形同虚设，而这正是诊断包最有用的部分。
- **宿主在 Java 里把 `*.crash` 文件转成 jsonl 再传** — 不改共享代码；但清洗规则（只留异常类型、路径只留文件名）就要在 Java 里再写一份，与 client-core 的白名单漂移，违背「上传内容只来自 Rust 写出的诊断包」的约定。

## Consequences

- **收益**：导出诊断包和 MCP 上传在用过键盘的设备上不再失败；目录里有崩溃记录时会进诊断包；按行文件的来源照常可用。
- **代价**：`TelemetryStore::begin_session` 每次开会话都会把目录里的 `*.crash` 收进遥测队列并删掉文件，而 Android 在输入法崩溃后几乎立刻重启它、开新会话，所以这个目录通常是空的，诊断包和 MCP 上传里的「崩溃日志」一节多半什么也没有。这次只修了「诊断包一定失败」；要让这一节真正有内容，需要遥测删文件前另存一份有上限的、已清洗的最近崩溃摘要供诊断包读取，这会改变崩溃数据在本机的保留期，留给单独的改动决定。`at` 是文件的修改时间而不是崩溃时刻的精确时间（两者在写崩溃记录时几乎相同）。

## Verification

`cargo test -p msime-client-core --lib diagnostics`（`a_telemetry_crash_directory_is_read_as_crash_logs`、`a_crash_directory_keeps_only_the_newest_records`）；Android 开关见 `platforms/android/tests/settings/McpUploadSwitchPolicySmoke.java`，由 `bash platforms/android/check-host.sh` 运行。
