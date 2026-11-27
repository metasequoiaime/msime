# Agent Note: 只保留 main，发版改为推 tag 触发

Status: implemented

## Problem

仓库原来是两条长期分支：`develop` 是集成分支，所有 PR 发往它；`main` 是发布分支，只经 `scripts/release-pr.sh` 从 `develop` 切出的 `release/<版本>` 分支开 PR、用 merge commit 合入，`branch-guard.yml` 拒绝从别的 head 向 `main` 开 PR。发版是在某个分支上手动触发 `release-<平台>.yml`，工作流按触发分支给 tag 加后缀：`main` 不加、是正式版，`develop` 加 `-beta`，其他分支加 `-alpha`，tag 由工作流自己建。

这套流程的代价都落在发版那一步上：

- 每次发版要走两个 PR（版本号 PR 进 `develop`、release PR 进 `main`），release PR 必须用 merge commit，网页上点错成 squash 后 `main` 不再是 `develop` 的祖先，下一次发版只能靠 `release-pr.sh` 的「保留 develop 内容」分支兜底（2026-10-08 linux-0.11.0 #5879）。`release-pr.sh` 还为此长出了分叉核对、正文预算（[发布 PR 正文使用有界提交摘要](2026-10-14-release-pr-body-budget.md)）等专门逻辑。
- 发什么由「在哪个分支上点运行」决定，正式版和测试版的区别藏在触发分支里，tag 是工作流事后建的，事前没有一个可审阅、可引用的发版动作。
- 每个 CI workflow 都要分别处理 `develop` 和 `main`（触发分支、发往哪个分支的 PR 跑多少、nightly 只在 `develop` 上更新 appcast 和体积单），两套规则一直在漂移。

维护者（houko）决定：所有 PR 合到 `develop`，再整体合到 `main`，然后删掉 `develop`，以后只要 `main`；nightly 默认用 `main`；发版改为打 tag 之后自动触发。

## Decision

- **只有 `main`。** `main` 是默认分支，所有 PR 发往它。不再有 `release/*` 分支、`develop` 到 `main` 的同步 PR，也不再限制哪个 head 可以发往 `main`。各平台版本号（`platforms/<os>/version.txt` 及随它一起改的文件，macOS 的 `build-number.txt`）和别的改动一样经普通 PR squash 合入 `main`。
- **发版就是推 tag。** 版本号 PR 合入后，在 `main` 上包含它的提交打 `<前缀>X.Y.Z` 并推送，对应的 Release 工作流由 tag 触发。前缀沿用已有的（`android-v`、`ios-v`、`macos-v`、`linux-v`、`windows-v`、`windows7-v`、`harmony-v`，以及引擎、网页引擎、SDK、打包工具各自 workflow 里的前缀）。工作流构建前核对 tag 里的 `X.Y.Z` 等于被打 tag 的提交里的版本号、那个提交是 `origin/main` 的祖先，任一不符就失败、什么都不发；GitHub Release 发在推上来的 tag 上，工作流不再自己建 tag。
- **测试版是 `-beta.N`。** `<前缀>X.Y.Z-beta.N`（`N` 从 1 起，为兼容也接受旧的 `-beta`）发成 prerelease，应用内测试版通道（桌面三平台的 `update_check.rs`、Android 的 `UpdateApi.java`、macOS 的 Sparkle 测试版 appcast）把它当作与原来的 `-beta` 相同的测试版。Google Play、Homebrew cask、Linux 发行版包定义、稳定版 appcast、网页引擎的 npm 包、crates.io 等原来只在 `main` 上正式版之后发生的事，现在只在不带后缀的正式版 tag 上发生。`-alpha` 不再存在，因为不再有别的分支发布。
- **手动触发保留。** `workflow_dispatch` 在不是 tag 的 ref 上只构建产物、不发布；ref 是已有 tag 时与推送这个 tag 相同，用于重跑一次失败的发布或传非默认的输入。
- **nightly 和 CI 改用 `main`。** 定时运行本来就在默认分支上跑；原来写死 `refs/heads/develop` 的条件（nightly 的 APK 体积单、macOS 每日构建的 appcast 通道等）改为 `refs/heads/main`，各 CI workflow 的触发分支只列 `main`，原来按「发往 `develop` 还是 `main`」区分的行为收拢为发往 `main` 的那一种（偏重的 macOS、iOS 套件改由定时运行、手动触发和合并队列承担，CodeQL 改在发版 tag 上完整扫描）。
- **文档和代理规则随之改写。** `AGENTS.md`「发版」、`docs/open-source-release.md`、`release-platform` skill 改为「PR → main；版本号 PR → 合并 → 推 tag → 发布工作流；测试版是 main 上的 `-beta.N` tag」。原来针对 `develop` 的规则（先修红、连着合 PR 时跟上新的集成分支、合并后不再推、从 `origin/develop` 新切分支等）保留意图和事故记录，对象改为 `main`；`blob/develop`、`tree/develop`、`raw.githubusercontent.com/.../develop/` 链接改指 `main`。事故记录里「进了 develop」这类描述的是当时的事实，原样保留。

