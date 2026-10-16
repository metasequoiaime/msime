# Agent Note: 安装和卸载期间占住 TSF 的 Server 启动互斥量

Status: implemented

## Problem

在一台正用着水杉输入法的 Windows 11 上升级安装 full，[Files] 阶段报 `msime-others.db` DeleteFile 失败，错误码 32（文件被另一进程占用）。安装器在 `PrepareToInstall` 里已经按顺序停掉看门狗和 Server（`StopImeProcesses`），但进程列表显示看门狗和 Server 都是安装器启动之后才起来的。

把它们拉起来的是已经装着的 TIP。TIP 加载在每个正用着本输入法的程序里，安装向导自己的窗口也在其中：Profile 激活时（`OnActivated` → `_WakeServerIfNeeded`）或连续按键都连不上 Server 时，`RequestServerLaunch` 会经 `LaunchServerIfNeeded`（`platforms/windows/tsf/IME/MetasequoiaIME.cpp`）按注册表里的 `ServerPath` 启动 Server；被这样拉起的 Server 带 `--production`，又会拉起旁边的看门狗（`server_main.cpp` 的 `start_watchdog`）。Server 一启动就打开 `server\resources` 下的词库，安装器替换这些文件时就失败了。会不会撞上取决于安装过程中有没有在安装窗口或别的程序里激活、使用输入法，所以有时能装好、有时报错。

## Decision

`StopImeProcesses` 开头先调用 `HoldServerLaunchGate`，用 Inno 的 `CreateMutex` 创建 `Local\MetasequoiaImeServer.Launch<版本后缀>`。Inno 持有这个句柄到安装器退出，安装（`PrepareToInstall`）和卸载（`usUninstall`）两条路径都经过这里，而且都在停任何进程之前。

TIP 的 `LaunchServerIfNeeded` 在启动 Server 之前先创建同名互斥量：返回 `ERROR_ALREADY_EXISTS` 时当作别人正在拉起而直接返回；安装器是高完整性进程，中低完整性的 TIP 打开它时得到 `ERROR_ACCESS_DENIED`，同样直接返回。整个替换文件的过程里，TIP 因此不会再启动 Server。

版本后缀经 `edition_windows.py` 生成到 `editions.iss` 的新定义 `MyEditionNameSuffix`，与 C++ 侧的 `MSIME_EDITION_NAME_SUFFIX` 出自版本表的同一个字段 `name_suffix`。`scripts/test-windows-editions.py` 从 TIP 源码里读 `SERVER_LAUNCH_MUTEX_NAME` 的前缀，检查每个版本展开后的安装脚本都创建同名互斥量，并且在 `StopImeProcesses` 里先于 `StopProcessesUnder` 调用。

## Alternatives considered

- **改 TSF：安装期间不拉起 Server（例如检查一个「安装中」标记）** — 契约更明确，还能用 `Global\` 覆盖所有会话。否决：升级时加载在各程序里的是已经装着的旧 TIP，新 TIP 的逻辑要等这次安装完成、程序重新加载之后才生效，正好拦不住要拦的那一次。只能用旧 TIP 已经认的名字。
- **[Files] 条目加 `restartreplace` / `ignoreversion`，被占用就排到重启后替换** — Inno 原生支持，不碰进程。否决：Server 照样在用旧词库运行，新旧二进制和资源在重启前混在一起；而且问题不止一个文件，Server 打开的每一个资源都可能被占用。
- **在 [Files] 之前反复停进程、直到 Server 不再出现** — 不依赖 TIP 的内部名字。否决：TIP 随时可能在复制过程中再次拉起 Server，停多少次都留着窗口。

## Consequences

- **收益**：升级和卸载时，同一会话里的 TIP 不再把 Server 拉起来，词库和二进制的替换不再因为文件被占用而失败。不需要用户在安装前切换输入法。
- **代价与已知上限**：`Local\` 只覆盖安装器所在的会话。另一个同时登录的用户会话里的 TIP 仍可能拉起它自己的 Server，那种情况还会撞上同样的错误。安装器退出之后互斥量才释放，所以安装完成之前，本会话里的输入法连不上 Server（这本来就是装完之前的状态）。
- **重访信号**：TIP 的启动互斥量改名或改逻辑时，安装器要同步改，`test-windows-editions.py` 会在名字失配时失败；改用 `Global\` 或别的启动门控时，要先让旧 TIP 退出流通再删这条路径。

## Verification

- `scripts/test-windows-editions.py`、`scripts/test-editions.py`、`edition_windows.py gen --check` 通过。反向验证过两次：把互斥量名改错一个字符、或删掉 `StopImeProcesses` 里的 `HoldServerLaunchGate;`，检查都会失败。
- 本机 Inno Setup 6.7.3 用已暂存的 full 安装包输入编译 `msime_setup.iss`，ISCC 退出 0，确认 `CreateMutex` 作为支持函数可以解析。
- 未做端到端复现：没有在安装过程中驱动 TIP 去拉起 Server，验证新安装包确实不再报错 32。下一次在用着输入法的机器上升级安装时，在安装窗口里切换、使用输入法即可验证。

相关：[System32 部署与 `ExecNativeSys`](../feature/2026-10-08-windows-cs2-trusted-mode-system32-deployment.md)，`StopProcessesUnder` 为什么要用 64 位 PowerShell。
