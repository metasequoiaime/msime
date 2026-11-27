---
name: release-platform
description: Release one 水杉输入法 platform (Android, iOS, macOS, Linux, Windows, HarmonyOS) or the web engine end to end — pick the version number, bump it on main through a PR, push the release tag that triggers the release workflow and check what was published. Use whenever the user says 发版 / 发布 / 出一个新版本 / 下一个版本是多少, or asks to ship a beta.
---

# 发布一个平台

规则本身在 [AGENTS.md](../../../AGENTS.md)「发版」和 [docs/open-source-release.md](../../../docs/open-source-release.md)，这里不重复，只记录**照着做就能发出去的步骤**和每一步上踩过的坑。全程四步：定版本号 → 版本号 PR 进 main → 推 tag 触发发布工作流 → 核对发布。

六个平台各自独立发版，tag 带平台前缀（`android-v0.3.0`、`linux-v0.11.0`），一个平台发版不牵动其余五个。只有一个分支 `main`，tag 打在 main 上的哪个提交，发出去的就是那个提交上的**全部**代码，不是只带一个平台的改动。

## 动手前

按全局规范写出 `影响:`：正式版一旦发出，应用内更新会推给这个平台的所有用户，基本收不回来，最坏情况只能再发一个补丁版。用户说了「发吧」就是授权走完全程，包括合并版本号 PR 和推 tag，中途不用再问；但遇到下面「停下来问」的情况要停。

```sh
git fetch -q origin --tags
git tag --sort=-creatordate | grep -E '^<os>-v' | head -5          # 上一个版本
git show origin/main:platforms/<os>/version.txt                     # main 上现在的版本号
gh run list --workflow release-<os>.yml --limit 5                   # 有没有别人正在发同一个平台
gh pr list --base main --state open --search 'chore(release) in:title'   # 有没有还没合的版本号 PR
```

## 1. 定版本号

没有成文的规则，按历史惯例：

- **次版本号（0.2.x → 0.3.0）**：上个 tag 以来有用户看得见的新功能。0.1.3 → 0.2.0 是界面重设计，0.2.2 → 0.3.0 是浮动键盘、计算器、文本编辑面板这一批 feat。
- **补丁号（0.2.1 → 0.2.2）**：只有修复。

```sh
git log --format='%s' <os>-v<上一版>..origin/main -- platforms/<os> shared 'crates/client-*' crates/host-api \
  | sed -E 's/ \(#[0-9]+\)$//' | awk '{split($1,a,/[(:!]/); c[a[1]]++} END{for(k in c) print c[k], k}' | sort -rn
git log --format='%h %s' <os>-v<上一版>..origin/main -- platforms/<os> | grep -E '^[0-9a-f]+ feat'
```

给用户一个推荐和理由，不要列选项。

## 2. 版本号 PR 进 main

从 `origin/main` 开 worktree，分支 `chore/<os>-<版本号>-version`，提交 `chore(release): set <Platform> version to X.Y.Z`。

各平台要改的文件：

| 平台 | 文件 |
| --- | --- |
| Android | 只有 `platforms/android/version.txt`。versionCode 由 `gradle-app/app/build.gradle.kts` 按 `major*1e6 + minor*1e3 + patch` 算，不用手改 |
| Linux、Windows | 只有 `platforms/<os>/version.txt` |
| Windows 7 | 只有 `platforms/windows/version-win7.txt`（独立版本线，不动现代 Windows 的 `version.txt`）。发布用 `release-windows7.yml`，只发 full，tag 是 `windows7-vX.Y.Z` |
| iOS | `version.txt`、`project.yml` 的 `MARKETING_VERSION`、`MSIMEClient.xcodeproj/project.pbxproj` |
| macOS | 只有 `version.txt` 和 `build-number.txt`（新版本重置为 1）。`CFBundleShortVersionString`、`CFBundleVersion` 和关于页的兜底版本都由 CMake 从这两个文件生成，Sparkle 靠 `CFBundleVersion` 判断新旧，不要手写 |
| HarmonyOS | `version.txt`、`AppScope/app.json5` 的 `versionName` **和** `versionCode`（同样按 `major*1e6 + minor*1e3 + patch`，0.2.0 是 2000），versionCode 不递增系统拒绝覆盖安装 |

iOS、HarmonyOS 改完用 `git grep -nF '<旧版本号>' -- platforms/<os> apps/desktop` 再扫一遍，`47b8e97e1`（0.52.0 一次改了全部平台）是完整的参照。

**main 本身可能是红的，版本号 PR 要准备好顺手修它。** main 合并很频繁，两个各自通过 CI 的 PR 合在一起就可能编不过（下面两次发生时集成分支还叫 develop）。2026-10-08 发 Android 0.3.0 时一个小时里遇到两次：

