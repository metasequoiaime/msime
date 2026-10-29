# Agent Note: 选词位置与上屏效率写进宿主的统计目录，打开上传时不误报已上传

Status: implemented

## Problem

共享层在会话里攒下的选词位置和上屏效率，`HostSession` 一直写进 `options.user_data` 下的 `typing-statistics.json`。准备好的目录布局里 `user_data` 是 `state_root/user`，是 Engine 的个人词库目录；各宿主记字数和按键、统计页显示、`typing_upload_*` 上传读的都是宿主传入的统计目录，也就是 `preferences_directory`（`state_root`）下的那一份。`user_data` 下的统计文件不存在，读出来按关闭处理，于是所有宿主的选词位置和 Android 的上屏效率（只有 Android 计，见 `COUNTS_COMMIT_EFFICIENCY`）一条也没记进用户看得到的统计，上传的候选位次也一直是空的。原有测试把统计 store 建在 `root.join("user")`，和实现错在同一处，所以一直是绿的。

另一处：打开「上传到云端」后，共享设置页和 Android、iOS、Windows 的原生设置都固定说「最近 7 天的每日计数已传到你的水杉账号」。宿主在打开时立刻上传一次，但本机统计里还没有字数的日子时共享层什么都不发，`lastUploadMs` 仍为空，服务端也还不认识这台设备，这句话就不成立。

## Decision

`HostSession::statistics_directory` 返回 `state_root`，选词位置的开关读取、攒批写出和上屏效率（包括后台写效率的队列）都写这一份；没有 `preferences_directory` 的会话不知道统计在哪里，选词和效率都不记，不退回 `user_data`。后台效率队列的每一批带上自己的统计目录，不再从 Engine 选项推出。

目录改对以后，交出选词的那一路就不再是空转：`flush_selections` 在宿主的输入线程上跑（满 `SELECTION_BATCH` 次选词、失焦和销毁会话时），原来直接调 `record_selections`，在输入线程上取文件锁、先把本进程攒着的上屏整批写出、再整份读写统计文件并 fsync；`statistics_enabled` 也在 Android 的输入线程上为一个开关调 `load()`，做同样的事。周围的代码刻意不在这些线程上写统计（只有 Windows 的失焦和桌面宿主的销毁在当前线程上写），所以改成：会话交出选词时只调 `TypingStatisticsStore::queue_selections`，把计数加进本进程为这个统计目录攒着的批次，不取文件锁、不碰磁盘；写文件跟着宿主经 `record` 攒的上屏走，由已有的写出点一起写：失焦和销毁随后的 `flush_typing_statistics`（Windows 失焦、桌面销毁和 `msime_client_flush_all` 在当前线程上，其余在后台线程上）、满间隔的 `record`，以及这个目录上读写统计的任何操作（统计页、上传、后台写效率的队列），所以读统计的地方总看得到攒着的选词。攒着的选词放在单独一把进程级的锁 `PENDING_SELECTIONS` 里，只在加减计数和记文件状态的一瞬间拿着：上屏批次的 `PENDING_COMMITS` 在写文件时一直拿着，输入线程取它就可能等别的线程的 fsync。这把锁里还记着本进程上次读到或写下统计文件时看到的开关和清空代号：所知的开关关着时交来的选词当场丢掉；写出时文件里的统计关着，或代号变了（清空过），攒着的选词整批丢掉，和攒着的上屏一样；本进程还没读过文件时照攒，由第一次读到的开关和代号决定去留。统计文件还不存在也算读到了一个状态：默认值，统计关着，攒着的选词随之丢掉，写出那一步也只在读到的统计开着时合并选词，所以新装、从没打开过统计的配置不会因为选词建出一份关着却带着选词的统计文件。上屏效率是否要计也只看这个所知的开关（`known_enabled`），本进程还没读过时当作打开，写入时文件自己的开关作数。

