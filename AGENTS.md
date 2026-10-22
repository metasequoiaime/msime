# AGENTS.md

给在本仓库工作的编码代理的操作约定。**架构边界、验证流程和各平台的验证覆盖面在 [ARCHITECTURE.md](docs/ARCHITECTURE.md)，贡献流程在 [CONTRIBUTING.md](.github/CONTRIBUTING.md)——先读那两份，这里只写它们没有覆盖的操作细节。**

## 改动范围

- 只暂存明确列出的路径，不用 `git add -A`。
- **一个 issue 一个 PR。** 处理 GitHub issue 时，每个 issue 单独一条分支、一个 PR，正文写 `Fixes #N`（只做了其中一部分时写 `Refs #N`，不自动关闭）。不要把几个 issue 合进同一个 PR，即使它们是同一次一起处理的。做某件事时顺带发现、没有对应 issue 的小问题，并进那件事的 PR，不必另开。#7045 把 7 个互不相关的 issue（AI 候选占位、Linux 设置窗口闪烁、DeepL、五笔查码、Android 图片发送、Caps Lock 切换、候选栏跟随光标）塞进一个横跨六个平台的 PR，审查、合并和回滚都没法按 issue 分开，几项还没补完的验证也跟着其余的一起进了 develop。见 `.agents/notes/implemented/process/2026-10-11-one-issue-one-pr.md`。
- 不复制相邻仓库的未提交内容；上游来源以远端实际默认分支和固定提交为准。
- **「把功能对齐某个平台」不等于「把默认值改成那个平台的出厂值」。** 对齐的验收标准是**能力可达**：字段存在、有宿主读、设置页有控件、行为能按用户选择生效。出厂默认值属于产品决策，会改变现有用户的手感，必须单独授权。做对齐任务时若发现默认值与参考平台不同，**只记录不改**，把差异表写进决策笔记（参考平台出厂值、本仓现值、为什么这次不动）。2026-10-22 一次「补齐 Windows 对齐」的 PR 顺手把 `helpcode.enabled`、`emoji`、`kaomoji`、`navigation.left_right` 等 7 个默认值翻成 Windows 出厂值，被维护者当场指出越界：这些翻转会静默改变所有 macOS 老用户的输入手感，且没有单独评审。
- 日志、测试和提交中不得包含真实输入、凭据或私人资料，测试数据使用合成值。
- CI 会在 Pull Request 上运行，提交前仍先跑本地 quick 门禁。发布 workflow 一律手动触发，不要在任务没有明确要求发布时去碰它们。
- **改了文件读写、目录遍历、权限或安卓资源（drawable、主题、布局），合并前在安卓模拟器上跑过再说「验证过」。** macOS 上的单测和 CI 的编译、lint 都碰不到 Android 的限制：应用对 `/data` 只有搜索权限、SELinux 禁止应用建硬链接、Android 9 的 `GradientDrawable` 读不到带主题属性的径向渐变半径。0.3.0 带着这三个问题发出去（词库准备失败、匿名账号写不进、Android 9 一打开就崩溃），编译、lint 和 JVM 测试全绿。本机做法：`bash platforms/android/tests/device/start-emulator.sh 28`（35 同理），`MSIME_ANDROID_TEST_SERIAL=emulator-5566 bash platforms/android/tests/device/build-core-test.sh` 编出 client-base、client-core、client-providers 和 client-services 的单测，经 cargo runner `run-core-test.sh` 在设备上整套运行，再用 `smoke.sh emulator-5566 --core` 装包验收（API 35 不带 `--core`）。CI 的 `android-device.yml` 在合进 develop 后和发版前跑同样的两步，PR 上不跑，所以 PR 上要靠本机跑过。
- **不要把 Tauri 生成的工程当成某个平台的产品去启动、调试或验收。** 每个平台的产品本体都是 `platforms/<os>` 下的原生宿主，Tauri/React 只是公共组件，而且不是每个宿主都承载它：共享 React 设置页只在 macOS、Linux（Tauri 设置应用）和 HarmonyOS（ArkWeb 加载的 rawfile，不经过 Tauri）上作为设置界面出现，Windows 的原生宿主只拉起 Tauri 的共享面板，iOS App 与 Android 发布 APK 两者都不含（见 docs/ARCHITECTURE.md 的「设置界面按平台归属」）。装机、启动和设备验收一律针对原生宿主；`apps/desktop` 的目录名和 Tauri 生成工程里自带的 bundle id 都不构成例外。