- pre-push 门禁拒绝推送：`msime-engine` 的测试编不过，一个 PR 给 `NineKeySession::new` 加了参数，另一个 PR 的新测试还按旧签名调用。
- PR 的 `Android host` 失败：CI 编的是「PR 分支 + 当时最新的集成分支」，而它在等 CI 的时候又合进了一个漏了 import 的重构。

处理方式：先 `git fetch origin main` 看是不是已经有人修了（第一处在我修完后十几分钟被别人独立修掉，rebase 时 git 自动丢掉了重复的提交）；没人修就在同一个 PR 里修，提交分开写，PR 正文说明，不要 `--no-verify`。门禁日志很长，失败项用 `grep -nE 'FAIL|error'` 找。`Android host` 失败在本地复现用 CI 同款命令：

```sh
rbuild env ANDROID_HOME=/Users/xiaomo/Library/Android/sdk platforms/android/gradle-app/gradlew \
  --project-dir platforms/android/gradle-app --console=plain \
  compileFullReleaseJavaWithJavac compilePinyinReleaseJavaWithJavac compileWubiReleaseJavaWithJavac \
  compileJapaneseReleaseJavaWithJavac compileVietnameseReleaseJavaWithJavac compileTibetanReleaseJavaWithJavac
```

开 PR（英文，`--body-file`），等必过检查绿了再合并，和别的 PR 一样 squash：

```sh
gh pr checks <n> --required --watch --interval 30     # 常常超过 10 分钟，放后台跑
gh pr merge <n> --squash --delete-branch
```

`--delete-branch` 会把这个分支的本地 worktree 一起删掉，合并前不要待在那个目录里，后面的步骤也别指望它还在。合并后记下 squash 出来的提交号（`gh pr view <n> --json mergeCommit --jq .mergeCommit.oid`），tag 就打在它上面。

必过检查由仓库的分支规则集定义（2026-10 时是 macOS 15 arm64、macOS 15 x86_64、iOS Simulator、quality / Workflow validation、quality / Dependency review、Android host），以 `gh api repos/metasequoiaime/msime/rulesets` 为准。

## 3. 推 tag 触发发布工作流

版本号 PR 合入后，在它 squash 出来的那个提交上打 tag 并推送。不要用 `gh release create` 或网页建 tag，也不要在没合进 main 的提交上打：

```sh
git fetch -q origin --tags
sha=<版本号 PR 的合并提交>
git merge-base --is-ancestor "$sha" origin/main && echo on-main          # 必须输出 on-main
git show "$sha":platforms/<os>/version.txt                              # 必须等于要发的 X.Y.Z
git tag -a <os>-vX.Y.Z -m "<Platform> X.Y.Z" "$sha"                     # 测试版是 <os>-vX.Y.Z-beta.N
git push origin <os>-vX.Y.Z                                               # 一次只推一个 tag，见下
gh run list --workflow release-<os>.yml --limit 1        # 拿 run id，事件应当是 push
gh run watch <id> --exit-status --interval 30        # Android 六个版本并行，约 10 分钟
```