会话选项新增可选的 `selection_statistics`，写 `false` 的会话不记选词位置和上屏效率，缺省照常记。Windows 有两个会话处理同一次选词：Server 的会话选词、画候选窗，TIP 在应用进程里的宿主会话随后按文字选中同一个词（见 [空格和点选也在 TIP 的宿主会话里选词](2026-10-16-windows-space-selects-in-host-session.md) 和 [按文字匹配宿主会话的候选](2026-10-22-windows-host-candidate-identity-by-text.md)），两边的 `preferences_directory` 是同一个状态根。目录改对之后两边都会记，每次选词算两遍；TIP 在内存里构造会话选项时写入 `selection_statistics: false`，只由 Server 的会话记。其他平台每次输入只有一个会话，不写这个字段。

开关变更成功后的说法看宿主返回的状态：打开时 `lastUploadMs` 为空就说本机还没有可上传的计数、打字以后再打开会自动上传，有值才说已经传上去；关闭的说法不变。`packages/ui` 的共享设置页（鸿蒙的统计页也用它）用 `uploadChangedMessage`，Android 用 `TypingUploadApi.enabledMessage`，iOS 用 `TypingUpload.changedMessage`，Windows 原生设置和 Win7 设置页用 `upload_done_message(enabled, status)`，各处同一个判断。

与 [打字统计可选上传到云端](../feature/2026-10-17-typing-statistics-cloud-upload.md) 部分重叠：本篇只修正打开时的说法，上传路径和节流不变。与 [共享 Rust 空闲 IO 与热路径](2026-10-27-shared-rust-idle-io-hot-path.md) 相关：攒批与写出时机沿用那里的约定，本篇只改写到哪里。

## Alternatives considered

- **选词批次也走后台写效率的那个队列**：改动小，`record_selections` 照旧，只是换到后台线程上调。没有采用：每满一批选词就多一次整份读写和 fsync，与上屏批次各写各的；Windows 失焦和桌面销毁这些本来就在当前线程上收尾的地方还得另走一条同步的路，否则进程随后退出时后台线程来不及写；会话上的开关缓存也要等后台线程把答案送回来。攒进上屏的批次以后，写出点、开关和清空的规矩都与上屏共用一套。

- **让宿主把统计目录也指向 `user_data`**：只改一处读取就能让两边对上。没有采用：字数、按键、上传状态和词汇进度都已经在 `preferences_directory` 下，各平台的统计页和上传都读那里，挪目录要迁移所有现有用户的统计文件，而选词位置一直没写进去，没有需要迁移的数据。
- **没有 `preferences_directory` 时退回 `user_data`**：旧宿主不传这个字段时还能记。没有采用：退回去写的正是没人读的那份文件，等于把这次的问题保留成一条隐蔽的分支。
- **TIP 的会话用 `msime_client_set_private_session` 或 `SessionUse::Preview` 关掉统计**：都是现成的开关。没有采用：隐私会话同时关掉预测，而且只在 Android、鸿蒙和 iOS 上生效；Preview 还跳过向设置页报告词库代次等输入会话该做的事。镜像会话只是不该记统计，用一个只管这一件事的选项。
- **改由 TIP 记、Server 不记**：TIP 的会话在每个应用的进程里，低完整性或沙箱进程写不了状态根，几个应用进程还会同时写同一个文件。Server 是固定的一个进程，候选窗也由它画。
- **打开时一律说「已打开上传」，不提是否传上去**：一句话覆盖所有情况。没有采用：用户打开上传就是想知道数据到了没有，有值时明确说已传上去、没值时说什么时候会传，比含糊的说法更有用。

## Consequences

选词位置和上屏效率从此出现在统计页和上传里，而交出选词不在任何宿主的输入线程或 UI 线程上读写统计文件，统计页和上传读的时候先把攒着的写进去；升级后之前一直缺的这两项从零开始累计，不补算历史。`user_data` 下如果有旧版本残留的统计文件，继续没人读，也不删除。

