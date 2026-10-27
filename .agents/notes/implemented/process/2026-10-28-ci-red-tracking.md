# Agent Note: develop 红灯自动开单、APK 体积预警和 PR 上的 Fedora ctest

Status: implemented

## Problem

2026-10-25 到 10-27，`Nightly builds` 每天都红，三天里没有人发现。合进 develop 后的 `Native Platform CI`（push 事件）最近 30 次运行里只有 3 次成功、13 次失败、12 次取消，失败大多来自 `android-device.yml` 的设备套件间歇失败。设备套件按[PR 上只编译和测试](2026-10-09-no-packaging-on-pull-requests.md)只在合并后跑，那篇笔记的 Consequences 里已经写了「develop 上失败时没有自动回滚或通知，靠看 develop 的 Actions 页面」。

红成常态之后，真正的回归混在里面进了 develop，没人看见：

- Android 拼音版瘦包 135,623,964 字节，超过 `build-apk.sh` 135,000,000 字节的上限，nightly 的 `release (pinyin)` 失败。瘦包一直贴着上限走（[五笔码表按需下载](../feature/2026-10-28-android-on-demand-wubi.md)实测 128,435,407 字节，余量约 6.6 MB），没有任何提前提示。
- `linux-candidate-native-canvas-chrome` 只在 Fedora 44 的打包容器里失败（`release-linux.yml` 的 rpm 作业以 Release 构建并跑 ctest）。PR 上的「Linux native host」只在 Debian bookworm 容器里编 Debug，从来跑不到它。

维护者（houko）决定现在就补上下面四件事。设备套件本身的间歇失败另有改动在修，自动重试或隔离间歇测试是之后单独的改动，都不在本篇。

## Decision

### 红灯跟踪 issue

`.github/workflows/ci-red-tracker.yml` 以 `workflow_run`（`completed`）监听 `Nightly builds`、`Native Platform CI`、`Core CI`、`iOS CI`、`macOS CI`、`macOS package`，只处理 develop 上 push、schedule、workflow_dispatch 触发的运行，pull_request 的运行一律不管。判定和文案在 `scripts/ci_issue_tracker.py red-build`：

- 每个 workflow 至多一张开着的跟踪单，标签 `ci-red`（不存在就 `gh label create --force` 建），指派 houko。哪张单属于哪个 workflow 靠正文里的 `<!-- ci-red-tracker workflow=<workflow 文件路径> -->` 认，不靠标题。nightly 和 develop 的推送 CI 各自一张，不会一张红单盖住另一个 workflow 的失败。
- 失败或超时：没有开着的单就新开，标题「<workflow 名> 失败：<运行日期>」（UTC），正文有运行链接、提交号和提交标题、失败的 job 名及链接，以及从上一次成功以来第一次失败的运行和提交、连续失败的次数。已有单就追加一条评论。每次运行（含 attempt）带一个标记，同一事件重复送达不会重复评论。
- 成功：有开着的单就评论「已转绿」并关闭；没有就什么都不做。取消和跳过的运行既不算红也不算绿。
- 只让同一 workflow 最新一次有结论的运行改变状态。`workflow_run` 事件可能乱序到达，更旧的失败不能重开已经转绿的单，更旧的成功也不能关掉更新的失败开的单。
- 并发组按被监听的 workflow 划分，`cancel-in-progress: false`，两次运行同时结束时不会各开一张单。GitHub 的并发组只保留一个排队者，排队的那次被更新的一次顶掉时，更新的那次按运行历史重新算出起点，不丢状态。
- 权限只有 `issues: write`、`actions: read`、`contents: read`，只用 `GITHUB_TOKEN` 和 runner 自带的 `gh`，不引入新的 action。它检出默认分支上的脚本，不检出、不执行被跟踪运行的代码。写进 issue 的只有 job 名、链接、提交号和提交标题，不放日志。提交标题放进代码样式，里面的 `@` 提及和 `#` 编号不会通知人或链出去。

### APK 体积预警线

`build-apk.sh` 的 135,000,000 字节硬上限不变，在它下面加一条 132,000,000 字节的预警线（`MSIME_ANDROID_WARN_APK_BYTES` 可改，不能高于上限）。越过预警线时照常出包，但打印警告和 `::warning::` 注解。`GITHUB_STEP_SUMMARY` 存在时总是写上体积和两条线，超过上限而失败的构建也写。设了 `MSIME_ANDROID_SIZE_REPORT` 时，GitHub 渠道的 APK 体积另写成一行 JSON：瘦包带两条线，其余版本没有上限，两条线为 null。

