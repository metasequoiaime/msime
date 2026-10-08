---
name: release-platform
description: Release one 水杉输入法 platform (Android, iOS, macOS, Linux, Windows, HarmonyOS) or the web engine end to end — pick the version number, bump it on develop, cut the release branch to main, merge it the right way, dispatch the release workflow and check what was published. Use whenever the user says 发版 / 发布 / 出一个新版本 / 下一个版本是多少, or asks to ship a beta.
---

# 发布一个平台

规则本身在 [AGENTS.md](../../../AGENTS.md)「发版」和 [docs/open-source-release.md](../../../docs/open-source-release.md)，这里不重复，只记录**照着做就能发出去的步骤**和每一步上踩过的坑。全程五步：定版本号 → 版本号 PR 进 develop → release PR 进 main → 触发发布工作流 → 核对发布。

六个平台各自独立发版，tag 带平台前缀（`android-v0.3.0`、`linux-v0.11.0`），一个平台发版不牵动其余五个。但 release PR 是把**整个 develop** 带到 main，不是只带一个平台的改动。

## 动手前

按全局规范写出 `影响:`：正式版一旦发出，应用内更新会推给这个平台的所有用户，基本收不回来，最坏情况只能再发一个补丁版。用户说了「发吧」就是授权走完全程，包括合并两个 PR 和触发工作流，中途不用再问；但遇到下面「停下来问」的情况要停。

```sh
git fetch -q origin --tags
git tag --sort=-creatordate | grep -E '^<os>-v' | head -5          # 上一个版本
git show origin/develop:platforms/<os>/version.txt                  # develop 上现在的版本号
gh run list --workflow release-<os>.yml --limit 5                   # 有没有别人正在发同一个平台
gh pr list --base main --state open                                 # 同一时间只能有一个 release PR
```

## 1. 定版本号

没有成文的规则，按历史惯例：

- **次版本号（0.2.x → 0.3.0）**：上个 tag 以来有用户看得见的新功能。0.1.3 → 0.2.0 是界面重设计，0.2.2 → 0.3.0 是浮动键盘、计算器、文本编辑面板这一批 feat。
- **补丁号（0.2.1 → 0.2.2）**：只有修复。

```sh
git log --format='%s' <os>-v<上一版>..origin/develop -- platforms/<os> shared crates/client-core crates/host-api \
  | sed -E 's/ \(#[0-9]+\)$//' | awk '{split($1,a,/[(:!]/); c[a[1]]++} END{for(k in c) print c[k], k}' | sort -rn
git log --format='%h %s' <os>-v<上一版>..origin/develop -- platforms/<os> | grep -E '^[0-9a-f]+ feat'
```

给用户一个推荐和理由，不要列选项。

## 2. 版本号 PR 进 develop

从 `origin/develop` 开 worktree，分支 `chore/<os>-<版本号>-version`，提交 `chore(release): set <Platform> version to X.Y.Z`。

各平台要改的文件：

| 平台 | 文件 |
| --- | --- |
| Android | 只有 `platforms/android/version.txt`。versionCode 由 `gradle-app/app/build.gradle.kts` 按 `major*1e6 + minor*1e3 + patch` 算，不用手改 |
| Linux、Windows | 只有 `platforms/<os>/version.txt` |
| iOS | `version.txt`、`project.yml` 的 `MARKETING_VERSION`、`MSIMEClient.xcodeproj/project.pbxproj` |
| macOS | `version.txt`、`build-number.txt`、`Info.plist.in`、`src/core/SupportWindowController.mm` 里的兜底版本字符串 |
| HarmonyOS | `version.txt`、`AppScope/app.json5` 的 `versionName` **和** `versionCode`（同样按 `major*1e6 + minor*1e3 + patch`，0.2.0 是 2000），versionCode 不递增系统拒绝覆盖安装 |

