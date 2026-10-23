# Agent Note: 聚焦和建会话时，词库更新的选择记录与代次校验每次解析只做一次

Status: implemented

## Problem

iPhone XR 上 `msime_client_focus` 约占 59 ms CPU，`msime_client_create` 约 89 ms，其中很大一部分是文件系统检查（`std::fs::canonicalize` 和 `msime_path_trust::reject_symlinked_components` 带来的 lstat、getattrlist）。

聚焦时 `HostSession::refresh_resource_packs` 要找粤拼、注音、笔画、日文，资源目录不带五笔码表时还有五笔，最多五个文件；建会话时 `HostOptions::into_engine_options` 找同样几个。原来每个文件各调一次 `updated_dictionary_file`，每次都从头做同一套事：读状态目录里的版本记录、解析资源锁；调 `dictionary_updates::selected`，它逐层检查 `installed.json` 路径上的符号链接、读并校验选择记录、规范化状态目录、再逐层检查桌面词库代次目录；装了更新时再规范化一次状态目录、校验代次目录；最后 `installed_file` 又把 `selected` 和代次目录查找整套做一遍。退回随包目录的那一步还为每个文件规范化一次状态目录。这五个文件出自同一份选择记录，重复的部分对结果没有任何影响。

## Decision

`crates/host-api/src/lib.rs` 的 `DownloadedDictionaries` 代表一次解析（一次聚焦刷新或一次建会话）里对状态目录的查找。版本记录和资源锁、选择记录（含它的全部路径检查）、桌面词库和语言词库两个代次各自的目录与校验（`dictionary_updates::verify`），以及状态目录的规范化，各只在第一次用到时做一次，这次要找的每个文件共用。文件本身的检查仍逐个做：`msime_client_providers::dictionary_updates::generation_file` 对每个文件逐层检查符号链接、确认是普通文件、长度与清单一致。`installed_file` 改为调用同一个 `generation_file`，行为不变。

`LanguageDictionaries::resolve`、`japanese_dictionary`、`wubi_dictionary` 改收 `Option<&DownloadedDictionaries>`。`refresh_resource_packs` 和 `into_engine_options` 各建一个，把它交给这三处。

**它不跨调用保留。** 每次聚焦、每次建会话都新建一个，从头读选择记录、重做全部规范化和符号链接检查。设置应用随时可能换掉选择记录、回退或删除代次，下一次聚焦就能看到；也就没有要失效的缓存。一次解析内部，选择记录在读过第一个文件之后被换掉，这次解析仍按已读的那份选文件。原来逐个文件重读时，同一次解析里的五个文件反而可能出自两份不同的选择记录，所以这不比原来弱。

每一项都是用到时才做：没装更新时不碰代次目录；资源目录带着五笔码表时 `wubi_dictionary` 照旧不查状态目录；选择记录里没有语言词库代次时不校验它。每种情况下选中的文件与原来相同：没装更新、装了且选中、`installed.json` 坏了或不在、代次校验不过、路径上有符号链接、单个文件退回资源包或随包目录。

## Measurements

Mac Studio（M 系列）上 release 构建，在测试进程里用 `proc_pid_rusage(RUSAGE_INFO_V4)` 的 `ri_instructions` 和 `task_info(TASK_EVENTS_INFO)` 的 `syscalls_unix`，统计 500 次循环（建会话 50 次）的增量，取平均。这台编译机同时跑着 CI，墙钟时间不可靠，指令数和系统调用数不受负载影响。测量用的探针是一个临时的 ignored 测试，没有提交。

夹具有两种：「没装更新」是状态目录里没有 `dictionary-updates`，HostOptions 记录了带齐三个语言词库的随包目录；「装了更新」是选中一份词库更新，语言词库代次带齐合成文件并能通过校验，桌面词库代次只有空目录、校验不过（没有真实的句子模型文件，造不出能校验通过的桌面代次），所以日文和五笔照常退回资源包。两种情况下改动前后解析出的路径完全相同。

| 场景 | 改动前 指令 / 系统调用 | 改动后 指令 / 系统调用 | 指令减少 |
| --- | --- | --- | --- |
| 装了更新，`msime_client_focus(true)` | 22.98 M / 1105 | 6.67 M / 336 | 71% |
| 装了更新，`refresh_resource_packs` | 22.70 M / 1104 | 6.48 M / 335 | 71% |
| 装了更新，建会话再销毁 | 29.44 M / 1440 | 13.26 M / 671 | 55% |
| 没装更新，`msime_client_focus(true)` | 6.08 M / 327 | 3.21 M / 175 | 47% |
| 没装更新，`refresh_resource_packs` | 5.93 M / 326 | 3.02 M / 174 | 49% |
| 没装更新，建会话再销毁 | 12.17 M / 633 | 9.31 M / 481 | 23% |

