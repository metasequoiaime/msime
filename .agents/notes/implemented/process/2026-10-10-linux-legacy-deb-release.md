# Agent Note: legacy .deb 随 Release Linux 发布，两个架构在原生 runner 上构建

Status: implemented

## Problem

[Debian 10 基线的 legacy .deb](../feature/2026-10-09-linux-legacy-glibc228-deb.md) 给 #6311 加了一条在 buster 容器里构建、只含 IBus 宿主的 `.deb` 构建线，但当时的范围是「不改发布工作流」，于是留下三处缺口：

- 用户拿不到它：发布页没有这个包，UOS 20、Debian 10 的用户要自己装 Docker、克隆仓库再跑 `package-legacy-container.sh`，而 #6311 的报告者要的正是一个能直接装的包。
- amd64 从没产出过：维护者手上的 Studio 是 arm64，Rosetta 下 Rust Release 构建估计要四五个小时，那次只构建了镜像。
- 没有任何东西会自动跑它：主线以后用到 Python 3.8+、IBus 1.5.20+、glibc 2.29+ 或 GCC 9+ 才有的东西时，要等有人手动跑这个脚本才会发现。

## Decision

- 新增 `.github/workflows/build-linux-legacy.yml`，矩阵是 `amd64` 在 `depot-ubuntu-24.04-8`、`arm64` 在 `depot-ubuntu-24.04-arm-8`，与 `release-linux.yml` 的 deb/rpm 用同一对原生 runner，`MSIME_LEGACY_ARCH` 等于 runner 架构，不经模拟。每个架构：取离线释义与语言词库（与发布页的包一致，`package-legacy-container.sh` 在暂存目录里有它们时才装）→ 跑 `package-legacy-container.sh`（编译、符号版本核对、干净 buster 里 apt 安装、ctest、IBus 1.5.19 运行时验收，一样不少）→ 核对产物恰好是 `msime-linux_<版本>_<架构>.deb`，包名、版本、架构字段都对 → 复制为 `msime-linux-legacy_<版本>_<架构>.deb`，对这个要上传的文件用 `check-elf-symbol-versions.sh` 按固定上限 `GLIBC=2.28 GLIBCXX=3.4.25 CXXABI=1.3.11 GCC=7.0.0` 再核一次 → 上传为 `msime-linux-legacy-deb-<架构>-<版本>`。
- 触发方式：
  - `workflow_call`：`release-linux.yml` 新增 `legacy` job 调用它，`ref` 是 `source` job 解析出的提交，`version` 与 deb/rpm 同样取输入或 `version.txt`。新增 dispatch 输入 `legacy`（布尔，缺省 true）；取消勾选时 job 跳过，发布照常。勾选了却失败时 `publish` 不发，与 deb、rpm 失败时一样。`publish` 按 `msime-linux-*-VERSION` 取附件，legacy 的产物名天然落在这个模式里，`SHA256SUMS` 本来就在 publish 里按全部附件重算。
  - `schedule`：每周一 02:23 UTC 在 develop 上跑一次，作为主线回归的兜底。
  - `pull_request`：只在 legacy 构建线自己的文件（这个 workflow、`package-legacy-container.sh`、`Dockerfile.legacy`、`legacy-runtime.sh`、`check-elf-symbol-versions.sh`）变化时跑。
  - `workflow_dispatch`：只构建不发布。
- 附件名用 `msime-linux-legacy_<版本>_<架构>.deb`，包内的 `Package` 仍是 `msime-linux`：
  - 不改名的话，legacy 与发布页的包同为 `msime-linux_<版本>_<架构>.deb`，`download-artifact` 的 `merge-multiple` 会让后到的覆盖先到的。
  - 检查更新挑包已移到 Rust（`crates/client-core/src/update_check.rs`）：给 full 挑包前先由 `is_other_edition_linux_package` 去掉 `msime-linux-<字母>` 开头的别的版本的包，`msime-linux-legacy_` 正好被它排除；各版本按 `msime-linux-<id>_` 前缀匹配，也不会匹配。`crates/client-core/src/update_check/tests.rs` 的发布夹具里加了两个架构的 legacy 附件，钉住这一点。版本表里将来不能有 id 为 `legacy` 的版本。
- 符号版本核对从 `package-legacy-container.sh` 的内联循环抽成 `platforms/linux/tests/tools/check-elf-symbol-versions.sh <deb 或目录> PREFIX=VERSION…`：容器里第 1 步照旧从 buster 的库读出上限后调用它，发布工作流对改名后的文件用固定上限调用它，人也可以拿它查任意一个包。ELF 判断改用文件头前四个字节，不依赖 `file`。

## Alternatives considered