`release-android.yml` 给每个版本设这个路径，不论构建成败都上传 `android-apk-size-<版本>` artifact。名字不以 `msime-android-` 开头，不会被 publish 和 nightly 汇总当成安装包收走。nightly 新增 `android-size` 作业，跟在 android 之后，只在 develop 上运行。它下载这些报告，调用 `scripts/ci_issue_tracker.py apk-size`：任一瘦包越过预警线时开或更新一张 `apk-size` issue，正文是各版本体积、两条线、较上次的变化、距上限的余量和处理提示，评论记每一次。所有版本都有报告、瘦包全部回到线下时关闭。有版本没报告时不关，因为证明不了它也回到了线下。nightly 通过 job 间的 artifact 传数据，与它汇总各平台安装包的方式相同。

### PR 上的 Fedora ctest

`package-container.sh` 新增 `MSIME_PACKAGE_TESTS_ONLY=1`：同一个镜像、同样的 Release 构建、同样的 CMake 配置（包括 `MSIME_ENABLE_PACKAGING=ON` 和语音运行库，后者还多注册一项 ctest），跑完 full 版的 ctest 就停，不跑 cpack、不写 SHA256SUMS、不出任何包。版本缺省只有 full，桌面二进制缺省不编。`ci-platforms.yml` 新增「Linux native host (Fedora)」，只在 `needs.changes.outputs.linux == 'true'` 时以 `MSIME_PACKAGE_FORMAT=rpm MSIME_PACKAGE_TESTS_ONLY=1` 运行它，runner 和 45 分钟超时与「Linux native host」一致；平台没改动时和其他作业一样只打印一行就结束。这是测试运行，不是打包，不违反「PR 上不打包」。

没有另写一份 Fedora 构建脚本，是因为 rpm 作业的构建步骤本来就在 `package-container.sh` 里，打包只是循环末尾的 cpack 和最后的 SHA256SUMS。在那里加一个开关，PR 跑的就是发版的那条配置；`scripts/test-linux-distro-packaging.py` 比对的 CMake 选项和 cargo 目标一个都没变。

### 先修红的约定

`AGENTS.md` 增加一条：develop 的推送 CI 或 nightly 红着时先修红；修好之前不合并功能 PR，除非失败已经在 `ci-red` issue 里写明原因、确认与要合的 PR 无关。代理开 PR 前先看 `gh run list --branch develop` 和 `ci-red` issue，有红的就在 PR 正文写明。

## Alternatives considered

- **发 Slack 或邮件通知**：推送比 issue 更醒目，失败当时就有人看到。没有采用：仓库没有 Slack 工作区，邮件和 GitHub 自带的失败通知只发给触发运行的人；定时的 nightly 不对应任何人，推送 CI 的通知也只到合并者手里，而且它们和每次 PR 运行的通知混在一起，三天没人发现就是这样来的。issue 有指派、有状态、能在 PR 里引用，「修好之前不合」也需要一个能链接、能写原因的地方。
- **把设备套件放回 PR 上跑**：回归在合并前拦下，责任最清楚。没有采用：[PR 上只编译和测试](2026-10-09-no-packaging-on-pull-requests.md)的理由仍然成立，每个碰到共享 crate 的 PR 都要多打两次 APK、起两台模拟器，而设备套件现在的间歇失败会让无关的 PR 一起变红，把 develop 上的噪音搬到每一个 PR 上。
- **提高 APK 体积上限**：拼音版只超了约 0.6 MB，改一个数字今晚就能绿。没有采用：维护者明确保留 135,000,000 字节（见[五笔码表按需下载](../feature/2026-10-28-android-on-demand-wubi.md)），上限挡的是整组按需资源回到包里；悄悄抬上限只会让下一次越线更晚被发现。预警线提前约 3 MB 提示，留出把数据改成资源包的时间。
- **在 PR 上把整个 rpm 打出来**：和发版完全一致，连 `check-rpm-install.sh` 的安装验证都能跑。没有采用：那是打包，要编桌面二进制、对六个版本各跑一次 cpack，违反「PR 上不打包」，也比只跑 ctest 慢得多。这次漏网的是测试，不是包。
- **把 Fedora 开发包装进 Debian 门禁镜像，或给 `build-container.sh` 加一个 Fedora 变体**：不碰发版脚本。没有采用：失败来自 Fedora 自己的 Pango、字体和编译器，Debian 上装不出来；另起一份 Fedora 配置要照抄 `package-container.sh` 的 Release 选项，和发版那条迟早漂移。
- **跟踪单按日期每天开一张**：实现最简单。没有采用：连红的那几天会摊成几张单，看不出起点，转绿时也不知道该关哪一张。

