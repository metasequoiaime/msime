# Agent Note: 落定重排模型接线状态写进诊断日志

Status: implemented

## Problem

#10341 的报告者在 macOS 0.55.0 打开「桌面神经联想」并装好 `settled-model` 资源包后，看不出模型有没有加载：诊断日志里只有 `focus_in` 和 `preferences_applied`，模型相关的一行都没有。他怀疑 IMK 宿主传的 `preferences_directory` 为空或不对，`settled_model_file()` 因此静默返回 `None`。

维护者经静态 C ABI 做过隔离对照（自动查找、显式路径、会话启动后安装三种布局都加载了模型并移动了首选），代码里也找不到路径遗漏，但报告者的原生 IMK 进程没有验证过。缺的是证据：共享层在以下几种情形下都什么也不写，诊断日志无法区分。

- 没有随包模型，状态目录下也没有已安装的资源包（`settled_model_file` 返回 `None`，按设计是常态）。
- 宿主没给绝对路径的 `preferences_directory`，资源包根本不会被找（`state_root?` 短路）。
- 模型文件在，但 `bounded_file::open_private` 或 `bounded_file::read` 失败（符号链接、FIFO、不可信的多链接、超过 64 MiB、权限）：`sentence_model()` 用 `.ok()` 吞掉了这些错误，只有 `SentenceModel::load` 失败会报 `sentence model ignored`。
- 模型加载成功：也没有任何一行。

另外，同一路径的模型在进程内只加载一次（`SENTENCE_MODELS` 缓存），读失败只在第一次加载时有机会报告，之后的会话即使补上报告也只会拿到缓存里的 `None`。

## Decision

只加可观测性，不改模型查找、加载与排序行为。

- `ffi::sentence_model()` 把 `open_private` 和 `read` 的错误（`NotFound` 除外）报告为 `sentence model unreadable: <路径>: <错误>`。`NotFound` 仍不报告：键盘模型只有带它的版本才有，落定重排模型在调用前已经查过文件在不在。
- `report_settled_model`（`crates/host-api/src/lib.rs`）按会话当前状态写一行，类别固定：
  - `settled model attached`：模型已加载并挂在会话上；
  - `settled model not loaded`：找到了文件却没能用上，原因见同一次加载的 `sentence model unreadable` / `sentence model ignored`；缓存命中的后续会话只有这一行；
  - `settled model missing`：有状态目录，但没有随包模型也没有已安装的资源包；
  - `settled model missing without state directory`：没有随包模型，且会话选项里没有绝对路径的 `preferences_directory`。
- 报告时机只有三处，都在低频的会话事件上：`create_session`（仅 `SessionUse::Input`）、`apply_pending` 里后台加载完的模型换上时、`apply_pending` 重建时 `neural_desktop` 由关变开且没有在途加载时。按键路径和停顿评分路径（`start_settled_rerank` / `poll`）上不写。
- 开关关着时三处都不写，预览会话也不写，关着开关的用户日志里没有新行。开关打开那一刻要再报一次，因为 macOS 宿主用 `runtime-options.json` 里嵌的偏好快照建会话，`preferences.json` 的值要大约一秒后经 `updatePreferencesSnapshot` 送到；报告者看到的 `runtime-options.json` 里 `neural_desktop: false` 就是这个快照。
- `HostSession` 新增 `settled_model_attached`，记录 runtime 上是否真挂着模型。不读 runtime 自己的槽：后台评分进行中模型被评分任务借走（`settled_reranker.take()`），槽是空的，会误报 `not loaded`。
- 这些是状态行，不是失败；`diagnostics.rs` 的模块文档同步扩大到覆盖这类低频状态行。类别是不带冒号的固定英文短语，macOS（`input_method_main.mm` 的出口）只记 `host_api: <类别>`，Windows Server 的 `logs\server.log` 同样只记类别，Linux 没登记出口时连同细节写 stderr。

### 测试

`crates/host-api/src/tests.rs` 的三条测试经登记的诊断出口取行，覆盖：装好、没装、没有状态目录、开关关着不写、文件超过读入上限（稀疏文件）时的 `unreadable` 加 `not loaded`、会话启动后安装并聚焦换上、开关由关变开时补报，以及无变化时不重复。诊断出口是进程级的，登记它的测试共用 `DIAGNOSTIC_SINK_SERIAL` 串行，守卫在测试结束时清掉出口；断言只按各自的临时目录过滤，不受并行测试的报告干扰。原有的 `plugin_failures_reach_the_registered_diagnostic_sink` 改用同一套共享捕获。

## Alternatives considered

- **按 issue 的猜测改路径查找或在宿主侧强制传 `preferences_directory`。** 最强的理由是报告者的症状与「状态目录没传到」完全吻合，改了可能直接修好。不用：v0.55.0 标签和 develop 都有资源包回退，macOS `prepareSession` 的会话选项保留了该目录，C ABI 对照也没复现；没有证据的修复可能修错地方，还会让真正的原因继续不可见。先拿到 `missing without state directory` 或 `attached` 这类行再决定。
- **每次停顿评分都写一行（是否评分、是否移动首选）。** 能直接回答「为什么首选没变」。不用：IMK 每个文本客户端一个会话、每次停顿都会触发，日志量大；可信词典首选跳过评分等常态也会被写成噪音。若报告者的日志显示 `attached`，下一步再在 macOS `scheduleSettledRerank` / `finishSettledScore` 加按轮询的状态行。
- **只在开关打开时报告缺失，加载成功不报告。** 行数最少。不用：没有 `attached` 这一行，日志里「没有任何行」又回到和「宿主根本没走到这里」分不开的状态，正是 #10341 要消除的歧义。

## Consequences

- 收益：报告者打开诊断日志、切换一次输入框，就能给出「没装 / 没有状态目录 / 读不了 / 已接上」之一，不必再靠 atime（APFS 上普通读取不一定更新它）。Windows、Linux 宿主无需改动即获得同样的行。
- 代价：诊断模块原先只报失败，现在多了一类成功状态行，约定稍有放宽；开着开关时每个会话建立都会写一行 `attached`，IMK 一个文本客户端一个会话，日志开着时行数随打开的输入框增加，但日志是用户主动打开的。
- 本改动不修任何已复现的缺陷，#10341 在拿到报告者带这些行的日志前保持打开。
- 相关：[Windows host-api 诊断出口](../feature/2026-10-10-windows-clipboard-privacy-panel-statistics-host-diagnostics.md)、[macOS 诊断日志启动顺序](2026-10-10-macos-diagnostic-startup.md)。
