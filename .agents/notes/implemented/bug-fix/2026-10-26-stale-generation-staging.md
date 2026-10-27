# Agent Note: 词库代次的暂存目录和 n-gram 临时副本被中途杀掉的进程留下后不再挡住启动

Status: implemented

## Problem

今天在全新的 iOS 模拟器上，第一次启动在准备词库代次的途中被杀，`MSIME/user/dictionaries/<content id>.incoming` 留了下来；之后键盘每次启动都报 `Runtime dictionary staging directory already exists`，直到手动删掉这个目录。真机上同样会发生：iOS 键盘扩展内存上限很紧，第一次复制约 200 MB 词库时常被系统杀掉；Android 与 HarmonyOS 的进程退到后台也会被回收。

`prepare_runtime_paths_for`（`crates/engine/src/user_dictionary/generation.rs`）在固定名字的 `<id>.incoming` 里暂存代次，用独占的 `fs::create_dir` 建它，已存在就报 `RUNTIME_STAGING_EXISTS`。进程内出错时会删掉暂存目录，进程被杀时没有人删，于是这个固定名字永远被占着。这一行为照搬自参考实现：`metasequoiaime/msime-engine` 提交 `a9b9f09` 的 `core/runtime_paths.cpp`（代码注释里的 `RP:`）第 155-157 行写着 “Exclusive directory creation also prevents overlapping preparation for the same ID”，第 174-179 行在异常时 `remove_all` 暂存目录。RP:155-157 要的是同一个 id 的两次准备不会写进同一个暂存目录，报错只是 C++ 达到这一点的手段；它没有考虑进程被杀，也让并发准备的一方直接失败。

Android 早在 #4282 就碰到过同一个问题，在 `Bootstrap.clearInterruptedStaging` 里于首次准备前删掉 `*.incoming` 绕过；iOS、HarmonyOS、桌面宿主都没有这层绕过，而且这个问题本来属于引擎。

同一个文件里还有一处同类的固定名字：`stage_generation_copies` 给已就绪的代次补后来才随资源发布的 n-gram 表时，先删掉固定的 `<表名>.incoming`，再独占新建它、复制、改名就位。两个进程同时补同一张表时，一方会删掉另一方正在写的文件，另一方随后的改名要么找不到文件（`No such file or directory`，准备失败），要么把对方写到一半的副本改名就位；后者会让映射这张表的会话读到半截数据。

## Decision

引擎改为每次尝试一个暂存目录，发布靠改名，被丢下的暂存目录按“多久没写”清理：

