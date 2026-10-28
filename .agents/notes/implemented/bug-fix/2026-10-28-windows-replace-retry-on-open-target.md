# Agent Note: Windows 替换被短暂占用的存储文件时重试，设置页不再把写盘失败当成并发更新

Status: implemented

## Problem

共享存储发布 JSON 文档（偏好、剪贴板历史与分类、版本记录、资源清单、社区资源库、词库合集、账号会话、遥测队列等）的方式是：在同目录写临时文件、同步，再用 `tempfile::NamedTempFile::persist` 替换目标。Windows 上这一步是 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`，只要有别的进程打开着目标文件，就返回访问拒绝 5 或共享冲突 32；目标句柄的共享标志里有没有 `FILE_SHARE_DELETE` 都一样（PR #10119 的 CI 对照已经证实，见 [设置页分页的验证说明](../feature/2026-10-20-windows-settings-page-tabs.md)）。杀毒软件、备份与同步工具、搜索索引经常为了读一下就打开文件几毫秒，它们不取 `preferences.lock` 这类存储锁。存储没有任何重试，这次保存直接失败。

失败之后，WinUI 设置页 `platforms/windows/settings/main.cpp` 的 `commit()` 不分原因，一律提示「设置可能已在其他窗口中更新，或与其他设置冲突」，从磁盘重读旧文档、整页重画：用户刚做的修改被旧值换掉，提示还指向一个并不存在的并发修改。PR #10119 排查 CI 偶发失败时看到的正是这个画面（滑块回到旧值、页首多一条「无法保存这项设置」），当时的结论「宿主自己的读者都在锁里读，所以不是产品问题」只对宿主自己的进程成立。

## Decision

**存储层**：`crates/client-base/src/storage.rs` 新增 `persist_replacing(temporary, path)`，签名和返回值与 `NamedTempFile::persist` 相同，宿主经 `msime_client_core::file_lock::persist_replacing` 调用。

- 只在 Windows 上重试，只认 `ERROR_ACCESS_DENIED`（5）、`ERROR_SHARING_VIOLATION`（32）、`ERROR_LOCK_VIOLATION`（33）。其他错误第一次就返回。
- 第一次失败后再试最多七次，间隔 10、20、40、80、160、320、320 毫秒，合计 950 毫秒。
- 重试期间临时文件留在原处，目标要么是完整的旧内容、要么是完整的新内容；最后仍失败时临时文件随 `PersistError` 交回，调用方丢弃它即删除。
- 调用方在存储锁里调用时等待也在锁里，同一把锁的读者最多多等这一秒，不会有别的写入方插进来。
- Unix 上直接调用 `persist`，`rename(2)` 不受打开的句柄影响，行为不变。

`replace_private_file_with_permissions` 的非 Unix 分支（`replace_private_file`、`write_private_file_at` 等一族都经过它）以及所有在 Windows 上替换已有文件的 `persist` 调用点都改用它：`client-core` 的偏好 `atomic_write`、剪贴板历史与分类、版本记录、资源清单；`client-services` 的社区资源库、词库合集、账号会话与匿名账号、遥测、诊断导出、账号数据与词库导出、词库快照下载；`host-api` 的 MCP 客户端配置和云词库快照；Tauri 外壳的 `shared/atomic_file.rs`。重试循环 `retry_transient_replace` 把替换函数和等待函数当参数，测试据此在各平台上注入失败、不必真的睡。

**WinUI 设置页**：`PreferencesSaveFailure.h` 按 host-api 返回的错误文字把保存失败分成三类，处理各不相同：

- 冲突（`preferences changed; reload before saving`）：照旧重读已存的设置，说明设置已在其他窗口中更新。
- 不被接受（取值越界、按键冲突、提交的文档无效、返回内容读不了、认不出的错误）：照旧重读，说明它与其他设置冲突或取值无效。
- 写盘失败（`preferences storage failed:` 开头，即 `PreferencesError::Io`）：不重读。修改留在 `PreferencesDocument` 里，页面照它显示，预览也跟着它走；页首以警告说明「暂时没能保存，修改已保留，稍后会自动重试」，按 1、2、4、8 秒的间隔再存四次。之后不再自动重试，改说「仍未保存」，在下一次修改、窗口失去焦点或关闭时再存。期间任何一次保存成功就换成「刚才没能保存的修改现已保存」。窗口重新获得焦点时若磁盘修订号已被别处推进，保留的修改作废，按冲突说明。

这两句错误文字由 `crates/client-core/src/preferences/tests.rs` 的 `error_wordings_the_harmony_host_matches_on` 钉住。

**Win7 设置窗口**不改行为：`platforms/windows/settings/win7/PreferencesStore.h` 早已把冲突与存储失败分开，存储失败的说明是「读写设置失败，请重新读取后重试」，没有误指其他窗口。它在失败后重读快照是有意的（`SettingsWindow.cpp` 的注释写明不猜测是否落盘、不自动重试覆盖），存储层的重试已经消除了这里的瞬时失败。

## Alternatives considered

- **改用带 POSIX 语义的按句柄改名（`SetFileInformationByHandle` 的 `FileRenameInfoEx`，`FILE_RENAME_FLAG_POSIX_SEMANTICS | FILE_RENAME_FLAG_REPLACE_IF_EXISTS`）。** 最强的理由是它从根上去掉了这类失败：目标被打开时照样替换，旧句柄继续读旧内容，与 Unix 一致，不需要等。不用它，是因为它只对共享标志里带 `FILE_SHARE_DELETE` 的句柄有效，而这里要对付的杀毒、备份工具恰恰常以不带删除共享的方式打开文件，照样失败；它还要求 Windows 10 1607 以上和 NTFS，Win7 版用不了；而且要在 `client-base` 里加 Win32 FFI 和 `windows-sys` 依赖。重试对两种打开方式都有效，代码也只在一处。
- **在设置页 C++ 里重试，存储层不动。** 改动集中在出问题的那个窗口。但同一个替换失败会落在每一个写这些文件的宿主上（TSF Server、Win7 设置、Tauri 面板、MCP 服务），只修一个界面，其余的照样丢写入；而且界面层看不到是哪一步失败、临时文件还在不在，只能整份重存。
- **设置页在写盘失败时也重读，只把提示改准确。** 最省事，也不会让页面显示没存下的值。但用户的修改照样消失，只是换了一句说法；本来再等一秒就能存下的修改，要用户自己重做一遍。

## Consequences

- **收益**：目标被短暂占用（几毫秒到约 1 秒）时，所有共享 JSON 存储的写入都会成功，宿主和设置页看不到这类失败。更长的占用下，WinUI 设置页保留用户的修改并自动补存，提示说的是真实原因；真正的并发修改仍走原来的冲突路径。
- **代价与已知上限**：访问拒绝 5 也可能是真的没有权限（只读文件、ACL），这时每次保存要多等约 1 秒才报错；在存储锁里等待时，同一把锁的读者（包括输入时读偏好的 Server）在这一秒里也被挡住。WinUI 的保存在界面线程上同步执行，持续失败时每次保存都会让界面卡住约 1 秒。偏好写入成功但随后清空剪贴板历史失败时，同样报写盘失败，下一次重试会因修订号已前进而按冲突重读：读回的就是用户刚存下的值，提示却说其他窗口更新过。`crates/engine/src/host/options.rs` 的翻译词条副本、引擎词库代次里的文件改名和 `rename_private_entry` 的目录改名不在这次范围内：引擎不依赖 `client-base`，代次文件被映射时的改名失败是持久的、已有专门处理，重试只会白等。

## Verification

- `storage.rs` 的单测在各平台上运行：瞬时错误（5、32、33）重试到成功且等待依次为 10、20、40 毫秒；非瞬时错误（2、112、不带系统错误码）只尝试一次、不等待；一直失败时共尝试 8 次、合计等待 950 毫秒，每次尝试时临时文件都在，交回的临时文件丢弃后被删除；正常发布后目录里不留临时文件。
- 只在 Windows 上编译的两条：以 `FILE_SHARE_READ | FILE_SHARE_WRITE`（不带删除共享）打开目标，先确认不重试的 `persist` 返回 5 或 32、原内容不变，再让另一线程 100 毫秒后关闭句柄，`persist_replacing` 成功并读到新内容；句柄一直开着时约 950 毫秒后返回 5 或 32，目标仍是旧内容，目录里没有遗留的临时文件。`preferences/tests.rs` 的 `save_waits_for_a_reader_that_briefly_holds_the_document_open` 对 `PreferencesStore::save` 做同样的验证。这些由 `ci-platforms.yml` 的「Windows native settings」作业在 Windows runner 上运行。
- `platforms/windows/tests/ui/preferences_save_failure.cpp` 核对三类失败的判定、重试间隔和各条说明（写盘失败的说明不提其他窗口）；本机用 `clang++ -std=c++17 -Wall -Wextra -Werror` 编译运行通过，CI 的 MinGW 交叉构建编译它。
- WinUI 页面本身（保留值、提示条、自动补存）没有在真机上制造过写盘失败去验收。