本进程所知的开关可能落后于别的进程刚做的改动：设置页刚打开统计时，本进程在下一次读统计文件之前（宿主下一次 `record`、刷新原生开关缓存时的 `load`，最多一个上屏攒批间隔）交来的选词仍按关着丢掉；刚关掉时攒下的那些在写出时按文件的开关丢掉。这和攒着的上屏是同一个误差。统计文件不存在时同样记成「所知关着」，Android 在本进程读过一次之后不再攒效率，直到宿主的 `record` 读到打开的文件。读不出统计文件时攒着的选词留在内存里，等下一次能读时再写，不像上屏那样整批丢掉；计数是定长的，留着不涨内存。`record_selections` 保留，给能在当前线程上等磁盘的调用方，host-api 不再用它。

Windows 上只在 TIP 会话里完成、Server 的会话不经手的选词，不记选词位置。这次没有逐个方案核对哪些选词属于这种情况；如果统计页里 Windows 某个方案的候选位次明显偏少，就是这个取舍的代价，要回来看是否让那条路径由 TIP 记。

候选预览（`preview_candidates`）的「上屏不计数」测试原来检查的是 `user/` 下的统计，修正后改为在真正的统计目录打开开关再断言没有计数，否则这个断言会永远成立。

## Verification

- `cargo test -p msime-host-api`：新增 `selection_statistics_land_in_the_hosts_statistics_directory`（写进 `preferences_directory`，`user/typing-statistics.json` 不存在）、`a_session_without_a_statistics_directory_records_no_selections`（不传目录时 `user_data` 下一条不记）和 `a_mirror_session_leaves_selection_statistics_to_the_session_it_follows`（`selection_statistics: false` 的会话一条不记，同一份统计的正常会话照常记），原有选词、隐私会话、双拼效率、后台效率队列和算式候选的统计测试改为在宿主的统计目录建 store。
- 后续修正，输入线程不写统计文件：`cargo test -p msime-client-services` 新增 `queueing_selections_never_touches_the_statistics_file`（目录建不出来时照样攒下，攒下以后文件不变，flush 后写进且只写一次）、`readers_and_the_due_record_write_queued_selections`、`queued_selections_follow_the_switch_and_a_reset_elsewhere`、`a_reset_in_this_process_clears_queued_selections` 和 `the_background_flush_writes_queued_selections`；模拟别的进程改文件的测试辅助不再经 `read_locked`/`write_locked`，那两处会把状态记成本进程所知的。`cargo test -p msime-host-api` 新增 `selections_handed_over_on_the_input_thread_wait_for_the_statistics_flush`：统计文件读不出时选满一批，文件不变，恢复后 `flush_pending_commits` 写进全部 32 次；把 `flush_selections` 临时改回 `record_selections` 时它以 0 对 32 失败。`queued_selections_never_create_a_statistics_file_while_statistics_are_off_by_default`：统计文件不存在时攒下的选词，经 `flush_pending_commits`、后台写效率时的取锁和随后打开统计，都不建出文件、不补记；改动前它在第一处 `assert!(!path.exists())` 失败。原有断言失焦后统计内容的测试不改，它们用的 `load()` 先写出攒着的批次，与统计页一样。
- `apps/desktop` 的 vitest：`use-typing-statistics-upload.test.tsx` 新增打开后 `lastUploadMs` 为空时的说法，`typing-statistics-upload.test.tsx` 原有断言不变。
- Windows `typing_statistics_model.cpp` 在 macOS 上用 clang `-std=c++17 -Wall -Wextra -Werror` 编译运行通过；`main.cpp`、Win7 的 `StatisticsForm.h` 和 TIP 的 `msime-tsf-host-configuration-test`（会话选项带 `selection_statistics: false`）靠 PR 的 Windows CI。Android `TypingUploadApiSmoke` 与 iOS `RustCloudPagesTests` 由各自 CI 运行。
- 没有在真机或真实账号上验证上传后的候选位次。