## Alternatives considered

- **保留 `develop` 作集成分支，只把发版改成推 tag。** 最强的理由是隔离：自动合并的改动先在 `develop` 上积累，`main` 只在发版时前进，`main` 的每个提交都是发过或准备发的状态，`raw.githubusercontent.com/.../main/` 上的文件也只在发版时变化，比直接读一个自动合并的分支更稳；已发出的客户端引用 `develop` 的地址也不会断。没有采用：隔离的代价就是现在要去掉的那部分——每次发版的同步 PR、merge commit 的约束、两套 CI 规则；而 tag 本身已经能精确指定发出去的是哪个提交，不需要一条分支来代表「已发布」。运行时读分支文件的信任问题本来就不靠分支隔离解决，`AGENTS.md` 早已要求客户端把能被分支内容影响的东西在编译期钉死。
- **保留按分支加后缀，只是不再要 release 分支（在 `main` 上触发就是正式版，测试版另设一条 `beta` 分支）。** 改动最小，现有工作流的后缀逻辑几乎不用动。没有采用：又回到「发什么由在哪条分支上点运行决定」，测试版需要一条要同步的长期分支，正是这次要去掉的东西。
- **测试版沿用不带编号的 `-beta`。** 客户端解析一字不改。没有采用：tag 现在由人推、指向固定提交，同一个版本号的第二个测试版没法再用同名 tag；带编号的 `-beta.N` 也符合 SemVer 的预发布排序，旧的 `-beta` 仍然接受。

## Consequences

- **收益**：发版只剩一个版本号 PR 和一次 `git push` tag，发的是哪个提交、正式版还是测试版都写在 tag 上，可以事前审阅、事后引用；不再有 merge commit 合错、`main` 与 `develop` 分叉的问题，CI 只需要维护一套分支规则。
- **已发出的客户端引用 `develop` 的地址**：桌面三平台已发出的版本读语法模型目录 `https://raw.githubusercontent.com/metasequoiaime/msime/develop/resources/grammar-models.catalog.json`（`crates/client-providers/src/resource_packs.rs` 的 `GRAMMAR_CATALOG_URL`），设置页和关于页里的许可证等链接也指向 `blob/develop/...`。新版本改指 `main`，但 `develop` 删掉后，还没升级的客户端读目录会得到 404，语法模型的「检查更新」和下载在那些版本上失败（已装好的模型不受影响），许可证链接打不开。要不要在删除 `develop` 之外另留一个只含这几个文件的 `develop` 分支、或者接受旧版本这部分失效，由维护者决定；`PRIVACY.md` 已写明较早的版本读的是 `develop` 上的同一个文件。
- **`main` 是自动合并的**：所有 PR 直接进 `main`，`main` 不再只在发版时前进。运行时从 `main` 拉取的文件仍不能当信任根，这条约束不变。
- **推 tag 的运行没有输入**：iOS 的 TestFlight 上传和送外部组、HarmonyOS 的签名和华为上传都在推 tag 之后马上开始，来不及先取消再手动触发，所以这三项改由推 tag 前设的仓库变量决定：`IOS_TAG_TESTFLIGHT_EXTERNAL=false` 不送外部组，`HARMONY_TAG_APPGALLERY` 选华为上传模式，`ANDROID_TAG_PLAY_TRACK` 选 Play 轨道（第一次上架时 `none`）；没设时与手动触发的默认值相同，手动触发时输入总有值，不读变量。其余非默认输入（例如 Windows 发布全部六个版本）只在对已有 tag 手动触发时传。
- **一次只推一个 tag**：一次推上去超过三个 tag 时 GitHub 不为它们产生 push 事件，发布工作流一个都不会启动，也不报错。`release-pr.sh --tag` 一个 tag 推一次，`scripts/test-release-pr-tag.py` 用本地远端的 pre-receive 钩子核对每次推送只带一个 tag。
- **发布不移动 tag**：`publish-release.sh` 原来在覆盖已有发布时用 `RELEASE_PUBLISH_TOKEN`（个人访问令牌）把 tag 强制指到这次构建的提交。tag 由人推之后这一步有害：附注 tag 的 ref 指向 tag 对象而不是提交，PATCH 会真的改动 ref、把它变成轻量 tag，而个人访问令牌改 ref 会产生新的 tag push 事件，再触发一整次发布（Play、Homebrew、发行版包、appcast 全部重来）。现在它只核对 tag 指向的提交等于要发布的提交，不等就失败，从不写 ref。
- **测试版 tag 的过渡**：已经装在用户手上的 Windows、Linux、HarmonyOS 客户端的 `is_beta_tag` 只认不带编号的 `-beta`，新的解析只对之后的构建生效。在那个平台发出第一个包含新解析的正式版之前，测试版打不带编号的 `-beta`（`release-pr.sh --tag --beta bare`），之后再用 `-beta.N`。Android 的 `UpdateApi.pick` 只比 `X.Y.Z`，同一版本号上的第二个测试版和随后的正式版都不会提供给装了第一个测试版的人（以前同一版本号只有一个 `-beta`，重跑原地覆盖，所以没有暴露）；Android 每个 `X.Y.Z` 只打一个测试版 tag，`release-pr.sh --tag` 拒绝第二个。要让 Android 也按运行 ID 区分同一版本号的构建，需要另改 `UpdateApi`，不在本次范围内。
- **引擎和 C 接口分 tag 发 crates.io**：引擎 tag 只发 msime-path-trust 和 msime-engine，C 接口留给自己的 `msime-engine-capi-v` tag；否则引擎 tag 把 C 接口带上 crates.io 之后，C 接口的 tag 找不到要发的东西而失败，先推 C 接口的 tag 又因为依赖还没发而失败。一起改版本号时先推引擎的 tag，等那次运行成功再推 C 接口的；C 接口这一版已经在 crates.io 上且一致时，它的 tag 不上传、照常成功。
- **签名和外部平台的分支白名单要跟着改**：Windows 专用签名 runner 分组原来只授权 `release-windows.yml@refs/heads/main` 和 `@refs/heads/develop`，tag 触发时 workflow 引用变成 `@refs/tags/windows-v<版本>`，组织设置要相应修改；SignPath、crates.io Trusted Publishing 等按 workflow 和 ref 授信的外部配置也要逐一核对。这些在仓库外，不随本次改动生效。
- **事故记录保留原样**：规则文字里提到的 `develop` 事故（2026-10-25 到 10-27 nightly 连红、#10318 与 #10322 的合并顺序、#9106 合并后推送丢失等）描述的是当时的集成分支，没有改写成 `main`。