- 推 tag 一次推一个（`scripts/release-pr.sh --tag` 就是这样推的）。一次推上去超过三个 tag 时 GitHub 不为它们产生 push 事件，发布工作流一个都不会启动，也不报错；不要用 `git push --tags`，也不要把几个平台的 tag 写在同一条 `git push` 里。 Windows 正式签名跑在组织的 `windows-signing` runner 组上，它的 workflow 白名单只收具体的 ref、不认 `windows-v*` 通配，所以 `release-pr.sh --tag` 每推一个 `windows-v` tag 就把 `release-windows.yml@refs/tags/<tag>` 加进白名单（只留 main 和最近 5 个 tag），推 Windows tag 的人要有 `admin:org` 权限；手动推 Windows tag 时也要照做，否则签名 job 拿不到 runner。
- 工作流构建前先核对：tag 里的 `X.Y.Z` 等于这个提交里的版本号，这个提交是 `origin/main` 的祖先。任一不符就失败，什么都不发；这时删掉推错的 tag（`git push origin :refs/tags/<tag>`，本地 `git tag -d <tag>`），改对了再推。GitHub Release 发在推上来的这个 tag 上，工作流不再自己建 tag。
- 正式版的 tag 是 `<os>-vX.Y.Z`；测试版是 `<os>-vX.Y.Z-beta.N`（`N` 从 1 起，同一版本号每发一次加一；不带编号的 `-beta` 仍被接受，只为兼容旧 tag）。测试版一律是 prerelease，稳定版通道的应用内更新检查不会提供，桌面三平台的测试版、每日构建通道和 Android 的预览版通道提供。**用户要 beta 就打 `-beta.N`**，打在 main 上即可。过渡期例外：用户手上已经装着的 Windows、Linux、HarmonyOS 客户端是 `update_check.rs` 改成认 `-beta.N` 之前的构建，只认不带编号的 `-beta`，所以在那个平台发出第一个包含新解析的正式版之前，测试版打不带编号的 `<os>-vX.Y.Z-beta`（`scripts/release-pr.sh --tag --beta bare <os>`），之后再用 `-beta.N`。Android 的预览版通道只比 `X.Y.Z`（`UpdateApi.pick`），同一个版本号上的第二个测试版不会提供给已经装了第一个的人，正式版也不会，所以 Android 每个 `X.Y.Z` 只打一个测试版 tag，要再发测试版就先改版本号（`release-pr.sh --tag` 会拒绝）。包内版本号是 `X.Y.Z`，所以测试版的 `X.Y.Z` 必须是还没发过正式版的那一版：main 上的 `version.txt` 还等于刚发出的正式版时，先照第 2 步把修订号加一（macOS 0.54.0 之后的测试版就是这样发成 `macos-v0.54.1-beta` 的），再打 `-beta.1`。正式版之后不要再在同一个版本号上打测试版。
- 以前按分支加的 `-alpha` 已经没有了。只想构建、不发布时，在任意分支上手动触发：`gh workflow run release-<os>.yml --ref <分支>` 只出 workflow artifact，不建 release。对已有 tag 手动触发（`--ref <tag>`）与推送这个 tag 相同，用来重跑一次失败的发布。
- 推 tag 触发的运行没有输入。iOS 的 TestFlight 上传和送外部组、HarmonyOS 的签名和华为上传都在推 tag 之后马上开始，等它跑起来再取消、改成手动触发已经来不及，所以推 tag 时要用非默认值的，推之前先设仓库变量，这次运行结束后删掉：`gh variable set IOS_TAG_TESTFLIGHT_EXTERNAL --body false`（只传内部测试组、不送外部组）、`gh variable set HARMONY_TAG_APPGALLERY --body none`（也可以是 `upload`、`public-test`）、`gh variable set ANDROID_TAG_PLAY_TRACK --body none`（也可以是 `internal`、`alpha`、`beta`、`production`），删掉用 `gh variable delete <名字>`。对已有 tag 手动触发时直接 `-f testflight_external=false`、`-f appgallery=none`、`-f play_track=none`，不读这些变量。
- Android 按 `editions.json` 打六个版本（full、wubi、pinyin、japanese、vietnamese、tibetan），任何一个失败整个发布不会发出去（`fail-fast: true`）。签名密钥只在 GitHub 托管 runner 上解码，不要把这个 job 挪到自托管或 Depot。
- Android 和 macOS 先看这次带进来的词库锁有没有改五笔码表：`git diff <上一个同平台 tag> HEAD -- resources/desktop-dictionary.lock.json` 里 `msime-wubi.db` 那一项的 `sha256` 变了，就是改了。两端的全功能版都不随包带码表，升级后已装的 `wubi` 资源包对不上新锁、随之失效，五笔从键盘里消失，要等用户打开一次设置应用重新取得码表才回来（Android 上还要在弹窗里同意下载）（词库自动更新已下载过同一份码表时从本机复制，不联网）。这种版本要告诉维护者，并在发布说明里写一句「用五笔的朋友升级后请打开一次水杉输入法，按提示下载新的五笔码表」。来龙去脉见 `.agents/notes/implemented/feature/2026-10-28-android-on-demand-wubi.md`。
- iOS 默认还会上传 TestFlight 并提交外部测试审核（`testflight`、`testflight_external` 两个输入），推 tag 时不想送审就先设 `IOS_TAG_TESTFLIGHT_EXTERNAL=false`（见上），手动触发时 `-f testflight_external=false`。nightly 只在这个版本号已有 `ios-v` 标签时才送外部组，所以 main 改了 iOS 版本号就要尽快发 iOS，否则外部测试者一直停在旧版本号的构建上；2026-10 的 0.52.0 因为没跑 iOS 发布，nightly 已经把 0.52.0 送给外部测试者一周多，官网还显示 0.51.5。
- HarmonyOS 正式版 tag 在 GitHub 发布之后会自动上传华为测试专区并提交公开测试审核（`appgallery` 默认 `auto`）。CI 会同步 `platforms/harmony/appgallery-materials.json` 的新版本特性，每次发版前把它改成这一版的内容；应用图标华为不让 API 改（`app-file-info` 返回 204144641），换图标时用仓库里的 `app_icon_appgallery.png` 在控制台手动传，截图、审核账号、VIP账号、隐私与备案等只能由 AGC/secret 配置一次。日志里看 `package_id=`、`materials_status=synced`、`test_version_id=`、`review_status=submitted`；这一步失败时 GitHub release 已经发出，按 `platforms/harmony/README.md` 的恢复路径用 `source_release` 或 `test_package_id` 补传，不要重跑整个任务。推 tag 时不想送审就先设 `HARMONY_TAG_APPGALLERY=none`（见上），手动触发时 `-f appgallery=none`。测试版不进华为（versionCode 与随后的正式版相同）。
- Android 正式版 tag 还会在 full 那一格另打一份 Google Play 用的 AAB（artifact `google-play-msime-android-<版本>`），`play` job 在 GitHub 发布之后把它传到 Play：默认 `play_track=beta`（开放测试）、`play_status=draft`，传上去后要在 Play Console 里点发布。Play Console 只认手动上传的第一个包，第一次上架那次发版推 tag 前先设 `ANDROID_TAG_PLAY_TRACK=none`（见上），从 artifact 下载 AAB 手动上传。`GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` 没配时 `play` job 失败，GitHub 发布不受影响。测试版不进 Play（versionCode 与随后的正式版相同）。
- 网页引擎（`release-web-engine.yml`）不同：版本号不读 `version.txt`，tag 是 `web-engine-vX.Y.Z`，只有 main 上的正式版 tag 发 npm 的 `latest`；在别的 ref 上手动触发只构建不发布。

