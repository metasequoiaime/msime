# Agent Note: 重置学习数据途中被杀时，下一次准备把挪开的原文件放回去

Status: implemented

## Problem

重置学习数据（`crates/engine/src/user_dictionary/reset.rs` 的 `swap`）对每个文件做两次改名：先把原文件挪成 `.<名字>.backup.<时间戳>`，再把 `.<名字>.reset.<时间戳>` 新副本改名成原名。顺序是工作主词库、英文词库、最后日志。进程恰好死在一个文件的两次改名之间时，这个文件只剩在备份里：

- 缺的是代次里的词库：`open_ready_generation` 发现副本不全，之后每次启动都报 `Incomplete runtime dictionary generation`，输入法起不来，只能手动把备份改回原名。
- 缺的是日志：下一次准备当作没有日志，新建一个空日志，学到的词全在备份里没人用，等于重置在用户不知情时完成了一半。

进程内出错时 `restore` 会按相反顺序换回原文件；进程被杀时没有人做这一步。iOS 键盘扩展的内存上限、Android 与 HarmonyOS 回收后台进程都会在任意一步杀掉进程。[词库代次的暂存目录](2026-10-26-stale-generation-staging.md)那次排查把这条记成了遗留问题。

## Decision

准备代次时，在打开日志和词库之前调用 `restore_interrupted_reset`（在 `reset.rs`，命名规则与 `swap` 写在同一处）：

- 目标文件不在（`symlink_metadata` 报 `NotFound`）、同目录里有 `.<名字>.backup.<十进制时间戳>` 普通文件时，选择时间戳最大的备份，以不覆盖已有目标的方式放回原名。只在发布成功后删除同一时间戳没来得及发布的 `.<名字>.reset.<时间戳>`；输掉竞争或发布失败时保留备份和新副本。
- 选择备份与发布是两个步骤，但存在检查不承担互斥：其他准备者可能在这两步之间先恢复文件。发布使用已有 `tempfile 3.27` 的 `TempPath::persist_noclobber`；把现有备份交给 `TempPath` 后先调用 `disable_cleanup(true)`，避免错误结果被丢弃时删除仍可用于恢复的原件。Android、Linux 与 Apple 平台优先使用内核的不覆盖改名，Windows 不启用覆盖标志；不新增依赖。
- 词库路径沿用 `RuntimePaths::dictionary` 的现名／旧名优先级：已有现名或旧名文件均不动；两者都缺失时先找现名备份，找不到再恢复 `assets::LEGACY_NAMES` 中对应的旧名备份，并发布到调用方请求的现名。这样旧代次的 `msime.db`、`english.db` 在被挪开之后仍能被找到，后面的代次回放和反查索引沿用现名入口；清理的 `.reset.` 仍按备份源的旧名定位。
- 调用点：`prepare_runtime_paths_for` 在 `quarantine_corrupt_journal` 之前对日志调用；`open_ready_generation` 在检查副本之前对每个代次词库调用。
- 目标在（包括是符号链接）时什么也不做；放不回去也不报错，由原有准备流程继续处理，词库仍缺失时照常拒绝该代次。

效果是被杀的那次重置没有发生过：已经发布的新副本留着，但日志还是旧的，下一次回放把学到的词写回新副本，与重置之前一致；用户再点一次重置即可。其他说不清来由的残缺代次（例如只有 `.ready`、没有词库）仍然被拒绝。

### 做重置的进程其实还活着时

iOS 可能把做重置的 App 挂起在两次改名之间，另一个进程（键盘扩展）这时准备代次。恢复者成功放回原件后会删除同一时间戳的 `.reset.` 新副本；原重置进程之后若再发布这个副本，会因源文件不存在而失败，并按已有 `restore` 逻辑回滚已发布的其他文件。若原进程抢先发布，恢复者不能覆盖已经就位的目标。

这里保证的是单个文件的恢复不覆盖竞争者已经发布的文件，不把几个文件的备份、恢复、回放宣称为一个原子事务。准备路径没有取得重置操作的 `DictionaryAccess` 维护锁，因此不能用外层已有锁作为跳过不覆盖发布的理由。

## Alternatives considered