## Verification

- 文档改动：`git grep -n -w develop` 核对 `AGENTS.md`、`.github/CONTRIBUTING.md`、`docs/`、各 README 和 `.agents/skills/` 里剩下的 `develop` 只是事故记录、指向别的仓库（`MSIME-Windows`、`MSIME-Apple`、`msime-windows` 的默认分支）、`nix develop` 或固定提交的对照记录。
- `pnpm run verify-notes`、`git diff --check`。
- 工作流、客户端解析和 nightly 的改动由同一批提交里的对应检查验证；推 tag 触发、tag 与版本号核对和发布到 tag 上要合进 `main` 后第一次发版时实际验证。

## Related notes audit

- [发布 PR 正文使用有界提交摘要](2026-10-14-release-pr-body-budget.md)：部分被取代。release PR 不再存在，但同样的字节预算仍用在版本号 PR 的正文上（`release-pr.sh` 生成、`release-pr-body.py` 的 `BODY_BYTE_BUDGET` 校验，列的是这个平台上一个 tag 以来的提交）；该篇记录的是当时的决定，不改。
- [develop 红灯自动开单、APK 体积预警和 PR 上的 Fedora ctest](2026-10-28-ci-red-tracking.md)、[推送前确认 PR 还开着，连着合并时逐个跟上 develop](2026-10-30-push-and-merge-sequencing.md)、[一个 issue 一个 PR](2026-10-11-one-issue-one-pr.md)：决定不变，规则对象从 `develop` 改为 `main`，`AGENTS.md` 已就地改写；笔记本身记录当时的分支，不改。
- [更新通道与后台检查下沉到共享 Rust](../architecture/2026-10-25-update-channels-core.md)、[Windows 的更新通道与常驻进程后台提醒](../feature/2026-10-25-windows-update-channels.md)：测试版通道的语义不变，`-beta.N` 与原来的 `-beta` 同样归为测试版；按分支加 `-alpha` 的部分不再适用。
- [Windows 发布在 CI 里经 SignPath 签名](2026-10-08-windows-signpath-ci-signing.md)、[Windows 手动发布自动完成 SimplySign 签名](2026-11-17-windows-simplysign-release.md)：签名只在发版 tag 上发生，原来按 `main` / `develop` 分支放行的部分改为按 tag，runner 分组的授权要由维护者在组织设置里改。
- [语法模型保留本地快照](../feature/2026-10-26-grammar-model-local-snapshot.md)、[语法模型目录读不到时退回锁文件](../bug-fix/2026-11-03-grammar-catalog-lock-fallback.md)：目录地址从 `develop` 改为 `main`，信任根仍是编译期钉死的上游 Release；旧版本读 `develop` 的后果见上文 Consequences。
- proposed 与 rejected 里没有同类方案。