iOS、macOS、HarmonyOS 改完用 `git grep -nF '<旧版本号>' -- platforms/<os> apps/desktop` 再扫一遍，`47b8e97e1`（0.52.0 一次改了全部平台）是完整的参照。

**develop 本身经常是红的，版本号 PR 要准备好顺手修它。** develop 合并很频繁，两个各自通过 CI 的 PR 合在一起就可能编不过。2026-10-08 发 Android 0.3.0 时一个小时里遇到两次：

- pre-push 门禁拒绝推送：`msime-engine` 的测试编不过，一个 PR 给 `NineKeySession::new` 加了参数，另一个 PR 的新测试还按旧签名调用。
- PR 的 `Android host` 失败：CI 编的是「PR 分支 + 当时最新的 develop」，而 develop 在等 CI 的时候又合进了一个漏了 import 的重构。

处理方式：先 `git fetch origin develop` 看是不是已经有人修了（第一处在我修完后十几分钟被别人独立修掉，rebase 时 git 自动丢掉了重复的提交）；没人修就在同一个 PR 里修，提交分开写，PR 正文说明，不要 `--no-verify`。门禁日志很长，失败项用 `grep -nE 'FAIL|error'` 找。`Android host` 失败在本地复现用 CI 同款命令：

```sh
rbuild env ANDROID_HOME=/Users/xiaomo/Library/Android/sdk apps/desktop/src-tauri/gen/android/gradlew \
  --project-dir platforms/android/gradle-app --console=plain \
  compileFullReleaseJavaWithJavac compilePinyinReleaseJavaWithJavac compileWubiReleaseJavaWithJavac \
  compileJapaneseReleaseJavaWithJavac compileVietnameseReleaseJavaWithJavac compileTibetanReleaseJavaWithJavac
```

开 PR（英文，`--body-file`），等必过检查绿了再合并。进 develop 的 PR 用 squash 没问题：

```sh
gh pr checks <n> --required --watch --interval 30     # 常常超过 10 分钟，放后台跑
gh pr merge <n> --squash --delete-branch
```

`--delete-branch` 会把这个分支的本地 worktree 一起删掉，合并前不要待在那个目录里，后面的步骤也别指望它还在。

必过检查由仓库规则集 `main and develop protection` 定义（2026-10 时是 macOS 15 arm64、macOS 15 x86_64、iOS Simulator、quality / Workflow validation、quality / Dependency review、Android host），以 `gh api repos/metasequoiaime/msime/rulesets` 为准。

## 3. release PR 进 main

**用 `scripts/release-pr.sh`，不要手切分支。** 它从 `origin/develop` 切出 release 分支、处理 main 与 develop 的分叉、生成带版本号对照表和提交列表的 PR 正文，全程不动当前工作区。

```sh
git fetch -q origin
git worktree add -q --detach ~/worktrees/release-<os>-X.Y.Z origin/develop
cd ~/worktrees/release-<os>-X.Y.Z
bash scripts/release-pr.sh --version <os>-X.Y.Z --dry-run   # 先看分支名、标题和合并方式
bash scripts/release-pr.sh --version <os>-X.Y.Z
```

**要在一个停在 `origin/develop` 的干净 worktree 里跑。** 脚本自己不碰工作区，但它最后的 `git push` 会触发 pre-push 门禁，而门禁检查的是**当前工作区的文件**，不是要推的提交。在主仓库里跑，主仓库停在哪个旧的、坏掉的 develop 上，门禁就按那个失败（2026-10-08 就是这样被拦了一次）。发完把这个临时 worktree 删掉。

- `--version` 只决定分支名和标题。单平台发版写 `<os>-X.Y.Z`，得到 `release/<os>-X.Y.Z`；不带时取 macOS 的版本号，那是全平台一起发时用的。
- main 被 squash 过（不在 develop 的祖先里）时，脚本会核对 main 的内容是否与 develop 历史上某个提交逐字节相同，相同才生成「保留 develop 内容」的合并提交；不同说明 main 上有 develop 没有的 hotfix，脚本退出并列出冲突文件，**这时停下来问用户**。
- 已经有打开的 release PR 时脚本拒绝运行，先处理那个。

