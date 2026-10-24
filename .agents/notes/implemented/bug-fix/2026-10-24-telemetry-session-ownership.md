# Agent Note: 遥测会话按 id 归属，IBus 宿主收到 SIGTERM 时正常结束会话

Status: implemented

## Problem

Linux 近 30 天的匿名遥测里，0.10.1 的 `session_crash`（552）比 `session`（525）还多，0.11.0 的崩溃会话占比约 41%，0.12.x 与 0.13.0 只有 3–4%。逐项核对后，这些数字由三件事叠加而成，其中两件到今天的代码里还在。

一、真实崩溃，已在 0.12.0 修复。0.10.1 与 0.11.0 的 `crash` 事件绝大多数属于 `std::terminate: nlohmann::...::type_error 306` 一组（约 1840 条），就是 [Linux IBus 宿主的 glib 回调不得对 null 视图调用 value()](2026-10-10-linux-glib-callback-null-view.md) 修掉的那个 abort。崩溃守护每次把宿主拉起来都是一个新会话，同一台设备在几天里反复崩溃几百次（0.10.1 的 552 条里有 505 条来自两台设备），所以 `session_crash` 远多于正常结束的会话。0.12.x 与 0.13.0 里每条 `session_crash` 都有同一会话 id 的 `crash`，反之亦然，计数本身是一致的。

二、同一目录里有两个宿主进程时，会话归属错乱（现存）。`TelemetryStore` 只有一个会话标记 `telemetry-session.json`，后开始的进程会覆盖先开始的进程的标记；而 `end_session()` 和 terminate 路径的 `record_crash()` 都按「标记里现在是谁」找会话。于是：

- 一次 terminate 写两份记录：terminate 回调按标记写到别的进程的会话名下，随后 `abort()` 触发的 SIGABRT 处理函数按自己 begin 时拿到的路径再写一份。下次启动给那个没崩的会话报 `session_crash`，真正崩溃的会话只剩一条没有 `session_crash` 的 `crash`。线上 0.11.0 有 43 条这样的孤儿 `SIGABRT: aborted`（20 台设备），每一条都和同一设备、同一次发送里的一条 `type_error 306` 成对出现；0.10.1 也有同样的成对样本。
- 先退出的进程把后开始的进程的会话当作自己的结束、删掉它的标记；后者之后崩溃时没有标记可对，`session_crash` 丢失，自己的 `session` 也不再报。

同一用户的两个图形会话（本机加远程桌面）各起一个 `ibus-daemon`，两个宿主共用 `$XDG_STATE_HOME/msime`；重复拉起的宿主在争组件名失败退出之前也已经开始过会话。线上数据只能证明标记被别的会话改写过，分不出是哪一种。

三、IBus 宿主大多数正常结束不报 `session`（现存）。`ibus-daemon` 停止组件时向启动器发 SIGTERM（ibus `bus/component.c` 的 `bus_component_stop()`），启动器原样转给宿主；注销和关机也是 SIGTERM。宿主没有处理它，按默认动作直接退出，主循环之后的 `telemetry::end()` 不会执行，只有碰巧先等到总线断开的那部分会话会被记下。这让崩溃率的分母偏小：0.11.0 有 647 台设备报过 `active`，只有 391 台报过 `session`。Fcitx5 自己把 SIGTERM 转成正常退出，插件析构时会结束会话，不受影响。

## Decision

- `TelemetryStore` 在 `telemetry-state.json` 里记着尚未结束的会话（`open_sessions`，最多 16 个，超出淘汰最旧的）。`begin_session` 为其中每个留下了自己崩溃记录的会话补报一条 `session_crash`（版本取自那个会话），并把它移出列表；只留下标记的会话照旧不算崩溃。旧版本只写了标记、没有列表时，标记里的会话按未结束会话处理，升级后第一次启动仍能认领它的崩溃记录。
- `SessionStart` 新增 `session_id`。新增 `end_session_by_id` 与 `record_crash_for`：前者只结束给定的会话，标记只在仍属于它时才删；后者把记录写到这个会话自己的文件，不看标记。Host API 的 `msime_client_telemetry_end` 与 `msime_client_telemetry_record_crash` 各加一个可选的 `session` 字段；不给时保持原来按标记的行为，macOS、iOS、Android、HarmonyOS 的宿主不需要跟着改。
- `platforms/common/Telemetry.cpp`（Linux 两个宿主和 Windows Server 共用）记住 begin 返回的 `session_id`：`end()` 只结束自己的会话，本进程没有会话（上报关着）时不调用；`record_terminate()` 写自己会话的记录，与信号处理函数写的是同一个文件（`O_EXCL`），所以 terminate 之后的 SIGABRT 不再记第二份。
- IBus 宿主进入主循环前为 SIGTERM、SIGHUP、SIGINT 装处理函数（已被忽略的保持忽略）。处理函数只写管道唤醒主循环并调用 `alarm(5)`；主循环据此 `ibus_quit()`，走完收尾和 `telemetry::end()` 后把该信号恢复为默认动作再 `raise()`，启动器和 `ibus-daemon` 看到的退出状态与以前相同。主循环卡住时管道没人读，5 秒后 SIGALRM 的默认动作结束进程；`SA_RESETHAND` 让第二个同样的信号立即生效。

## Alternatives considered

