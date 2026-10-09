# Agent Note: CS2 Trusted Mode 的 System32 部署

Status: implemented

## Problem

[Windows 游戏里由水杉画候选窗](2026-10-08-windows-game-candidate-overlay.md) 把 `cs2.exe` 放进了强制叠加的内置表，但前提是 TIP 能加载进 CS2。安装器原来把 64 位 TSF DLL、宿主 DLL 和运行时 DLL 一起装在 `{commonpf64}\<install_dir>\msime_v<版本>\`，而 CS2 的 Trusted Mode 拒绝加载系统目录以外的外来 DLL：水杉只有加 `-insecure` 或 `-allow_third_party_software` 启动时才进得去，前者不能匹配对战。CS2 是 Steam 上玩家最多的游戏之一，DLL 加载不进去时候选窗、兜底定位都无从谈起，这一步也不是输入通道能绕开的。

## Decision

64 位 TIP（Windows on Arm 上是 Arm64X 那一份）连同它从同目录加载的宿主 DLL 和运行时 DLL，装进 `{sys}\IME\<install_dir>\msime_v<版本>\`（`msime_setup.iss` 的 `MySystemTipDir`），并在那里 `regserver`，`InprocServer32` 和 TSF profile 的图标路径都指向系统目录里的这一份。32 位 TIP 留在 `{commonpf32}`，Server 和其余程序仍在 Program Files。

依据（调研，均未在本仓实机核实）：

- Valve 的 Steam 支持页（help.steampowered.com/en/faqs/view/09A0-4879-4353-EF95）只说 Trusted Mode 下外来软件一律被拦、没有白名单，正常模式下注入的 DLL 必须签名；没有公开放行规则。
- `cs2.exe` 导入 `WinVerifyTrust`、`CryptCATAdmin*`、`GetSystemDirectoryW`（SteamTracking/GameTracking-CS2 的字符串表），与「系统目录 + 有效 Authenticode 签名」的判据一致。被拒的 DLL 在 `Steam\userdata\<id>\730\local\cfg\trustedlaunch.cfg` 和控制台留下 `Denied a request to load '<路径>'`。
- 实测先例来自第三方输入法：清风输入法（huanfeng/WindInput）同样用 Certum 证书签名，装在 Program Files 时被拒，2026-09 改装到 `System32\IME\WindInput\` 后能用；QQ 五笔装在 `System32\IME\QQWubiTSF\`，也能用。两者都在 `System32\IME` 的子目录里，不要求直接放在 `System32` 下，也不要求 EV 证书。
- 2020 年对 CS:GO 的逆向显示拦截点是一个检查所有可执行映射的 `NtOpenFile` 钩子，所以 TIP 从同目录加载的宿主 DLL 和运行时 DLL 大概率也逐个过检，它们必须和 TIP 在同一个系统子目录里、同样签名。发布流程（`Sign-PackageBinaries-SimplySign.ps1`）本来就给 `tsf_dll` 下的每个 DLL 签名。

安装器的处理：

- 用本版本自己的子目录 `System32\IME\<install_dir>\` 而不是直接放进 `System32`：各版本的 TIP 都叫 `MetasequoiaImeTsf.dll`，运行时 DLL 也是 vcpkg 的通用名字，放在一起会互相覆盖，也会和别的软件撞名。版本子目录沿用 Program Files 那套 `msime_v<版本>[.n]`，升级时新版本进新目录，被已打开进程占着的旧 DLL 不影响安装。
- 用 Inno Setup 7 编出的 Setup 是 32 位进程，`[Code]` 的文件函数和 `Exec` 都被 WOW64 重定向到 SysWOW64，只有 `ExecWithNativeSysDir` 绕开；Inno 6 在 64 位安装模式下两者本来就绕开。CI 用 6.7.1，本机构建优先用 7，签名发布用的是本机构建的那份。所以系统目录只经 `ExecNativeSys`（按 ISPP 版本选 `ExecWithNativeSysDir` 或 `Exec`）启动的 `cmd.exe`（`SystemDirExists`）和 64 位 PowerShell（`TryDeleteSystemVersionDirs`，进程不是 64 位时什么都不删）去查和删，`[Dirs]`、`[Files]` 由 Inno 按 64 位模式映射。按路径停 Server 的 `StopProcessesUnder` 原来在 Inno 7 下启动的是 32 位 PowerShell，读不到 64 位进程的映像路径、什么都停不掉，这次一并改走 `ExecNativeSys`。
- 系统目录里的文件带 `uninsrestartdelete`：卸载时被占用的排到重启后删除。`GetVersionDir` 因此也避开系统目录里已经存在的版本目录，否则重启前重装同一版本，新装的 TIP 会在重启时被一起删掉。
- 升级时 `PrepareToInstall` 删掉系统目录里本版本以外的版本目录，卸载时删掉整个 `System32\IME\<install_dir>`，被占用的留到下一次。
- TSF DLL 自己的代码不需要改：它不按自己所在的目录拼任何路径，Server 路径和数据目录都从注册表读（`ReadServerPath`、`StateDirectory.h`），宿主 DLL 是普通导入，从 TIP 所在目录找到。

## Alternatives considered

- **维持现状：CS2 只在 `-insecure` 下可用** — 不往系统目录写任何文件，安装器、升级和卸载都不用改，爆炸半径为零。但 `-insecure` 下不能匹配对战，对真正玩 CS2 的用户等于不可用，而这个群体很大。
- **只把 64 位 TIP 放进 System32，宿主和运行时 DLL 留在 Program Files** — 系统目录里只多一个文件。但宿主 DLL 是 TIP 的普通导入，加载器先在 TIP 所在目录里找；而且拦截点检查每一个可执行映射，留在 Program Files 的依赖照样会被拒。
- **直接放进 `System32` 根目录，按版本给文件改名** — 和微软自带输入法的位置一样。但要给每个版本的 TIP、宿主和每个运行时 DLL 都起不冲突的名字，TIP 的导入表还写死了宿主 DLL 的名字；版本子目录已有两个实测通过的先例，不需要冒这个险。
- **做成安装器里的可选组件，只有需要 CS2 的用户才往系统目录写文件** — 不玩 CS2 的用户系统目录不受影响。但两种布局要分别注册、分别测试、分别升级和卸载，`InprocServer32` 指向哪一份取决于安装选项；而系统目录副本对其他应用没有坏处，微软拼音、QQ 五笔、搜狗都装在系统目录里。
- **另做不依赖 DLL 加载的输入通道** — 对所有 DLL 进不去的游戏都有效。但可行的形态要先抢焦点，独占全屏下会切出游戏，回填文本要 `SendInput` 或剪贴板；CS2 的问题出在加载这一步，部署是直接对症的那条路。
- **冒用 Source 2 白名单里别家输入法的 profile 描述，让游戏自己画候选** — 游戏原生的候选界面。但等于冒充别家产品，而且水杉的候选窗已经由强制叠加画出来，不需要游戏画。

## Consequences

- 64 位 TIP 的 `InprocServer32` 指向 `C:\Windows\System32\IME\<install_dir>\msime_v<版本>\MetasequoiaImeTsf.dll`。`install-smoke.ps1`、`coexistence-smoke.ps1` 按这个位置核对注册、宿主 DLL 和卸载后的清理；`tsf-registration.ps1`、`lifecycle.ps1` 在 CI 上核对安装脚本的条目和清理调用。
- 往系统目录写文件的爆炸半径比 Program Files 大：装错、删错会影响所有加载 TIP 的进程，安全软件也更可能把它当成可疑行为。清理只删 `System32\IME\<install_dir>` 下的 `msime_v*`，不碰别的目录。
- 只有签了名的发布包才过得了 Trusted Mode。CI 产出的未签名安装包装出来的 TIP 在 CS2 里仍会被拒，这不是部署问题。
- 经 SignPath 签名的安装包（[CI 经 SignPath 签名](../../implemented/process/2026-10-08-windows-signpath-ci-signing.md)）只签 TIP 和宿主 DLL：SignPath Foundation 不签上游代码，TIP 旁边 vcpkg 构建的 `fmt.dll`、`libcurl.dll`、`zlib1.dll` 保持未签名。按上面的拦截点推断，这一份在 Trusted Mode 下大概率仍被拒；要让它过，TIP 和宿主 DLL 得把这几个依赖静态链接进去，让系统目录里只剩本项目签名的 DLL。用 SimplySign 在本地签的安装包不受影响，它连这几个 DLL 一起签。
- Trusted Mode 的规则是 Valve 未公开的实现，任何一次游戏更新都可能改变。Valve 自己的文案里还有一条「从 Windows 系统目录加载了外来软件，会话已降级为 allow third party software 模式」（`SFUI_FileVerification_NotTrustedLaunch2`），签了名的系统目录 DLL 到底是静默放行还是放行但降级，调研没能排除。
- 升级时被占用的旧版本目录要等下一次安装或重启才清掉；卸载后被占用的文件要等重启。

## Verification

- CI：`tsf-registration.ps1`（64 位和 Arm64X TIP 及其依赖装进 `{#MySystemTipDir}\{code:GetVersionDir}` 并带 `uninsrestartdelete`，32 位留在 `{commonpf32}`）、`lifecycle.ps1`（系统目录只经 64 位 PowerShell 清理，升级保留当前版本、卸载全删，`GetVersionDir` 查系统目录）、`scripts/test-windows-editions.py`（六个版本都能展开 `MySystemTipDir`）。
- 发布流程的 `install-smoke.ps1`、`coexistence-smoke.ps1` 在真实的 Windows runner 上安装、升级、卸载并核对系统目录。
- 尚待实机确认（签名的正式安装包，x64）：不加任何启动项启动 CS2，切到水杉打字。
  - `trustedlaunch.cfg` 和控制台里没有本 DLL 的 `Denied a request to load`。
  - 主菜单没有出现「downgraded to 'allow third party software'」提示，能进入官方匹配。
  - 诊断日志里有 `[game] process=cs2.exe … forced=1`，候选窗可见。
  - 任一项不成立，就在这里记下实测结果，并另开一篇笔记处理。