## 4. 核对发布

**发布页的标准，所有平台一样：** 版本发布页上只放用户该下载的安装包（macOS 的 pkg、APK、deb/rpm、安装程序等）和一份只列这些安装包的 `SHA256SUMS`，发布说明开头是从 `shared/contracts/editions.json` 生成的「下载哪个」指引。应用内更新包、调试符号、给发行版打包用的源码包这类用户不会手动下载的文件，放到各自的滚动 release（例如 macOS 的 `macos-appcast`、`macos-symbols`），不挂在版本发布页上，免得用户看着一长串附件不知道选哪个。macOS 从 0.54.1 起按这个标准发，维护者确认过这就是以后的样子；其他平台的发布页还没跟上的，核对时把多出来的附件当作要修的问题报告出来。

```sh
gh release view <os>-vX.Y.Z --json tagName,isPrerelease,targetCommitish,assets --jq '.tagName, .isPrerelease, .targetCommitish, (.assets[]|"\(.name) \(.size)")'
```

- 正式版 `isPrerelease` 必须是 false，否则应用内更新拿不到。
- 附件齐全：Android 是六个 APK（`msime-android.apk`、`msime-android-<edition>.apk`）、一份只列这六个 APK 的 `SHA256SUMS`，以及每个 APK 各一个 `.sha256`，发布说明开头是从版本表生成的下载指引。`.sha256` 是标准之外暂时保留的：0.2.0 到 0.4.2 的应用内更新只认 `<新版本的 tag>/<apk>.sha256`，挪到别的 release 就等于删掉，什么时候撤由维护者看读 `SHA256SUMS` 的安装够多了再定，核对时不要把它们当作多出来的附件报告（见 `.agents/notes/implemented/process/2026-10-20-android-release-page-sha256sums.md`）。有两两大小完全相同的 APK（full 与 pinyin、tibetan 与 vietnamese）是正常的，0.2.2 起就这样，哈希不同就行。
- macOS：`macos-vX.Y.Z` 上只有六个 pkg（`<dmg_prefix>-X.Y.Z-universal.pkg`，文件名不带 -beta 之类的后缀）和只列它们的 `SHA256SUMS`，发布说明开头是从版本表生成的下载指引（保留 `<!-- msime-download-guide -->` 标记，官网靠它跳过这一段）。0.56.x 及以前是 DMG，那些版本不重跑、不补 pkg。第一个 pkg 版本 0.57.0 必须发成正式版（不带后缀的 tag、不是 prerelease）：官网要看到第一个非预发布的 pkg 才切换安装说明。dSYM 在滚动的 `macos-symbols` release 上（`msime-macos-<发布名>-dsym.zip`）。
- Linux：`linux-vX.Y.Z` 上只有每个版本两个架构的 `.deb`、`.rpm` 与 `.tar.gz`，勾选了 `legacy` 时再加两个 `msime-linux-legacy_X.Y.Z_<架构>.deb`，以及只列这些安装包的 `SHA256SUMS`；发布说明开头是从版本表生成的下载指引。正式版另把三个源码包和 `msime-X.Y.Z-SHA256SUMS` 传到滚动的 `linux-sources` release，COPR、OBS、PPA 与 Gentoo 从那里取；测试版的源码包只在 workflow artifact 里。
- Windows：发布默认 `editions=full`，只有对 tag 手动触发时显式选择 `all` 才发布六个独立版本；以本次选择为验收依据，见[默认只签署完整版](../../notes/implemented/process/2026-11-17-windows-release-full-default.md)。`windows-vX.Y.Z` 上只放所选版本的安装程序（`<installer_base_name>_vX.Y.Z.exe`）、只列它们的 `SHA256SUMS`，以及每个安装程序各一个 `.exe.sha256`：full 共 3 个附件，all 共 13 个。发布说明开头的下载指引也只列所选版本。`.sha256` 是标准之外保留的：`render.py` 生成包管理器清单时拿它和 GitHub 摘要互相核对，设置页拿不到摘要时也让用户去下载它，核对时不要把它们当作多出来的附件报告。符号包在滚动的 `windows-symbols` release 上（`msime-windows-<版本 id>-<发布名>-symbols.zip`，本次 full 上传 1 个，all 上传 6 个），不在版本发布页上。默认发布仍保留全部文件签名和安装验收，共用缓存只减少相同文件的重复签名；26 是 Windows 0.5.0 full 的实测文件签名基线，不是固定用量或账户月累计。维护者用 SimplySign 签名后替换安装程序时，`SHA256SUMS` 里那一行要一起换（`platforms/windows/packaging/README.md`「发布步骤」）。
- macOS 正式版：`macos-appcast` release 上的 `appcast-<版本 id>.xml` 应当被这次发布更新，`<sparkle:version>` 是这次的构建号、`url` 指向同在 `macos-appcast` 上的 `*-input-method.zip`（设置应用的 appcast 指向 `*-settings.zip`），旧版本的更新包已被删掉。工作流日志里出现 `SPARKLE_ED_PRIVATE_KEY is not set` 说明这次没有 Sparkle 更新，已装的用户只能手动下载安装包。
- Android 下载 full 包核对包名、版本和签名证书。证书不是 `3a889e43…8e4132`（msime-release）的话，老用户没法覆盖升级，必须撤下：

  ```sh
  gh release download <os>-vX.Y.Z -R metasequoiaime/msime -p 'msime-android.apk' -D /tmp/relcheck
  BT=~/Library/Android/sdk/build-tools/36.0.0
  $BT/aapt2 dump badging /tmp/relcheck/msime-android.apk | head -1          # versionName X.Y.Z, versionCode 递增
  $BT/apksigner verify --print-certs /tmp/relcheck/msime-android.apk | grep SHA-256
  rm -rf /tmp/relcheck
  ```
