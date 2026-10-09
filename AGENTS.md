# AGENTS.md

给在本仓库工作的编码代理的操作约定。**架构边界、验证流程和各平台的验证覆盖面在 [ARCHITECTURE.md](ARCHITECTURE.md)，贡献流程在 [CONTRIBUTING.md](CONTRIBUTING.md)——先读那两份，这里只写它们没有覆盖的操作细节。**

## 改动范围

- 只暂存明确列出的路径，不用 `git add -A`。
- 不复制相邻仓库的未提交内容；上游来源以远端实际默认分支和固定提交为准。
- 日志、测试和提交中不得包含真实输入、凭据或私人资料，测试数据使用合成值。
- CI 会在 Pull Request 上运行，提交前仍先跑本地 quick 门禁。发布 workflow 一律手动触发，不要在任务没有明确要求发布时去碰它们。
- **改了文件读写、目录遍历、权限或安卓资源（drawable、主题、布局），合并前在安卓模拟器上跑过再说「验证过」。** macOS 上的单测和 CI 的编译、lint 都碰不到 Android 的限制：应用对 `/data` 只有搜索权限、SELinux 禁止应用建硬链接、Android 9 的 `GradientDrawable` 读不到带主题属性的径向渐变半径。0.3.0 带着这三个问题发出去（词库准备失败、匿名账号写不进、Android 9 一打开就崩溃），编译、lint 和 JVM 测试全绿。本机做法：`bash platforms/android/tests/device/start-emulator.sh 28`（35 同理），`bash platforms/android/tests/device/build-core-test.sh` 编出 client-core 单测后用 `MSIME_ANDROID_TEST_SERIAL=emulator-5566 bash platforms/android/tests/device/run-core-test.sh <程序>` 在设备上整套运行，再用 `smoke.sh emulator-5566 --core` 装包验收（API 35 不带 `--core`）。CI 的 `android-device.yml` 在合进 develop 后和发版前跑同样的两步，PR 上不跑，所以 PR 上要靠本机跑过。
- **不要把 Tauri 生成的工程当成某个平台的产品去启动、调试或验收。** 每个平台的产品本体都是 `platforms/<os>` 下的原生宿主，Tauri/React 只是它承载的公共组件（见 ARCHITECTURE.md）。装机、启动和设备验收一律针对原生宿主；`apps/desktop` 的目录名和 Tauri 生成工程里自带的 bundle id 都不构成例外。

## 工具链

- **Xcode 27 的模拟器界面是 `/Applications/Xcode.app/Contents/Applications/DeviceHub.app`，`Simulator.app` 已经不存在了。** `open -a Simulator` 和 `open -b com.apple.iphonesimulator` 都会失败，而 `xcrun simctl` 的 boot、install、launch、screenshot 全都照常工作——于是很容易把「窗口没出现」误判成「模拟器没起来」。设备真实状态以 `xcrun simctl list devices` 为准，要看画面才需要开 DeviceHub。

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

非平凡改动（改了行为、架构、跨文件契约、流程与工具链、测试策略、落盘/网络/配置格式，或其他维护者日后可能重访的决定）必须带一篇笔记，写入 `.agents/notes/`；方法、格式与判定标准见 `.agents/skills/write-notes-like-deepseek/SKILL.md`。

- 动手前先检索旧笔记：有归属就地更新，不另起新篇；决定翻转才新建并互链。
- 新想法先写 `proposed/`，落地随同代码改动转 `implemented/`；被否且值得记的进 `rejected/`。
- 被放弃的方案先写它最强的理由，再解释为什么不用。
- 提交前跑 `pnpm run verify-notes`，红了先修再交。机械性小改（样式、格式化、打标、不改行为的补丁）不写笔记，直接提交。

## 发版

