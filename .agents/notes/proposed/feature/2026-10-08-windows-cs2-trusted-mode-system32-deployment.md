# Agent Note: CS2 Trusted Mode 的 System32 部署

Status: proposed

## Problem

[Windows 游戏里由水杉画候选窗](../../implemented/feature/2026-10-08-windows-game-candidate-overlay.md) 把 `cs2.exe` 放进了强制叠加的内置表，但前提是 TIP 能加载进 CS2。调研结论（未实机核实）是 CS2 的 Trusted Mode 只放行系统目录里的 DLL，拒绝装在 Program Files 下的模块；而安装器把 TSF DLL、宿主 DLL 和运行时 DLL 一起装在 `{commonpf64}\<install_dir>\msime_v<版本>\`（`platforms/windows/installer/msime_setup.iss` 的 TSF 段）。结果是只有加 `-insecure` 启动时水杉才进得去，而 `-insecure` 下用户不能匹配对战。

DLL 加载不进去时，候选窗、兜底定位都无从谈起，这一步也不是输入通道能绕开的。不解决的话，CS2 对水杉来说就是不可用。

## Proposal

先做实验，结论出来之前不改安装器。

实验（手工，在一台装好正式版的 x64 机器上）：

1. 把 64 位的 `MetasequoiaImeTsf.dll`、它链接的宿主 DLL（full 版是 `msime_host_api.dll`，各版本的名字见 `shared/contracts/editions.json` 的 `host_dll`）和同目录的运行时 DLL 复制到 `System32` 下，把 TIP 的 `InprocServer32` 改指向 System32 里的那份。
2. 不加 `-insecure` 启动 CS2，切到水杉打字：诊断日志里出现 `[game] process=cs2.exe … forced=1`，Server 日志没有 `Pipe identity rejected`，候选窗可见。
3. 分别只搬 TSF DLL、搬 TSF DLL 加宿主 DLL、全部搬过去，确认 Trusted Mode 到底检查哪些模块。
4. 用测试证书签名和正式证书签名各试一次，确认 Trusted Mode 认不认第三方签名的 System32 DLL。

实验通过后再设计安装器改动，要回答这些开放问题：

- **依赖 DLL 的搜索路径**：TSF DLL 通过 COM 按 `InprocServer32` 的完整路径加载，它依赖的宿主 DLL 和运行时 DLL 从哪里找、要不要一起放进 System32，要以第 3 步的结果为准（`platforms/windows/installer/README.md` 说明宿主 DLL 和运行时 DLL 都会被加载进宿主进程）。
- **文件名冲突**：System32 没有版本目录，也没有按版本区分的子目录。各版本（full、pinyin、wubi 等）的宿主 DLL 名字不同，但 TSF DLL 都叫 `MetasequoiaImeTsf.dll`，同时装两个版本会互相覆盖，System32 里的文件名必须按版本区分。
- **SysWOW64**：CS2 只有 64 位，32 位 TIP 留在 Program Files 是否就够；Windows on Arm 上 64 位 TIP 是 Arm64X，那份要不要同样处理。
- **注册路径**：`regserver` 写进注册表的是 DLL 所在路径，System32 和 Program Files 两份并存时，`InprocServer32` 指向哪一份；卸载和降级时怎么恢复。
- **升级时 DLL 被占用**：现在每个版本装进自己的 `msime_v<版本>` 目录，旧 DLL 被已打开的进程占着也不影响新版本。System32 里没有这层隔离，被占用的文件只能延迟到重启后替换，安装器要能处理「这次升级要重启才完整」。
- **范围**：System32 部署是所有用户默认都装，还是安装器里的一个可选项（只有需要 CS2 的用户才往系统目录写文件）。

## Alternatives considered

- **维持现状：CS2 只在 `-insecure` 下可用** — 不往系统目录写任何文件，安装器、升级和卸载都不用改，爆炸半径为零；内置表里的 `cs2.exe` 对 `-insecure` 已经生效。但 `-insecure` 下不能匹配对战，对真正玩 CS2 的用户等于不可用。
- **另做不依赖 DLL 加载的输入通道** — 不用碰系统目录，对所有 DLL 进不去的游戏（被反作弊挡掉的那些）都有效。但可行的形态要先抢焦点，独占全屏下会切出游戏，很多游戏失焦还会关掉聊天框；回填文本要 `SendInput` 或剪贴板，前者受 UIPI 限制、反作弊态度不明。CS2 的问题出在加载这一步，部署方案是直接对症的那条路。

## Acceptance criteria

- 实验记录写进本笔记：哪些模块必须进 System32、签名要求、`InprocServer32` 指向 System32 时 CS2 不加 `-insecure` 能加载 TIP 并显示候选窗。任一项不成立就把本篇转为 `Status: rejected — <原因>`。
- 安装器改动（如果做）：全新安装、覆盖升级（CS2 正在运行和没运行两种）、卸载、两个版本并存四种情况下，System32 里的文件和注册表都正确；`platforms/windows/installer/tests/` 的生命周期和 TSF 注册套件覆盖这些情况。
- 不玩 CS2 的用户在普通应用里的行为不变。

## Risks

- 往 System32 写文件的爆炸半径远大于 Program Files：装错、删错会影响所有加载 TIP 的进程，安全软件也更可能把它当成可疑行为。
- Trusted Mode 的放行规则是 Valve 未公开的实现，任何一次游戏更新都可能改变；部署方案随时可能失效，需要有办法从诊断日志里看出「DLL 没加载」。
- 升级时被占用的 System32 文件要等重启才能替换，期间新旧 DLL 可能和新旧 Server 混跑（混跑时的退化见 implemented 笔记的「代价与已知上限」）。
- 回滚：安装器改动若做成独立的可选组件，出问题时去掉组件即可恢复到只装 Program Files 的布局；做成默认安装的话，回滚要靠下一版安装器主动清理 System32 里的旧文件。