- **每次尝试独占新建自己的暂存目录。** 名字是 `<id>.<随机串>.incoming`，由 `tempfile::Builder::tempdir_in` 独占新建，重名就换一个随机串。两次准备绝不共用一个暂存目录，RP:155-157 要的保证仍在；名字每次不同，被杀留下的目录不再挡住任何一次准备。出错时丢弃 `TempDir`，只删这次自己的目录。content id 只含 `[0-9A-Za-z_-]`，不含 `.`，所以名字能无歧义地拆回 id。
- **整套做完才改名成 `<id>`。** `.ready` 在改名前写好并落盘，读的一方要么看不到 `<id>`，要么看到带 `.ready` 的完整代次。`<id>` 已经存在且非空时改名一定失败：Unix 的 `rename(2)` 报 `ENOTEMPTY`/`EEXIST`；Windows 上标准库先调 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`，目录目标拒绝访问，再用 `FileRenameInfoEx` 的 POSIX 语义重试，非空目录报 `ERROR_DIR_NOT_EMPTY`。所以输掉的一方顶不掉别人已发布的代次。
- **这次暂存失败、而 `<id>` 已经就绪时，用就绪的那一份。** 失败的原因可能是并发准备的另一方抢先发布（改名失败），也可能是这次停顿太久、暂存目录被当成丢下的清掉了。两种情况都走与一开始就看到就绪代次相同的路径：检查副本、并五笔表、回放日志、补 n-gram 表。回放本来就是每次打开就绪代次都要做的，重复一次无害；若失败是回放本身出错，就绪路径会以同样的错误失败，不会把错误吞掉。
- **每次准备（包括走就绪路径的）先清掉被丢下的暂存目录。** 对象是 `dictionaries` 下名字形如 `<id>.incoming`（旧版本的固定名字）或 `<id>.<字母数字>.incoming` 的真实目录，任意 id 都算：内容 id 换代后旧 id 的半截目录也要有人收。判据是目录本身和里面每一项的修改时间都早于一小时前；读不到时间、时间在将来、读的同时有项目消失，都当作还有人在写。被判为丢下的目录先改名成 `<同名>.removing` 认领，再 `remove_dir_all`；`.removing` 不等变旧，见到就删。清理出任何错都忽略，不影响准备本身。

### 已就绪代次里补 n-gram 表

`stage_generation_copies` 用同样的办法：每张表先写进这次独有的 `<表名>.<随机串>.incoming`（`tempfile::Builder::make_in` 配合原有的 `create_private_file`，仍是 `O_NOFOLLOW` 加独占新建，重名换名），`sync_all` 后关闭句柄、改名成 `<表名>`；出错时丢弃 `TempPath`，只删自己的临时文件。几方同时补同一张表时，Unix 上后改名的一方以同样的字节替换前一方的文件，已经映射前一份的会话持有旧 inode，不受影响（这正是 n-gram 映射本来依赖的约定）；Windows 上目标被映射或打开时改名失败，这时只要目标已是普通文件，就丢掉自己的副本、用已就位的那张。刚建的代次（`replace_existing` 为真）是这次独有的暂存目录，没有竞争，改名失败照常报错。

每次调用先删掉代次目录里一小时没被写过的 `<表名>.incoming`（旧版本的固定名字）和 `<表名>.<字母数字>.incoming`，只认普通文件。文件按路径删除，不需要认领：主人若其实还活着，之后的改名找不到原路径，只会让那次准备失败，已就位的表不受影响。旧版本留下的固定名字新代码根本不用，升级后不会因为它出错，只在变旧后被清掉。

### 为什么是一小时

真在准备的进程一直在写自己的暂存目录：backup API 复制词库、合并五笔表、回放、建反查索引，最慢的设备上两次写入之间也只隔几十秒，一次完整准备在几分钟以内。取目录及其中每一项修改时间的最大值，正在复制的大文件会让整个目录保持“新”。一小时给最长的静默间隔留出几十倍余量。代价是被杀留下的半截目录要等一小时后的下一次准备才删，期间只占空间，不再挡住准备。Android 的 `clearInterruptedStaging` 保留，它在首次准备前、持有 `bootstrap.lock` 时立刻删，不让那一小时的空间白占。

### 主人其实还活着时会怎样

看不出主人死活，时间判据就不能证明“绝不碰活着的准备”。iOS 会把在准备途中的 App 挂起任意长时间，恢复后接着写。认领式改名保证即使这样也不会损坏任何东西：

- 清理方先改名再删。主人之后用原路径写 `.ready` 或改名都会碰到 `ENOENT`，这次准备失败，再按上面的规则改用已就绪的代次（如果有）；改名成功的那一刻目录还没开始删，主人也不可能把删到一半的目录发布成 `<id>`。
- 如果直接对原路径 `remove_dir_all`，Unix 的实现持着目录句柄逐项 `unlinkat`。主人恰好在删除途中把目录改名成 `<id>`，删除会沿着句柄继续删已经发布的代次，留下一个永远打不开的 `<id>`（下一次准备的改名因为非空失败、就绪检查又因为缺文件失败）。认领这一步就是为堵住这个窗口。
- Windows 上目录里还有打开的文件时改名本身就失败，正在写的暂存目录根本认领不了。

## 各平台的并发

| 场景 | 结果 |
| --- | --- |
| iOS 的 App 与键盘扩展、Android 的主进程与 `:ime`、HarmonyOS 的 App 与输入法扩展同时第一次准备 | 各写各的暂存目录，先改名的发布，另一方改名失败、删掉自己的暂存目录、打开已发布的代次 |
| Windows 的 Server 与设置程序，Linux 的 IBus 与 Fcitx5 | 同上；Windows 的改名语义见上文 |
| 一方在就绪路径上回放，另一方刚发布 | 只有已就绪的代次被两方读写，SQLite 的锁和 5 秒忙等待负责 |
| 新旧两个版本的进程交替运行（更新期间） | 旧版本仍用固定的 `<id>.incoming` 和 `<表名>.incoming`，新版本按一小时规则清理它们，不会删掉旧版本正在写的 |
| 几个进程同时给已就绪代次补同一张 n-gram 表 | 各写各的临时副本；Unix 上以相同字节先后替换，Windows 上改名失败的一方用已就位的表 |

Android 应用不能建硬链接（SELinux）；这里只用同一目录内的 `mkdir`、`rename` 和删除，不用 `persist_noclobber` 之类可能退回硬链接的接口。

## 不再产生的错误

`RUNTIME_STAGING_EXISTS` 没有任何路径会再产生，常量随之删除。唯一按这句文本匹配的是 Android 的 `PreparationFailureSmoke`，它只是借这句话测试去掉 `Bootstrap` 加的前缀，换成仍然存在的 `Incomplete runtime dictionary generation`。`Bootstrap.clearInterruptedStaging` 的 `endsWith(".incoming")` 同样匹配新名字，只改了注释。

## 同类问题的排查

在 `crates/engine`、`crates/host-api`、`crates/client-services`、`crates/client-core` 和 `crates/client-base` 里查了固定名字的暂存路径（`.incoming`、`.tmp`、`.staging`、`.partial`、`.new`、`with_extension`）以及独占新建、改名发布的调用点。词库与运行时准备路径上需要改的只有上面两处，其余的情况：

- `user_dictionary/state.rs` 的 `stage_dictionary_state_for` 独占新建调用方给的目录；唯一调用方 host-api 的 `dictionary_snapshot` 每次在 `snapshot-<随机串>` 临时目录里建它，不会被旧的挡住。但进程被杀时 `snapshot-*` 留在云词库的暂存目录里，没有人清理，只占空间、不挡启动。
- `user_dictionary/reset.rs` 的重置用带纳秒时间戳的 `.reset.`、`.backup.` 名字，不会被旧文件挡住；被杀时留下的临时文件同样没人清理。另外，若进程恰好在把原词库挪成 `.backup.` 之后、新词库改名就位之前被杀，代次里就缺这个词库，下一次准备报 `Incomplete runtime dictionary generation`。这不是固定名字的问题，这次没有改，后来由[重置途中被杀时放回原文件](2026-10-27-interrupted-reset-restores-originals.md)修复。
- `host/options.rs` 的翻译 sidecar 用 `tempfile` 的随机名 `.custom-translations-*` 写好再 `persist`，不会被挡；被杀时留下的小文件没人清理。
- client-core 的 `resources.rs` 安装资源时持有 `resources.lock`、每次一个随机暂存目录，并且已经有 `sweep_abandoned_stages`，没有这类问题。client-core 偏好与 host-api 云词库快照文件的临时名带随机串，并已有清理。host-api 的 `learned_translation` 独占新建遇到已存在当作成功。

## Alternatives considered

- **跨进程文件锁包住整个准备。** 最强的理由：后来的一方阻塞等先来的一方做完，不重复复制 200 MB，主人死了内核自动放锁，清理也能用“锁拿得到就是没人在写”精确判断。不用的原因：仓库约定文件锁只经 `msime_client_core::file_lock`（`scripts/test-file-locking-goes-through-client-core.py` 把关），因为 Android 上 `std::fs::File::lock` 直接失败；引擎不能依赖 client-base/client-core，它们的 `build.rs` 要联网拉 OpenCC 表，而引擎被 Nix、Debian、RPM 离线打包和网页引擎一起编译。在引擎里用 `libc::flock` 再写一份等于复制 client-base 那段按平台分支的逻辑，也会被那项检查拦下。另外，键盘扩展启动时阻塞等另一个进程复制完，在 iOS 上同样可能撞上看门狗。
- **用进程号判断暂存目录的主人是否还活着，死了立刻删。** 能比一小时更早回收空间。但 Linux 上不同 PID 命名空间（Flatpak、容器）里的进程号在本命名空间查不到，会把活着的准备判成死的；iOS 沙盒里 App 与扩展互相 `kill(pid, 0)` 的结果也不可靠；进程号还会复用。判错的方向恰好是删掉活着的准备，所以不用。
- **沿用固定名字 `<id>.incoming`，遇到已存在时按年龄删掉再重建。** 改动最小。但并发准备的一方仍会因为目录已存在而失败（今天两进程同时启动就是这样），而且删掉再建同一个名字时，旧主人如果还活着会和新主人写进同一个目录，正是 RP:155-157 要防的事。
- **清理时直接 `remove_dir_all`，不先改名认领。** 少一次改名。但留下了上面“删除途中被发布”的窗口，一旦命中就是永久打不开的代次，比这次修的问题更糟。
- **复用 client-core 的 `storage::StagingDirectory`**（见[共用暂存目录生命周期](../simplification/2026-10-12-shared-staging-directory.md)）。它在 Unix 上用父目录句柄防符号链接替换，但同样受“引擎不能依赖 client-core”所限；这里的父目录在进入前已由 `reject_redirected_directory` 检查过，与改动前的保护相同。

## Consequences

- **收益**：进程在准备途中被杀不再让输入法永久起不来，任何平台都不需要用户手动删目录；两个进程同时第一次准备也不再有一方报错。
- **代价**：被杀留下的半截目录最多多占一小时左右的空间（每个最多约 200 MB），反复在一小时内被杀会叠加几个；并发准备时两方都会完整复制一遍，输的一方白做一次。
- **已知上限**：`<id>` 存在但没有 `.ready`（不是本流程能产生的状态）时的处理不变，仍然报错。上一节列出的几处遗留文件只占空间，这次没有改。

## Verification

- 先写回归测试，在未改的 develop 代码上运行：`a_stage_left_by_a_killed_preparation_does_not_block_the_next_one`（留下旧固定名和新名字两个带半截词库的暂存目录，下一次准备必须成功，且这两个刚写过的目录原样保留）和 `concurrent_preparations_of_one_generation_all_succeed`（8 个线程用 `Barrier` 同时准备同一个 id，全部成功、拿到同一个代次、日志已回放、n-gram 表完整、最后只剩 `v1`），两者都以 `Runtime dictionary staging directory already exists` 失败。修复后通过。
- 把“失败后改用已就绪代次”那一支临时短路掉，并发测试连续 10 次都失败，说明它确实走到了“改名输给另一方”的路径；恢复后连续 30 次通过。
- `only_stages_untouched_for_an_hour_are_swept` 用注入的“现在”覆盖清理规则：刚写过的暂存目录保留，`.removing` 立即删，一小时后两种暂存名都删；代次目录、名字对不上的目录、普通文件和指向外部的符号链接都不动。`preparation_sweeps_stages_abandoned_an_hour_ago`（Unix）把修改时间改到一小时前：只有目录旧、里面文件新时不删，都旧了才在下一次（走就绪路径的）准备时删掉。
- n-gram 表：`concurrent_preparations_add_a_newly_shipped_table_once`（代次就绪后资源新增一张 8 MB 的表，8 个线程同时准备同一个代次）在改动前的代码上连续 5 次都以 `No such file or directory (os error 2)` 失败；修复后全部成功、就位的表与资源逐字节一致、代次里没有残留的 `.incoming`，连续 20 次通过。`table_copies_left_by_a_killed_preparation_do_not_block_and_are_swept_later` 留下旧固定名和新名字两个半截副本：补表照常成功、刚写过的副本保留，注入一小时后的时间再清理时只删这两个，名字对不上的文件、同名目录和代次本身的文件不动；Unix 测试里改了修改时间的副本在下一次准备时被删。
- 在 Mac Studio 上：`cargo test -p msime-engine --locked --lib` 2678 项、`--tests` 的 golden 31 项全部通过；`cargo clippy -p msime-engine --locked --all-targets -- -D warnings` 无告警；`cargo fmt --check` 通过。
- Android：把 `msime-engine` 的 lib 单测交叉编译成 `aarch64-linux-android`，在 Mac Studio 上的专用 AVD（API 28 的 `emulator-5566`、API 35 的 `emulator-5580`）里用 `adb shell` 跑 `user_dictionary::generation` 的 26 项（包括 n-gram 的并发与清理测试），两台都全部通过，包括并发、遗留暂存目录和改修改时间的清理测试。做法与 `run-core-test.sh` 相同，只是加了过滤、用了单独的设备目录。`build-core-test.sh` 只编 client-base、client-core、client-providers 和 client-services，碰不到引擎，没有跑；装 APK 走真实首启准备的 `smoke.sh --core` 也没有跑，这次的改动没有在应用沙盒里的首启流程中验证过。
- `bash scripts/verify-local.sh --quick` 通过，其中 `android host java` 一项编译并运行了改过的 `PreparationFailureSmoke`。