## 工具链

- **Xcode 27 的模拟器界面是 `/Applications/Xcode.app/Contents/Applications/DeviceHub.app`，`Simulator.app` 已经不存在了。** `open -a Simulator` 和 `open -b com.apple.iphonesimulator` 都会失败，而 `xcrun simctl` 的 boot、install、launch、screenshot 全都照常工作——于是很容易把「窗口没出现」误判成「模拟器没起来」。设备真实状态以 `xcrun simctl list devices` 为准，要看画面才需要开 DeviceHub。

## 本机安装

- **维护者在开发机上说「安装」（「更新安装」「切到 develop 安装」同理），指的是整套安装，不是只换输入法。** 做法是用 `platforms/macos/package-release.sh` 从当前分支打出完整的包（设置应用 `MSIME.app`，内嵌输入法 `水杉输入法.app`、核心词库和 `msime-mcp`），备份后替换 `/Applications/MSIME.app`，打开一次让它装好内嵌的输入法，再确认设置应用和输入法报的版本都是 `platforms/macos/version.txt` 里的版本。签名用钥匙串里 Developer ID 的 SHA-1（`MACOS_SIGNING_IDENTITY`），`MSIME_SPARKLE_ROOT` 指向固定版本的 Sparkle；包是 universal 的，Rust 部分按两个架构各编一遍，开编前按下文「Worktree」一节先查磁盘。
- 只跑 `platforms/macos/scripts/install.sh` 只换了输入法本体，设置应用和 `msime-mcp` 都还是旧的；`apps/desktop/src-tauri/tauri.conf.json` 的版本号也只在打包时被 `TAURI_CONFIG` 覆盖，不经打包构建出的设置应用一律报那个旧值。2026-10-19 连续几次「更新安装」都只换了输入法，用户打开的设置应用一直是 0.52.0，带着 develop 上早已修好的弹窗透底问题，版本号也对不上。

## 语言

本仓库的以下文本一律使用中文：

- 代码注释：新增或修改的注释，包括 Rust、Swift、C++、Kotlin、ArkTS、TypeScript、Python 和脚本中的注释与文档注释。
- Pull Request：标题和正文。
- 评论：PR 评论、issue 评论，以及对他人评论的回复。
- Code review：review 总结和每一条行内评论。

说明：

- 本规则优先于代理全局配置中「GitHub 上的文本用英文」之类的约定，只作用于本仓库。
- 代码中的标识符、命令、路径、错误码、日志字段和接口字段保持原样，不翻译；中文句子里引用它们时用反引号括起来。
- 修改已有的英文注释时，把改动到的那条注释改写成中文；不要为了改语言而批量重写没有改动的代码。
- PR 标题保留英文的 Conventional Commits 类型前缀，冒号后面写中文，例如 `fix(linux): 焦点进入时显示当前输入模式`。前缀不能翻译：`pr-triage.yml` 的 Conventional title 检查只接受英文类型，squash 合并后标题成为 develop 上的提交标题，release-please 也靠前缀归类，因此更新日志会是中文摘要。
- commit message 不在本规则范围内，格式见下一节。

## 提交

- 格式为 `type(scope): 摘要`，遵循 Conventional Commits。
- 每个可独立验证的切片单独提交。
- 不添加自动生成标记、AI 署名或 `Co-Authored-By` 水印。

## 决策笔记

非平凡改动必须带一篇笔记，写入 `.agents/notes/`；方法、格式与判定标准见 `.agents/skills/write-notes-like-deepseek/SKILL.md`。

- 判定标准、目录结构（`{lifecycle}/{class}/yyyy-mm-dd-topic.md`）、文件格式以 SKILL.md 为准，本节只写仓库约定：笔记正文一律用简体中文；仓库 CI（`quality.yml`）跑同样的三条校验。
- 动手前先检索旧笔记：有归属就地更新，不另起新篇；决定翻转才新建并互链。
- 新想法先写 `proposed/`，落地随同代码改动转 `implemented/`；被否且值得记的进 `rejected/`。
- 被放弃的方案先写它最强的理由，再解释为什么不用；确实没有真实备选时，`## Alternatives considered` 保留标题、正文写「无」，不要编造。
- 提交前跑 `pnpm run verify-notes`，红了先修再交。机械性小改（样式、格式化、打标、不改行为的补丁）不写笔记，直接提交。