- 发布说明由 `scripts/generate-release-notes.py` 生成：先用 EveryAPI 的模型写一份按「新增 / 修复 / 改进」分组的中文说明，失败时退回确定性版本，也就是把「最近 180 条」提交原样列出来，里面全是 `refactor(android): parse ... strictly` 这类用户看不懂的条目。看一眼正文开头就能分辨；要确认原因就查日志：

  ```sh
  gh run view <id> --log | grep 'EveryAPI 发布说明生成失败'
  ```

  android-v0.2.0 到 android-v0.3.0、linux-v0.11.0 这几次全是 `HTTP Error 403: Forbidden`：api.everyapi.ai 前面的 Cloudflare 按 User-Agent 拒绝 urllib 默认的 `Python-urllib/3.x`（error 1010），跟 token 和额度都无关。脚本现在会发明确的 User-Agent。如果又看到 403，先用 curl 带一个无效密钥打同一个地址：返回 401 说明网关是通的，问题在密钥；返回 403 和 `error code: 1010` 说明又被网关拦了。看到的是 `HTTP Error 400: Bad Request` 就是请求参数被模型拒绝：windows-v0.2.1 是请求带了 `temperature`，GPT-5 系列只接受默认值。脚本现在不带它，并把响应正文一起打进日志，原因直接看那一行。退回兜底版本的发布说明要告诉用户，可以照 `scripts/release-notes-example.md` 的格式重写一份，用 `gh release edit <tag> --notes-file` 替换。改公开的发布说明前先问用户。

## 收尾

- 版本号 PR 的 worktree 和本地分支当场删掉（全局规范）。
- 向用户报告：版本号、版本号 PR 链接、tag、release 链接、实际验证到哪一层（CI、模拟器、真机）。没在真机上装过就明说。