- 打版本一律从 `develop` 新建 `release/<版本号>` 分支（例如 `release/0.51.0`），由这个分支向 `main` 开 PR。禁止直接从 `develop` 向 `main` 开 PR 或合并，`branch-guard.yml` 的 Base branch 检查会拒绝这种 PR。
- 各平台版本号（`platforms/<os>/version.txt` 及随它一起改的文件，macOS 还有 `build-number.txt`）先经普通 PR 合入 `develop`，再从包含它的 `develop` 切 release 分支，让 `main` 上的版本号始终来自 `develop`。
- release 分支合入 `main` 用 merge commit，不要 squash：squash 会让 `develop` 的提交不在 `main` 的祖先里，下一次发版会在双方各自新增过的文件上冲突。
- 各平台的 Release 工作流按触发分支给发布加后缀：`main` 不加（`android-v0.1.4`），`develop` 加 `-beta`（`android-v0.1.4-beta`），其他分支加 `-alpha`。后缀只在 tag、标题和发布说明上，包内版本号仍是 `X.Y.Z`；带后缀的发布一律标为 prerelease，应用内更新检查不会提供它，Homebrew cask 和 Linux 发行版包定义也只在 `main` 的正式版之后更新。网页引擎只能从 `main` 发布，后缀只出现在其他分支的构建产物里。
- Release Android 的 `publish` 依赖 `device` job（`android-device.yml`，API 28 和 35 模拟器上跑 client-core 单测和装包验收），它不过就不发。不要为了赶发版把这个依赖去掉；它失败时先看上传的 logcat，确认是设备环境问题还是产品问题。
- 网页引擎（Release Web Engine）勾选 `publish` 时只能在 `main` 上运行，workflow 会拒绝其他分支：它同时发布 npm 包 `@msime/web-engine`，npm 的 `latest` 会被接入方的版本范围和不带版本号的 CDN 地址自动取走。只构建不发布时任何分支都可以。

## Worktree

本仓库以大量短生命周期的 worktree 开发，一个任务一个。它们积累得很快，而且每个都带着自己的构建产物，所以放在哪里和什么时候清理都很重要。

- 一律建在仓库**外部**，放在 `~/worktrees/<feature>/` 或副盘上专门的 `worktrees/` 目录下。不要建在仓库内部、主工作区旁边或桌面上。
- **不要用 `/tmp` 或 `/var/tmp`。** 那些目录归系统管，随时可能被清理，放在那里的未提交工作已经因此丢失过。
- `<feature>` 用 kebab-case，从分支名或 issue 编号推出；不要加 `wt-` 前缀，父目录已经说明了它是什么。
- 一路提交，不要把所有东西攒到最后。分支是第一道防线，目录只是第二道：一个 WIP 提交不花什么代价，却能扛住检出目录出的任何事。
- 分支合并或废弃后立刻移除 worktree：`git worktree remove <path>`，然后 `git branch -D <branch>`。目录被手工删掉的话跑一次 `git worktree prune`。
- **PR 合并后不要在原分支上继续提交或推送，下一项工作从 `origin/develop` 新切分支。** 仓库开着合并后自动删除 head 分支，但本地分支的 upstream 还指着它，再 `git push` 一次就会把它在远端重新建出来，还带着跟这个分支名无关的新提交。2026-10-06/07 两天 audit 留下的 31 个「PR 已合并却还在」的远端分支里，28 个都是这样：合并后几秒被 GitHub 删掉，几十秒后又被推了回来。关闭而不合并的 PR，GitHub 不会删它的分支，关的时候顺手 `git push origin --delete <branch>`。
- **只用仓库规定的产物目录，不要自己另起 `CARGO_TARGET_DIR`。** 规定的目录是 `target/`，以及各平台脚本和 README 固定下来的 `target/<platform>-cargo`（`macos-cargo`、`android-cargo`、`ohos-cargo` 等）。原生工程的构建目录同理只用 README 和 `scripts/verify-local.sh` 固定的那一个：macOS 的 CMake 构建目录是 `target/macos-isolated`（`MSIME_MACOS_BUILD` 的默认值，`platforms/macos/README.md` 和 `launch-platform-host` skill 都用它）。它是 CMake 目录，不要拿它当 `CARGO_TARGET_DIR`，也不要在它旁边再另起一个 macOS 构建目录。每多一个目录，整棵依赖树就要从头再编一遍、再占几个 GB。2026-09-24 有一个 worktree 为不同平台和子任务分别建了 `build/rust-local-asr`、`build/macos-local-asr`、`build/tauri-local-asr`、`build/macos-crossfix`、`build/ios-local-asr`，单它一个就堆到 34 GB；另一个在 `target/macos-cargo` 旁边又建了 `target/macos-isolated`，比规定目录还大一倍。这类目录和另外五六个并行的 worktree 一起，一小时内把 461 GB 的盘写满了三次。产物目录里的东西坏了（典型是 `platforms/macos/README.md` 说的过程宏 dylib），删掉坏的那部分让它重编，不要换一个新目录绕开它。

清理不是可选的杂务。一个残留的 worktree 会让它完整的构建树一直活着——`target/`、`node_modules/` 和 `gradle-home/` 各自都是几个 GB——几十个被遗忘的 worktree 足以填满一块盘。

删除前先确认安全：分支已并入 `develop`（是 `origin/develop` 的祖先，或者 `git cherry origin/develop <branch>` 报告每个提交都已应用），并且 `git status --porcelain` 没有已跟踪文件的修改。其它情况一律不动，可能还有别的会话在里面工作。