- **Pull Request 上不跑，只在推到 develop 时按路径跑**：这是 [PR 上只编译和测试](2026-10-09-no-packaging-on-pull-requests.md) 对 macOS 打包的做法，理由是功能分支不该背打包的代价。没有照搬：那篇针对的是「碰到共享 crate 的无关 PR 被拖去打包」，这里的路径过滤只包含 legacy 构建线自己的五个文件，改它们的 PR 要验证的就是打包本身，而且 legacy 打包同时是这条线唯一的编译和测试（ctest、IBus 1.5.19 运行时验收都在脚本里）。不在 PR 上跑的话，改坏 `Dockerfile.legacy` 要等合并后才知道，amd64 也要等到合并后才第一次产出。
- **推到 develop 时只要 `platforms/linux/**` 变化就跑**：回归能当天报出。没有采用：develop 上一周有一百多次碰到 `platforms/linux` 的合并，每次两台 8 核 runner 各跑几十分钟的 buster Release 构建，成本远超收益；legacy 会被打坏的情形（新语法、新符号）不需要当天发现，下一次发版前发现就够，所以用每周定时加发版时必跑。
- **另起包名 `msime-linux-legacy`**：附件名和包名一致，用户不会被「文件名带 legacy、包名却是 msime-linux」弄糊涂。没有采用，理由与 [legacy 包那篇](../feature/2026-10-09-linux-legacy-glibc228-deb.md) 相同：两者装同一套 `/usr` 路径，另起包名就要写 `Conflicts`/`Replaces`，`msime-linux-setup`、维护脚本和 IBus 组件都要按新包名改写；只改附件名不改包名，升级、卸载的行为与发布页的包一致，README 里写明卸载用 `apt remove msime-linux`。
- **legacy 默认不随发布（`legacy` 缺省 false）**：发布内容不变，最保守。没有采用：#6311 要的就是一个发布页上能直接装的包，缺省关掉等于每次发版都要有人记得勾选；构建出问题时（例如 archive.debian.org 不可用）取消勾选就能照常发版。
- **在 bookworm 的 runner 上直接对发布页的包跑 `check-elf-symbol-versions.sh`**：不能代替 legacy 构建，发布页的包本来就要 `GLIBC_2.34` 起；它只是让任何人能确认某个包能不能上 glibc 2.28 的系统。

## Consequences

- **收益**：发布页上 amd64 与 arm64 都有 legacy 包，UOS 20、Debian 10 的用户不用自己构建；amd64 第一次在原生 x86_64 上完整走完构建、安装、ctest 和运行时验收；主线把 legacy 打坏时，最晚在下一个周一或下一次发版时报出来。
- **代价**：每次发版多两台 8 核 runner 各跑一条 buster 里的 Release 构建，发版的总时长可能被它拉长；每周再多一次同样的运行。buster 已停止维护，软件源只在 archive.debian.org 上，它不可用时 legacy 失败会挡住发布，要取消勾选 `legacy` 重发。
- **仍未覆盖**：没有在 UOS 20、麒麟真机或真实桌面会话里打字；没有 fcitx 4 前端，只用 fcitx 4 的系统要先切到 IBus；定时运行失败只出现在 Actions 页面，没有通知。

## Verification

- 本机：`actionlint .github/workflows/build-linux-legacy.yml .github/workflows/release-linux.yml` 通过；`shellcheck platforms/linux/tests/tools/check-elf-symbol-versions.sh` 通过；`cargo test -p msime-client-core update_check` 通过（夹具含两个架构的 legacy 附件）。
- PR #6639 上的 `Linux legacy package`（run 38009996039）：amd64 与 arm64 都通过。两边的 Depends 都是 `ibus (>= 1.5.19), python3 (>= 3.7), procps, libasound2 (>= 1.1.0), libc6 (>= 2.28), libgcc1 (>= 1:4.2), …`，读出的上限都是 `GLIBC_2.28 GLIBCXX_3.4.25 CXXABI_1.3.11 GCC_7.0.0`，18 个 ELF 文件都不超出；干净 buster 里 apt 安装、ctest 73 项全部通过，ibus-daemon 1.5.19-4+deb10u1 与 Python 3.7.3 上的运行时验收通过。改名后的 `msime-linux-legacy_0.11.0_<架构>.deb` 按固定上限再核一次，同样通过。amd64 这是第一次完整产出。
- 本机 Docker：`check-elf-symbol-versions.sh` 对 buster 自己的 `ibus`、`libibus-1.0-5`、`python3.7-minimal` 报告不超出；对 `linux-v0.11.0` 发布页的 `msime-linux_0.11.0_arm64.deb` 列出 `msime-voice-local` 要的 `GLIBC_2.32`–`2.34` 和 `GLIBCXX_3.4.26`/`3.4.29` 并以 1 退出。CI 产出的 arm64 legacy 包在 `debian:bullseye`（glibc 2.31）、`ubuntu:20.04`（2.31）和 `ubuntu:22.04` 里 apt 安装成功，`ldd` 无缺库、无缺符号版本，`msime-mcp --version` 能运行；这几个系统上没有跑运行时验收。