聚焦的开销几乎全在 `refresh_resource_packs` 里（装了更新时 22.98 M 中的 22.70 M）。真机上桌面词库代次能校验通过，原来日文和五笔各要再走一遍完整的选择记录读取和桌面代次校验，现在共用一次，所以真机上省下的绝对量比表里装了更新的那几行还多。这些数字没有在 XR 上复测，不能换算成 XR 上的毫秒数。

## Alternatives considered

- **跨调用缓存选择记录和校验结果**（按 `installed.json` 的修改时间或文件身份判断是否失效）。最强的理由是聚焦时几乎不用再碰磁盘。不用它，是因为这些检查防的正是两次调用之间的替换：设置应用在别的进程里随时发布新代次、回退或删目录，符号链接也可能在两次聚焦之间被放进来。缓存要么拿 stat 判断失效，等于又把大部分检查做一遍；要么有一段时间用的是过期的、没再检查过的路径。按次解析没有失效问题，已经去掉了七成的指令。
- **让 `selected` 顺带交出规范化后的状态目录，两个代次目录都从它拼出来**，每次解析还能再少两次规范化和两遍状态目录的符号链接检查。没有做：`selected` 是 providers 的公开函数，还有 `selected_resources`、`languages` 等调用方，改它的返回值要连带改这些调用；这一步的收益没有单独测过，按上表剩下的系统调用数估计不大。

## Consequences

收益：聚焦时的指令数在装了更新时减少约 71%，没装时约 47%；建会话分别减少约 55% 和 23%。查找顺序和每一项安全检查都没变，所以不需要迁移，也没有新的失效路径。

代价：多了一个只在一次解析里存在的类型，三个查找函数的参数从状态目录换成它。以后新增一个从词库更新里找的文件，应当在同一个 `DownloadedDictionaries` 上调用 `updated_file`，不要为它另起一次查找。

剩下的开销（没装更新时聚焦仍约 3.0 M 条指令、175 次系统调用）落在资源包查找（`resource_packs::installed_file`，每个文件、落定重排模型、语法模型各一次）和其他检查上，这次没有分析，也没有动。

## Testing

`a_selected_dictionary_update_wins_until_it_stops_verifying` 覆盖：没装更新时用随包目录；选中的更新里的语言词库优先，建会话（`into_engine_options`）选的是同一组；`installed.json` 坏了、代次里一个文件被改（长度不变）、代次目录的上级被换成符号链接时，三个词库都退回随包目录；校验不过的更新目录不会被当作随包目录采用；桌面代次校验不过时日文和五笔不从它取。

`one_resolution_reads_the_dictionary_update_selection_once` 证明一次解析里选择记录只读一次、代次只校验一次：读过一个文件后把选择记录写坏、把另一个文件改成同长度的内容，同一次解析仍返回它；文件长度变了，逐个文件的检查仍会拒绝它；新的一次解析看见坏掉的选择记录，返回空。

`cargo test -p msime-host-api -p msime-client-providers --locked --no-fail-fast`：providers 251 项通过；host-api 单元测试 924 项通过、5 项失败，失败的 5 项（`listing_reports_every_pack_and_checks_the_state_root`、`ai_provider_update_export_returns_only_the_four_configuration_fields`、`currency_punctuation_follows_character_width_on_all_key_routes`、`generated_mode_selections_stay_out_of_selection_statistics`、`the_c_header_and_the_rust_exports_agree`）在未改动的 develop（fb9a250acb）上同样失败，与本改动无关。`cargo clippy -p msime-host-api -p msime-client-providers --all-targets -- -D warnings` 通过。没有在 Android 模拟器和 iPhone 真机上运行。

## Related notes

词库更新的分代安装、选择记录与校验规则见 [全平台词库独立更新](../feature/2026-10-18-dictionary-auto-update.md)，这里没有改变其中任何规则。同一台 XR 上按键路径的耗时分析见 [词库连接打开内存映射](2026-10-22-dictionary-mmap-for-slow-devices.md)。