- **每个会话一个标记文件，用 `flock` 判断它的进程是否还活着**：最强的理由是能准确区分「仍在运行」和「被杀掉」，不需要按数量淘汰。但锁要在进程整个生命周期里由 Rust 持有一个文件句柄，Host API 现在是无状态的 JSON 调用，六个平台的宿主都要多一个全局句柄的生命周期约定；Android 与 iOS 的进程模型下持锁行为也要各自验证。按 id 认领已经让「谁崩了、谁结束了」不再串，剩下只是被杀掉的会话在列表里多待一会儿，代价很小。
- **把 IBus 宿主的 `telemetry::begin` 挪到成功拿到组件名之后**：能让争组件名失败、立刻退出的重复宿主不开始会话，不改写标记。按 id 认领之后，这个重复宿主的会话既不会顶掉别人的会话，也不会被记成崩溃（它没有崩溃记录），挪动只剩缩小崩溃捕获窗口这一个效果：连 ibus-daemon 之前的崩溃就不再记录。所以不挪。
- **用 `g_unix_signal_add` 处理 SIGTERM**：代码最少。但它的回调在主循环里执行，主循环卡住时 SIGTERM 就再也结束不了进程，启动器会一直等下去；自己装处理函数才能加上 `alarm()` 期限。
- **只在服务端修正**：服务端的无崩溃率公式已经是 `session/(session+session_crash)`，问题出在客户端报上来的分子分母本身，服务端没法从事件里还原会话归属，也补不回没有上报的 `session`。服务端可做的只是展示层面的补充（按设备去重的崩溃率等），不能替代客户端修正。

## Consequences

收益：一次崩溃只产生一份记录、一条 `crash`、一条归属正确的 `session_crash`；正常结束的会话不再因为同目录的别的进程而丢失或被错记；IBus 宿主被 `ibus-daemon`、注销和关机结束时也报 `session`，崩溃率的分母不再只含少数碰巧先断开总线的会话。

代价：发布后 Linux 的 `session` 数会明显上升，0.14 起的无崩溃率与 0.13 及以前不可直接比较。`telemetry-state.json` 多一个最多 16 项的列表；同目录里同时运行超过 16 个会话的极端情况下，最旧会话之后的崩溃只报 `crash`。SIGTERM 之后宿主多花几十到几百毫秒做收尾（键入计数落盘最多等 2 秒），最长 5 秒后被 SIGALRM 结束。

缺口：0.12.x 与 0.13.0 剩下的崩溃组标题只有 `SIGABRT: aborted` 与 `SIGSEGV: segmentation fault`，不看单条事件的栈无法定位，这次没有处理。其中不带 `(code N)` 的 SIGSEGV 说明信号的 `si_code` 不大于 0，即由 `kill`/`raise` 一类用户态调用发出，而不是内存访问错误本身，值得先确认是谁发的信号。

## Verification

- `cargo test -p msime-client-services --lib telemetry`：35 项通过。新增的同目录并发会话用例覆盖 terminate 只记一份、只为崩溃的会话补报、按 id 结束不动别人的标记、被挤出列表的会话只报 `crash`、旧版本留下的标记仍被认领。
- `cargo test -p msime-host-api --lib telemetry`：ABI 用例核对 `session_id` 与崩溃记录文件名一致、带 `session` 的 end 只结束一次。
- `platforms/common/tests/telemetry.cpp`（ctest `common-telemetry`）在子进程里开始会话、经 Host API 模拟同目录的另一个宿主、再触发真实的 `std::terminate` 与 SIGABRT：把四个源文件换回 develop 上的版本重编后，它在 `crashed.size() == 1` 处失败（同一次崩溃得到两条 `crash`）；修复后只有一条，`session_crash` 与它同属崩溃的那个会话、版本是崩溃进程的，随后的正常结束只结束自己的会话、不动别的进程的标记。
- `platforms/linux/tests/core/ibus_startup_telemetry.py` 向真实的 `msime-linux-ibus` 发 SIGTERM：修复前队列里没有 `session`，修复后有本会话的 `session`，进程仍以 SIGTERM 退出，标记被删、没有崩溃记录。
- `bash platforms/linux/build-container.sh` 在容器里编译 IBus engine、Fcitx5 插件与全部单测并跑 ctest：111 项无失败（`linux-ibus-demand-activation` 照旧跳过），包括 `linux-ibus-launcher-supervisor`。

## Related notes

检索了活跃笔记里的遥测、崩溃记录、会话与 SIGTERM：[Linux IBus 宿主的 glib 回调不得对 null 视图调用 value()](2026-10-10-linux-glib-callback-null-view.md) 是 0.10.1/0.11.0 崩溃本身的修复，属于部分重叠，这篇引用它解释历史数据；[遥测崩溃清理绑定目录句柄](2026-10-09-telemetry-crash-cleanup-directory-handle.md)、[遥测缓存目录句柄](2026-10-09-telemetry-cache-directory-handle.md) 与 [遥测退避拒绝超出上限的未来截止时间](2026-10-21-telemetry-retry-clock-rewind.md) 分别持有存储访问边界和退避规则，与会话归属无关，不改动；[macOS 输入法被 SIGTERM 结束时照常结束使用统计会话](2026-10-24-macos-session-on-sigterm.md) 是同一个分母问题在 macOS 上的修复，属于部分重叠：IBus 宿主这里沿用它的约束（只接管默认处置的信号、卡住时仍按期限退出、退出状态不变），但因为 GLib 主循环要走完收尾，改用管道唤醒加 `alarm()` 期限。没有被本篇吸收的旧提案。
