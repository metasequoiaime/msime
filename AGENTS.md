# AGENTS.md

给在本仓库工作的编码代理的操作约定。**架构边界、验证流程和各平台的验证覆盖面在 [ARCHITECTURE.md](ARCHITECTURE.md)，贡献流程在 [CONTRIBUTING.md](CONTRIBUTING.md)——先读那两份，这里只写它们没有覆盖的操作细节。**

## 改动范围

- 只暂存明确列出的路径，不用 `git add -A`。
- 不复制相邻仓库的未提交内容；上游来源以远端实际默认分支和固定提交为准。
- 日志、测试和提交中不得包含真实输入、凭据或私人资料，测试数据使用合成值。
- CI 会在 Pull Request 上运行，提交前仍先跑本地 quick 门禁。发布 workflow 一律手动触发，不要在任务没有明确要求发布时去碰它们。
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

## 发版

- 打版本一律从 `develop` 新建 `release/<版本号>` 分支（例如 `release/0.51.0`），由这个分支向 `main` 开 PR。禁止直接从 `develop` 向 `main` 开 PR 或合并，`branch-guard.yml` 的 Base branch 检查会拒绝这种 PR。
- 各平台版本号（`platforms/<os>/version.txt` 及随它一起改的文件，macOS 还有 `build-number.txt`）先经普通 PR 合入 `develop`，再从包含它的 `develop` 切 release 分支，让 `main` 上的版本号始终来自 `develop`。
- release 分支合入 `main` 用 merge commit，不要 squash：squash 会让 `develop` 的提交不在 `main` 的祖先里，下一次发版会在双方各自新增过的文件上冲突。

## Worktree

本仓库以大量短生命周期的 worktree 开发，一个任务一个。它们积累得很快，而且每个都带着自己的构建产物，所以放在哪里和什么时候清理都很重要。

- 一律建在仓库**外部**，放在 `~/worktrees/<feature>/` 或副盘上专门的 `worktrees/` 目录下。不要建在仓库内部、主工作区旁边或桌面上。
- **不要用 `/tmp` 或 `/var/tmp`。** 那些目录归系统管，随时可能被清理，放在那里的未提交工作已经因此丢失过。
- `<feature>` 用 kebab-case，从分支名或 issue 编号推出；不要加 `wt-` 前缀，父目录已经说明了它是什么。
- 一路提交，不要把所有东西攒到最后。分支是第一道防线，目录只是第二道：一个 WIP 提交不花什么代价，却能扛住检出目录出的任何事。
- 分支合并或废弃后立刻移除 worktree：`git worktree remove <path>`，然后 `git branch -D <branch>`。目录被手工删掉的话跑一次 `git worktree prune`。
- **只用仓库规定的产物目录，不要自己另起 `CARGO_TARGET_DIR`。** 规定的目录是 `target/`，以及各平台脚本和 README 固定下来的 `target/<platform>-cargo`（`macos-cargo`、`android-cargo`、`ohos-cargo` 等）。原生工程的构建目录同理只用 README 和 `scripts/verify-local.sh` 固定的那一个：macOS 的 CMake 构建目录是 `target/macos-isolated`（`MSIME_MACOS_BUILD` 的默认值，`platforms/macos/README.md` 和 `launch-platform-host` skill 都用它）。它是 CMake 目录，不要拿它当 `CARGO_TARGET_DIR`，也不要在它旁边再另起一个 macOS 构建目录。每多一个目录，整棵依赖树就要从头再编一遍、再占几个 GB。2026-09-24 有一个 worktree 为不同平台和子任务分别建了 `build/rust-local-asr`、`build/macos-local-asr`、`build/tauri-local-asr`、`build/macos-crossfix`、`build/ios-local-asr`，单它一个就堆到 34 GB；另一个在 `target/macos-cargo` 旁边又建了 `target/macos-isolated`，比规定目录还大一倍。这类目录和另外五六个并行的 worktree 一起，一小时内把 461 GB 的盘写满了三次。产物目录里的东西坏了（典型是 `platforms/macos/README.md` 说的过程宏 dylib），删掉坏的那部分让它重编，不要换一个新目录绕开它。

清理不是可选的杂务。一个残留的 worktree 会让它完整的构建树一直活着——`target/`、`node_modules/` 和 `gradle-home/` 各自都是几个 GB——几十个被遗忘的 worktree 足以填满一块盘。

删除前先确认安全：分支已并入 `develop`（是 `origin/develop` 的祖先，或者 `git cherry origin/develop <branch>` 报告每个提交都已应用），并且 `git status --porcelain` 没有已跟踪文件的修改。其它情况一律不动，可能还有别的会话在里面工作。
