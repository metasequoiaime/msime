# Agent Note: 选词位置与上屏效率写进宿主的统计目录，打开上传时不误报已上传

Status: implemented

## Problem

共享层在会话里攒下的选词位置和上屏效率，`HostSession` 一直写进 `options.user_data` 下的 `typing-statistics.json`。准备好的目录布局里 `user_data` 是 `state_root/user`，是 Engine 的个人词库目录；各宿主记字数和按键、统计页显示、`typing_upload_*` 上传读的都是宿主传入的统计目录，也就是 `preferences_directory`（`state_root`）下的那一份。`user_data` 下的统计文件不存在，读出来按关闭处理，于是所有宿主的选词位置和上屏效率一条也没记进用户看得到的统计，上传的候选位次也一直是空的。原有测试把统计 store 建在 `root.join("user")`，和实现错在同一处，所以一直是绿的。

另一处：打开「上传到云端」后，共享设置页和 Android、iOS、Windows 的原生设置都固定说「最近 7 天的每日计数已传到你的水杉账号」。宿主在打开时立刻上传一次，但本机统计里还没有字数的日子时共享层什么都不发，`lastUploadMs` 仍为空，服务端也还不认识这台设备，这句话就不成立。

## Decision

`HostSession::statistics_directory` 返回 `state_root`，选词位置的开关读取、攒批写出和上屏效率（包括后台写效率的队列）都写这一份；没有 `preferences_directory` 的会话不知道统计在哪里，选词和效率都不记，不退回 `user_data`。后台效率队列的每一批带上自己的统计目录，不再从 Engine 选项推出。

开关变更成功后的说法看宿主返回的状态：打开时 `lastUploadMs` 为空就说本机还没有可上传的计数、打字以后再打开会自动上传，有值才说已经传上去；关闭的说法不变。`packages/ui` 的共享设置页（鸿蒙的统计页也用它）用 `uploadChangedMessage`，Android 用 `TypingUploadApi.enabledMessage`，iOS 用 `TypingUpload.changedMessage`，Windows 原生设置和 Win7 设置页用 `upload_done_message(enabled, status)`，各处同一个判断。

与 [打字统计可选上传到云端](../feature/2026-10-17-typing-statistics-cloud-upload.md) 部分重叠：本篇只修正打开时的说法，上传路径和节流不变。与 [共享 Rust 空闲 IO 与热路径](2026-10-27-shared-rust-idle-io-hot-path.md) 相关：攒批与写出时机沿用那里的约定，本篇只改写到哪里。

## Alternatives considered

- **让宿主把统计目录也指向 `user_data`**：只改一处读取就能让两边对上。没有采用：字数、按键、上传状态和词汇进度都已经在 `preferences_directory` 下，各平台的统计页和上传都读那里，挪目录要迁移所有现有用户的统计文件，而选词位置一直没写进去，没有需要迁移的数据。
- **没有 `preferences_directory` 时退回 `user_data`**：旧宿主不传这个字段时还能记。没有采用：退回去写的正是没人读的那份文件，等于把这次的问题保留成一条隐蔽的分支。
- **打开时一律说「已打开上传」，不提是否传上去**：一句话覆盖所有情况。没有采用：用户打开上传就是想知道数据到了没有，有值时明确说已传上去、没值时说什么时候会传，比含糊的说法更有用。

## Consequences

选词位置和上屏效率从此出现在统计页和上传里；升级后之前一直缺的这两项从零开始累计，不补算历史。`user_data` 下如果有旧版本残留的统计文件，继续没人读，也不删除。

候选预览（`preview_candidates`）的「上屏不计数」测试原来检查的是 `user/` 下的统计，修正后改为在真正的统计目录打开开关再断言没有计数，否则这个断言会永远成立。

## Verification

- `cargo test -p msime-host-api`：新增 `selection_statistics_land_in_the_hosts_statistics_directory`（写进 `preferences_directory`，`user/typing-statistics.json` 不存在）和 `a_session_without_a_statistics_directory_records_no_selections`（不传目录时 `user_data` 下一条不记），原有选词、隐私会话、双拼效率、后台效率队列和算式候选的统计测试改为在宿主的统计目录建 store。
- `apps/desktop` 的 vitest：`use-typing-statistics-upload.test.tsx` 新增打开后 `lastUploadMs` 为空时的说法，`typing-statistics-upload.test.tsx` 原有断言不变。
- Windows `typing_statistics_model.cpp` 在 macOS 上用 clang `-std=c++17 -Wall -Wextra -Werror` 编译运行通过；`main.cpp` 和 Win7 的 `StatisticsForm.h` 靠 PR 的 Windows CI。Android `TypingUploadApiSmoke` 与 iOS `RustCloudPagesTests` 由各自 CI 运行。
- 没有在真机或真实账号上验证上传后的候选位次。
