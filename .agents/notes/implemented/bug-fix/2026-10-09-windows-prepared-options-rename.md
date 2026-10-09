# Agent Note: Windows 首次准备改为按句柄重命名发布配置

Status: implemented

## Problem

Windows Server 首次运行时由 `platforms/windows/src/input/PrepareHost.h` 准备状态目录：先独占创建 `.runtime-options-prepared`，再给它建一个同目录硬链接 `runtime-options.json` 作为发布，最后用 `remove_private_file` 删掉临时名。

2026-10-08 的临时文件清理加固（`5e57e72e9`、`cac445a62`）让 `remove_private_file` 和 `read_private_file` 都经 `handle_is_trusted_file` 检查，要求 `NumberOfLinks == 1`。刚建完硬链接的临时文件链接数正好是 2，删除被拒绝，返回值又被 `(void)` 丢弃；之后 Server 读取 `runtime-options.json` 同样因为链接数为 2 被拒，抛出 `Configuration unavailable`，以退出码 1 结束。每一次全新安装后 Server 都起不来，TIP 没有 Server 可连，用户打不出中文。

`tests/runtime/prepare_host.cpp` 原本就断言临时名必须被删掉，但 CI 只做 MinGW 交叉编译，不在 Windows 上运行它，这次回归没有被任何门禁看见。

## Decision

Windows 分支用 `publish_private_file` 发布：以 `DELETE` 和 `FILE_FLAG_OPEN_REPARSE_POINT` 打开刚写好的临时文件，`handle_is_trusted_file` 通过后，在同一个句柄上用 `SetFileInformationByHandle(FileRenameInfo)` 改名为 `runtime-options.json`，`ReplaceIfExists = FALSE`。

- 发布仍然是原子的，目标已存在（包括并发创建的文件和符号链接）时改名失败并抛出 `Cannot publish prepared configuration`，不会覆盖别人的状态。
- 发布后的文件只有一个链接，和 `read_private_file`、`remove_private_file` 的单链接策略一致。
- 失败时保留临时文件，与原来「不删除已准备的数据，便于诊断」的约定相同。
- POSIX 分支不变，仍是同目录硬链接加 `std::filesystem::remove`。

同类的单链接策略加原子改名发布，见 [Android 下载临时文件拒绝硬链接](2026-10-10-android-download-part-hardlink.md)。

## Alternatives considered

- **保留硬链接发布，删除临时名时放宽到链接数 2** — 改动最小，硬链接的「不替换已存在目标」语义也已经验证过。但要在 `remove_private_file` 里为这一个调用点开例外，或者新加一个接受多链接的删除函数，等于在刚收紧的可信文件检查上开口子；而且删除失败时留下的依然是链接数为 2 的配置文件，Server 仍然读不了，问题只是被推迟到下一次删除失败。
- **`MoveFileExW` 不带 `MOVEFILE_REPLACE_EXISTING`** — 同样原子、同样不替换，代码更短。但它按路径重新解析源文件，检查与改名之间可以被替换掉，正是 10 月 8 日那批提交要消除的「先校验路径、再按路径操作」模式；按句柄改名把操作绑定到已经校验过的那个文件。

## Consequences

- **收益**：全新安装后 Server 能读到自己准备的配置并正常启动；发布路径与可信文件检查不再互相矛盾。
- **代价与已知上限**：Windows 与 POSIX 的发布方式分叉，`PrepareHost.h` 里多了一个只有 Windows 用的函数。已经被旧版本留下两个链接的状态目录不会被自动修复：删掉残留的 `.runtime-options-prepared` 即可恢复，链接数会回到 1。如果以后 `read_private_file` 的信任条件再变化，要同时检查这里的发布结果是否仍满足它。

## Verification

`platforms/windows/tests/runtime/prepare_host.cpp` 在 Windows 上新增断言：发布后的配置必须能被 `read_private_file` 读出；同时关闭此前未关闭的 `preserved` 文件流，否则 Windows 上临时目录清理失败，测试本身报错。在 Windows 11 上用 MSVC 构建 `windows-prepare-host` 并运行：修改前失败于 `Publication mismatch`，修改后输出 `Windows preparation orchestration passed`。装机验证：新装后 `runtime-options.json` 链接数为 1，`MetasequoiaImeServer.exe --production` 持续运行。