合并前确认 PR 正文里「版本号」表格中目标平台 main 与 develop 两列不同，且 develop 列是要发的版本。

**合并必须用 merge commit：**

```sh
gh pr checks <n> --required --watch --interval 30     # macOS 15 x86_64 最慢，2026-10-08 这一项跑了 23 分钟
gh pr merge <n> --merge
```

仓库三种合并方式都允许，网页上点错就是 squash。2026-10-08 的 linux-0.11.0（#5879）正文写着「请用 merge commit 合并」，结果还是被 squash 了，下一次发版的 release 分支只能靠脚本的「保留 develop」分支兜底。合并后核对两件事：`git log -1 --format=%P origin/main` 有两个父提交；`git diff --quiet origin/main <develop 的那个提交> && echo same` 输出 same，也就是 main 的内容和切分支时的 develop 逐字节一致。

## 4. 触发发布工作流

```sh
gh workflow run release-<os>.yml --ref main
gh run list --workflow release-<os>.yml --limit 1        # 拿 run id
gh run watch <id> --exit-status --interval 30        # Android 六个版本并行，约 10 分钟
```

- 版本号默认读 `platforms/<os>/version.txt`，publish 默认开，一般不用传 `-f`。
- tag 后缀由触发分支决定：`main` 无后缀、正式版；`develop` 加 `-beta`，`release/*` 等其他分支加 `-alpha`，带后缀的一律是 prerelease，应用内更新检查不会提供。**用户要 beta 就 `--ref develop`**，不需要改版本号也不需要 release PR。
- Android 按 `editions.json` 打六个版本（full、wubi、pinyin、japanese、vietnamese、tibetan），任何一个失败整个发布不会发出去（`fail-fast: true`）。签名密钥只在 GitHub 托管 runner 上解码，不要把这个 job 挪到自托管或 Depot。
- iOS 默认还会上传 TestFlight 并提交外部测试审核（`testflight`、`testflight_external` 两个输入），不想送审就 `-f testflight_external=false`。
- 网页引擎（`release-web-engine.yml`）不同：版本号不读 `version.txt`，publish 默认关，打开时会发 npm 的 `latest`，只能从 main 跑。

## 5. 核对发布

```sh
gh release view <os>-vX.Y.Z --json tagName,isPrerelease,targetCommitish,assets --jq '.tagName, .isPrerelease, .targetCommitish, (.assets[]|"\(.name) \(.size)")'
```

- 正式版 `isPrerelease` 必须是 false，否则应用内更新拿不到。
- 附件齐全：Android 是六个 APK（`msime-android.apk`、`msime-android-<edition>.apk`）各带一个 `.sha256`。有两两大小完全相同的 APK（full 与 pinyin、tibetan 与 vietnamese）是正常的，0.2.2 起就这样，哈希不同就行。
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

  android-v0.2.0 到 android-v0.3.0、linux-v0.11.0 这几次全是 `HTTP Error 403: Forbidden`：api.everyapi.ai 前面的 Cloudflare 按 User-Agent 拒绝 urllib 默认的 `Python-urllib/3.x`（error 1010），跟 token 和额度都无关。脚本现在会发明确的 User-Agent。如果又看到 403，先用 curl 带一个无效密钥打同一个地址：返回 401 说明网关是通的，问题在密钥；返回 403 和 `error code: 1010` 说明又被网关拦了。退回兜底版本的发布说明要告诉用户，可以按「新增 / 修复 / 改进」重写一份，用 `gh release edit <tag> --notes-file` 替换。改公开的发布说明前先问用户。

## 收尾

- 两个 PR 的 worktree 和本地分支当场删掉（全局规范）。
- 向用户报告：版本号、两个 PR 链接、release 链接、实际验证到哪一层（CI、模拟器、真机）。没在真机上装过就明说。