- **缺词库时直接从资源目录重新复制一份，再回放日志。** 最强的理由：不依赖备份是否还在，任何原因缺了词库都能自愈；日志是学习数据的来源，新代次本来就是资源副本加回放得到的。不采用：它会改掉「已就绪却残缺的代次一律拒绝」这条现有约定（`staging_failures_clean_up_and_incomplete_generations_are_refused` 守着它），把来历不明的损坏也悄悄盖过去；而这次的问题有确定的原因和确定的原件，放回原件更精确。
- **往前做完：把没发布的 `.reset.` 新副本改名就位。** 也能消除缺文件。不采用：被杀时日志很可能还是旧的（日志最后才换），词库换新、日志不换，下一次回放又把旧学习数据写回去，等于没重置；日志已经挪开时往前做完倒是一致，但两种情况分开处理，不如统一当作没发生、让用户重试，结果也更好预期。
- **发布前再检查一次目标是否存在，仍用普通 `rename`。** 改动最少，也能减少常见竞争。但检查与改名之间仍可插入另一个发布者，Unix 上普通改名会覆盖它，必须由最终发布操作保证不覆盖。
- **按时间戳清理一小时以前的 `.backup.`、`.reset.` 遗留文件。** 能回收空间（主词库的备份约 200 MB）。这次不做：重置进程若被挂起超过一小时、之后又回滚失败，备份被删就会重新落回缺文件的状态，正是这次要修的问题。放回原件之后，遗留的只剩已发布文件的旧备份和没用上的新副本，只占空间，不挡启动。

## Consequences

- **收益**：重置途中被杀不再让输入法永久起不来，也不会悄悄丢掉学到的词；不需要用户手动改文件名。
- **代价**：每次准备对日志和每个代次词库多一次 `symlink_metadata`；文件缺失时多读一次所在目录。
- **已知上限**：备份仍按系统时钟产生的十进制时间戳选择，没有增加跨文件提交协议。`persist_noclobber` 在不支持内核不覆盖改名的平台／文件系统上可能退回硬链接；Android 禁止硬链接时该回退会安全失败并保留备份，但不能据此声称已恢复，必须用设备验证确认支持。被杀的重置留下的其余文件（已发布文件的 `.backup.`、未用上的 `.reset.`）仍然没人清理，只占空间。日志被挪开时它旁边的 `-wal` 不动，放回日志后两者重新配对；重置前所有会话已经关闭，正常情况下没有 `-wal`。

## Verification

- 先写回归测试，在未改的代码上运行：`a_dictionary_left_aside_by_a_killed_reset_is_put_back` 以 `Incomplete runtime dictionary generation` 失败；`a_journal_left_aside_by_a_killed_reset_is_put_back` 因为下一次准备新建了空日志、读不到学过的行而失败。修复后两者通过。前者同时核对：放回的是时间戳最大的备份，更旧的备份原样保留，同一时间戳的 `.reset.` 被删；词库已经在时新出现的备份不动、词库字节不变。
- 在 Mac Studio 上：`cargo test -p msime-engine --locked --lib user_dictionary::` 167 项通过，`cargo clippy -p msime-engine --locked --all-targets -- -D warnings` 无告警。

- 审查补充的确定性交错回归先保留普通 `rename`：两个准备者都见到目标缺失，赢家发布新备份后，迟到者枚举到旧备份再发布，断言明确读到旧字节而失败。旧主词库名与旧英文词库名回归各自在完整准备链路上以 `Incomplete runtime dictionary generation` 失败。修复改用不覆盖发布，并从旧名备份恢复到现名；另核对已有文件的名称优先级、现名备份优先以及失败路径不删原备份和新副本。

- 审查补充修复后，`cargo test -p msime-engine --locked --lib user_dictionary::` 171 项通过；完整 `cargo test -p msime-engine --locked --lib -- --test-threads=2` 为 2684 项通过、57 项忽略、0 项失败；`cargo clippy -p msime-engine --locked --all-targets -- -D warnings` 通过。
- Android API 35 的 `emulator-5580` 上，交叉编译真实引擎测试二进制后经 `adb` 运行 `user_dictionary:: --skip hard_link --test-threads=1`：171 项通过、0 项失败（含全部新增回归），确认设备文件系统支持本次不覆盖恢复路径。本轮未安装 APK，也未在真实编辑器或报告者设备上人为制造杀进程。
- 在更新至 PR 最新集成基线后执行 `bash scripts/verify-local.sh --quick`，Windows 本机原生测试 173 项通过（2 项因宿主差异排除、108 项需 Windows 构建），HarmonyOS 设置页构建通过，契约注册检查通过。契约检查尚未全部汇总时，系统盘可用空间低于 2 GiB，外部磁盘监控主动终止本轮、退出码 125；因此完整 quick 未通过，后续 workspace、Android／Wasm 编译及 Linux／macOS 阶段未在该轮执行，不把此资源中止算作通过。
- `cargo fmt --all --check`、`pnpm run verify-notes`、`git diff --check` 通过。