## 发版

- 打版本一律从 `develop` 新建 `release/<版本号>` 分支（例如 `release/0.51.0`），由这个分支向 `main` 开 PR。禁止直接从 `develop` 向 `main` 开 PR 或合并，`branch-guard.yml` 的 Base branch 检查会拒绝这种 PR。
- 各平台版本号（`platforms/<os>/version.txt` 及随它一起改的文件，macOS 还有 `build-number.txt`）先经普通 PR 合入 `develop`，再从包含它的 `develop` 切 release 分支，让 `main` 上的版本号始终来自 `develop`。
- release 分支合入 `main` 用 merge commit，不要 squash：squash 会让 `develop` 的提交不在 `main` 的祖先里，下一次发版会在双方各自新增过的文件上冲突。
- 各平台的 Release 工作流按触发分支给发布加后缀：`main` 不加（`android-v0.1.4`），`develop` 加 `-beta`（`android-v0.1.4-beta`），其他分支加 `-alpha`。后缀只在 tag、标题和发布说明上，包内版本号仍是 `X.Y.Z`；带后缀的发布一律标为 prerelease，应用内更新检查不会提供它，Homebrew cask 和 Linux 发行版包定义也只在 `main` 的正式版之后更新。网页引擎只能从 `main` 发布，后缀只出现在其他分支的构建产物里。
- Release Android 的 `publish` 依赖 `device` job（`android-device.yml`，API 28 和 35 模拟器上跑 client-base、client-core、client-providers 和 client-services 单测和装包验收），它不过就不发。不要为了赶发版把这个依赖去掉；它失败时先看上传的 logcat，确认是设备环境问题还是产品问题。
- 网页引擎（Release Web Engine）勾选 `publish` 时只能在 `main` 上运行，workflow 会拒绝其他分支：它同时发布 npm 包 `@msime/web-engine`，npm 的 `latest` 会被接入方的版本范围和不带版本号的 CDN 地址自动取走。只构建不发布时任何分支都可以。

## Worktree

本仓库以大量短生命周期的 worktree 开发，一个任务一个。它们积累得很快，而且每个都带着自己的构建产物，所以放在哪里和什么时候清理都很重要。