## Consequences

- **收益**：develop 和 nightly 的每一次红都会出现在一张指派给维护者的 issue 里，带着第一次变红的提交；转绿自动关闭，开着的 `ci-red` 单就是当前的红灯清单。瘦包在越过上限之前约 3 MB 处就有 issue 提示。只在 Fedora 上失败的 Linux 测试在 PR 上就能看到。
- **代价**：设备套件修好之前，`Native Platform CI` 的单会频繁开关、评论很多；这正是要让人看见的噪音，间歇失败的重试和隔离另做。PR 上多一个 Linux 作业：Fedora 镜像在 Depot runner 没有缓存时要现装，Release 构建也比 Debug 慢，但只在 Linux 或共享路径改动时运行，也不是必需检查，不挡合并。
- **只能合并后验证**：`workflow_run` 只按默认分支上的 workflow 文件触发，nightly 的 `android-size` 也只在 develop 上维护 issue，所以真实的开单、评论和关单要合进 develop 后由下一次运行验证。Fedora 作业在本 PR 自己的 CI 上就会跑（改了 `ci-platforms.yml`，Linux 作业都会被点亮）；develop 目前的 `linux-candidate-native-canvas-chrome` 失败修好之前，它在本 PR 上是红的，说明它确实拦得住这类失败。
- **仍然没有覆盖**：iOS、macOS 的推送 CI（`ci-ios.yml`、`ci-macos.yml`、`ci-macos-package.yml`）不在监听列表里。要加只需把 workflow 名加进 `workflows:`。

## Verification

- `python3 scripts/test-ci-issue-tracker.py`：10 项，用假的 GitHub 覆盖首红开单（只列失败和超时的 job，取消的运行不打断连续失败的计数）、追加评论与重复事件、重跑 attempt、乱序事件、成功关单与重新开单、PR 和其他分支的运行、各 workflow 各一张单、没有失败 job 的运行，以及体积单的开、改、缺报告不关和全部回落关闭。
- 用假的 `gh` 走真实的 `GitHub` 类跑两条子命令，核对调用参数和渲染出的正文。
- 从 `build-apk.sh` 截出体积那一段，用 128,000,000、133,000,000 和 135,623,964 字节的假 APK 分别跑出通过、警告加注解、超过上限失败，三种情况都写了摘要和合法的 JSON 报告；非瘦包写出 null 阈值的报告；预警线高于上限或不是数字时失败；`LC_ALL=C` 下照常运行。
- 用假的 `docker` 跑 `package-container.sh`：`MSIME_PACKAGE_TESTS_ONLY=1` 传进容器，版本缺省为 full、桌面缺省为 0；不含 full 的版本列表和非法取值都以 2 退出。`scripts/test-linux-package-cache.py`、`scripts/test-linux-distro-packaging.py` 通过。
- `actionlint`（`SHELLCHECK_OPTS=--severity=warning`）、`bash -n`、`pnpm run verify-notes`、`git diff --check` 通过。本机没有跑 Fedora 容器构建，也没有构建 APK。

## Related notes audit

- [PR 上只编译和测试，打包挪到合进 develop 之后](2026-10-09-no-packaging-on-pull-requests.md)：部分重叠，互链。它说的「develop 上失败没有通知」由本篇的跟踪单补上，那句已就地改为指向本篇；PR 上不打包的决定不变，Fedora 作业只跑测试。
- [六个平台共用发布构建链的 nightly 产物](2026-10-14-six-platform-nightly-builds.md)、[nightly 按平台更新并保留失败平台的上次成功包](2026-10-16-nightly-partial-platform-updates.md)：本篇在同一个 nightly 里加一个只读报告、写 issue 的作业，不改汇总发布，失败平台照样让原构建 job 保持失败。决定不变，无需改动。
- [五笔码表按需下载](../feature/2026-10-28-android-on-demand-wubi.md)：同样守 135,000,000 字节的上限；本篇加预警线，不改上限。决定不变。
- proposed 与 rejected 里没有同类方案。