- 一律建在仓库**外部**，放在 `~/worktrees/<feature>/` 或副盘上专门的 `worktrees/` 目录下。不要建在仓库内部、主工作区旁边或桌面上。
- **不要用 `/tmp` 或 `/var/tmp`。** 那些目录归系统管，随时可能被清理，放在那里的未提交工作已经因此丢失过。
- `<feature>` 用 kebab-case，从分支名或 issue 编号推出；不要加 `wt-` 前缀，父目录已经说明了它是什么。
- 一路提交，不要把所有东西攒到最后。分支是第一道防线，目录只是第二道：一个 WIP 提交不花什么代价，却能扛住检出目录出的任何事。
- **worktree 里不要重编没改过的那一半。** 新 worktree 的 `target/` 是空的，一次 Windows 客户端构建要从零编近 400 个 crate；而 Rust 那一半（三个架构的宿主 DLL、MCP server、Tauri 面板壳）不依赖原生代码，`Build-Client.ps1` 为此留了 `-RustOutputs <目录>`：指向已经编好的那一份，它一个 cargo 和 pnpm 命令都不跑，只编 C++（`release-windows.yml` 正是这样把一次 Rust 构建分给六个 edition）。所以改动只碰原生代码（TSF、Server、WinUI 设置）时，先在带着缓存的工作区把 Rust 产物编出来（`Build-RustOutputs.ps1 -Architecture x64|x86|arm64 -OutputDirectory <目录>`），再让 worktree 用 `-RustOutputs` 取。注意 `installer/test.ps1` 和 `Invoke-LocalInstall.ps1` 不透传这个参数，要走这条路就按 `Invoke-LocalInstall.ps1` 里的顺序自己串那六个阶段（构建、暂存、签 payload、编安装包、签安装包、安装）。反过来，Rust 侧有改动时两边必须同源：Server 和 TSF 是同一次构建的协议对，新旧混用的后果不是报错而是行为错乱。2026-10-22 修一个只改了 TSF C++ 的缺陷，却让 worktree 从零编 390 个 crate，报出「要一个小时」才发现这件事本来二十几分钟就能做完。
- 分支合并或废弃后立刻移除 worktree：`git worktree remove <path>`，然后 `git branch -D <branch>`。目录被手工删掉的话跑一次 `git worktree prune`。
- **PR 合并后不要在原分支上继续提交或推送，下一项工作从 `origin/develop` 新切分支。** 仓库开着合并后自动删除 head 分支，但本地分支的 upstream 还指着它，再 `git push` 一次就会把它在远端重新建出来，还带着跟这个分支名无关的新提交。2026-10-06/07 两天 audit 留下的 31 个「PR 已合并却还在」的远端分支里，28 个都是这样：合并后几秒被 GitHub 删掉，几十秒后又被推了回来。关闭而不合并的 PR，GitHub 不会删它的分支，关的时候顺手 `git push origin --delete <branch>`。
- **只用仓库规定的产物目录，不要自己另起 `CARGO_TARGET_DIR`。** 规定的目录是 `target/`，以及各平台脚本和 README 固定下来的 `target/<platform>-cargo`（`macos-cargo`、`android-cargo`、`ohos-cargo` 等）。原生工程的构建目录同理只用 README 和 `scripts/verify-local.sh` 固定的那一个：macOS 的 CMake 构建目录是 `target/macos-isolated`（`MSIME_MACOS_BUILD` 的默认值，`platforms/macos/README.md` 和 `launch-platform-host` skill 都用它）。它是 CMake 目录，不要拿它当 `CARGO_TARGET_DIR`，也不要在它旁边再另起一个 macOS 构建目录。每多一个目录，整棵依赖树就要从头再编一遍、再占几个 GB。2026-09-24 有一个 worktree 为不同平台和子任务分别建了 `build/rust-local-asr`、`build/macos-local-asr`、`build/tauri-local-asr`、`build/macos-crossfix`、`build/ios-local-asr`，单它一个就堆到 34 GB；另一个在 `target/macos-cargo` 旁边又建了 `target/macos-isolated`，比规定目录还大一倍。这类目录和另外五六个并行的 worktree 一起，一小时内把 461 GB 的盘写满了三次。产物目录里的东西坏了（典型是 `platforms/macos/README.md` 说的过程宏 dylib），删掉坏的那部分让它重编，不要换一个新目录绕开它。
- **起新的构建之前先看本机磁盘还剩多少，不够就不要在本机编。** 一个 worktree 的第一次完整构建就要占 6–24 GB，几个会话同时在各自的 worktree 里起构建，盘会在一小时内被写满，写满之后所有会话的编译、测试和 git 操作一起失败，连带别的项目也停摆。动手前用 `df -h ~` 看剩余空间：低于 50 GB 时不要在本机起新的构建树，先清理自己留下的产物目录（按下文的检查确认没有别的会话在用），或者改用远端编译机（维护者机器上的远端编译方式见各自的全局规则），不要指望编到一半再说。2026-10-18 本机盘在几个小时里被并行的构建写满了三次，2026-10-19 又写到只剩 141 MB。清理别的 worktree 的产物时，`lsof +D` 和 `pgrep` 要查整个 worktree 目录，不能只查 `target/`：`scripts/verify-local.sh` 这类脚本是在 worktree 里以相对路径运行的，进程的命令行里不带 worktree 的路径，在两次编译步骤之间 `target/` 下也可能暂时没有打开的文件，只查 `target/` 会把正在跑的验证误判成空闲。

清理不是可选的杂务。一个残留的 worktree 会让它完整的构建树一直活着——`target/`、`node_modules/` 和 `gradle-home/` 各自都是几个 GB——几十个被遗忘的 worktree 足以填满一块盘。

删除前先确认安全：分支已并入 `develop`（是 `origin/develop` 的祖先，或者 `git cherry origin/develop <branch>` 报告每个提交都已应用），并且 `git status --porcelain` 没有已跟踪文件的修改。其它情况一律不动，可能还有别的会话在里面工作。
