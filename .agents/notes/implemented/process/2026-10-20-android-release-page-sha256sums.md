# Agent Note: Android 版本发布页加 SHA256SUMS 和下载指引，每个 APK 的 .sha256 暂时保留

Status: implemented

## Problem

维护者定下的发布页标准（`.agents/skills/release-platform/SKILL.md` 第 5 步，macOS 的做法见[macOS 版本发布页只放 DMG](2026-10-20-macos-release-page-only-dmgs.md)）是：版本发布页只放用户该下载的安装包和一份只列这些安装包的 `SHA256SUMS`，发布说明开头是从 `shared/contracts/editions.json` 生成的「下载哪个」指引，用户不手动下载的文件挪到别处。android-v0.4.2 上是六个 APK 各带一个 `.sha256`，共 12 个附件，没有 `SHA256SUMS`，发布说明也没说该下哪个。

`.sha256` 不能照 macOS 的办法挪到滚动 release 上。已安装的应用内更新（`platforms/android/java/app/msime/android/account/UpdateApi.java` 的 `update()`）用新版本自己的 tag 拼校验文件地址：`https://github.com/metasequoiaime/msime/releases/download/<新版本的 tag>/<apk>.sha256`，取不到就拒绝更新。逐个看过已发布 tag 里的这个文件：android-v0.2.0-beta 到 android-v0.3.1 只从 GitHub 取这个地址；android-v0.4.0 到 android-v0.4.2 先取 `https://dl.msime.app/gh/` 加同一个地址，失败再取 GitHub 原地址；android-v0.1.3 没有应用内更新。macOS 的 appcast 是一层可以改写的间接，Android 没有这层，地址里就是版本 tag，`android-checksums` 这类滚动 release 的地址根本不在这条路径上。国内镜像（msime-web 的 `workers/dl/index.mjs`）只原样转发 GitHub 上真实存在的资产，也接不住。下一个版本页上只要少了 `<apk>.sha256`，0.2.0 以来的每个安装每天的自动更新和手动检查都会失败（404 提示「这个版本没有本设备可用的安装包」），修好的更新器也送不到这些设备上。

## Decision

先做不弄坏任何东西的那一半：

- `release-android.yml` 的 publish 在 `dist/` 里写出只列 APK 的 `SHA256SUMS`（`sha256sum -- *.apk`，写完用 `sha256sum -c` 核对），APK 必须恰好每个版本一个，多一个少一个都不发。每个 APK 的 `<apk>.sha256` 照旧生成、照旧上传，内容与 `SHA256SUMS` 里那一行相同。上传的清单是 APK、`SHA256SUMS`、`<apk>.sha256`。
- 发布说明开头是 `edition_android.py download-guide` 生成的下载指引：一张「版本 | 安装包」表，名字和 APK 名都取自版本表，完整版（`msime-android.apk`）排第一、标为推荐，说明大多数人下载它、其他版本可以和完整版同时安装、`SHA256SUMS` 是什么、旁边的 `.apk.sha256` 是给旧版本应用内更新用的不用下载。带后缀的预发布另加一句：只在「预览版」更新通道提供。整段夹在 `<!-- msime-download-guide -->` 与 `<!-- /msime-download-guide -->` 之间，与 macOS 同一对标记；`generate-release-notes.py` 的更新内容接在后面。
- 应用内更新先读同一发布、同一来源上的 `SHA256SUMS`（`UpdateApi.parseSums`，按文件名精确匹配那一行，接受 `sha256sum` 的二进制模式 `*` 和 CRLF，同名两行摘要不同按没有处理）。取不到（0.4.2 及更早的发布没有它，404）或者里面没有这个 APK 时，退回同一来源上的 `<apk>.sha256`。`SHA256SUMS` 列了这个 APK 就以它为准，摘要对不上时不再拿 `<apk>.sha256` 放行。镜像与 GitHub 各自配对校验文件和 APK 的规则不变（[Android 更新按来源配对校验文件与 APK](../bug-fix/2026-10-12-android-update-source-pairing.md)），两份校验文件都只防传输损坏，挡住被替换的包的仍是安装前的包名、版本号和签名证书检查。

### 什么时候撤掉每个 APK 的 .sha256

由维护者决定：等足够多的安装已经换成读 `SHA256SUMS` 的更新器（第一个带这次改动的 Android 版本及以后）再撤。撤掉那一刻，还停在 0.2.0 到 0.4.2（以及那之后、这次改动之前的版本）的安装就再也收不到应用内更新，只能手动下载 APK 覆盖安装；包名和签名不变，数据保留。撤的时候改 `release-android.yml` 的 publish（不再生成和上传 `<apk>.sha256`）、`edition_android.py` 下载指引里关于 `.apk.sha256` 的那句、`scripts/test-android-release-page.py` 的资产清单和 SKILL.md 的核对条目；`UpdateApi` 里退回 `<apk>.sha256` 的分支留着，读旧发布用得到。

## Alternatives considered

- **照 macOS 的做法，把 `.sha256` 挪到滚动 release（例如 `android-checksums`），版本页只留 APK 和 `SHA256SUMS`** — 最强的理由是一步到位满足发布页标准，和 macOS 一模一样。否定：旧客户端按新版本的 tag 拼地址，滚动 release 的地址不在那条路径上，挪走等于删掉，0.2.0 以来的全部安装立刻收不到更新。
- **让 dl.msime.app 的 Worker 遇到 `android-v*` 下缺失的 `<apk>.sha256` 时，从同一 release 的 `SHA256SUMS` 截出那一行返回** — 最强的理由是版本页可以马上只放安装包，0.4.x 先走镜像，照样能更新。否定：0.2.x 和 0.3.x 不走镜像，照样坏；镜像不通时 0.4.x 退回 GitHub 也会坏；镜像从「原样转发、内容不变」变成「自己生成内容」，而它的缓存从不重新校验。改动还在另一个仓库，这个 workflow 没法与它一起验证。
- **新的更新器改成从 msime.app 的发行版列表（`/api/releases`）里拿 GitHub 记录的资产摘要，不读任何校验文件** — 最强的理由是少一次请求，摘要来自 GitHub 自己算的 `digest`。否定：`/api/releases` 的条目现在只有 tag、版本号、预发布标记和说明这类元数据，没有资产摘要，要在 msime-web 里扩接口；它又是带缓存的镜像，重跑覆盖同一版本时可能给出旧摘要。不管选哪种，旧客户端的约束都一样，撤掉 `.sha256` 的时机问题不会因此消失。
- **`SHA256SUMS` 没有列这个 APK 时直接拒绝，不退回 `<apk>.sha256`** — 最强的理由是校验来源只有一个，更简单。否定：退回不降低安全性（两者同一来源、只防传输损坏），而一份不完整的 `SHA256SUMS`（例如手工修补过的发布）不该让本来能用的 `<apk>.sha256` 失效。

## Consequences

- **收益**：版本发布页多了只列 APK 的 `SHA256SUMS`，发布说明第一段告诉人该下哪个。从这次改动所在的版本起，应用内更新不再依赖 `<apk>.sha256`，以后撤掉它们时不会弄坏这些安装。
- **代价与已知上限**：
  - 版本发布页暂时是 13 个附件（6 个 APK、`SHA256SUMS`、6 个 `.sha256`），还没达到「只放安装包和 `SHA256SUMS`」的标准，只能靠下载指引说明 `.sha256` 不用下载。
  - 新的更新器检查旧发布（没有 `SHA256SUMS`）时，每个来源多一次 404 请求。
  - 已发布的 android-v0.4.2 及更早的发布不在这里改：它们没有 `SHA256SUMS`，新的更新器读它们时走 `<apk>.sha256`。要给 0.4.2 补上，就是从那 6 个 `.sha256` 合成 `SHA256SUMS`，与 GitHub 给资产记录的 `digest` 核对后上传，再把下载指引加到发布说明前面，6 个 `.sha256` 原样保留；改公开发布前要先问维护者。
  - 官网下载页的更新记录（msime-web）读的是 `/api/releases`，那里已经把 HTML 注释剥掉，但下载指引本身仍会当成正文显示，与 macOS 一样要在 msime-web 里按那对标记跳过。

## Verification

- `python3 scripts/test-android-release-page.py`：把 workflow 里 publish 那一步的脚本原样取出来，在临时目录里配上仓库的版本表和 `edition_android.py`、记录参数的假 `generate-release-notes.py` 和 `publish-release.sh` 来跑。上传清单正好是六个 APK、`SHA256SUMS` 和六个 `.sha256`，`SHA256SUMS` 只列 APK、摘要正确，每个 `.sha256` 与它那一行相同；发布说明以下载指引开头、链接指向这个 tag，更新内容接在后面；beta 的说明写明是预览版；少一个 APK 或多一个 APK 时什么都不发布。下载指引另测完整版排第一、名字取自版本表、每个 Android 版本都有一行、缺完整版时拒绝生成。
- `UpdateApiSmoke`（`platforms/android/check-host.sh` 运行）：只有 `SHA256SUMS` 时核对通过且不读 `<apk>.sha256`；没有 `SHA256SUMS` 时每个来源都先试它、再退回 `<apk>.sha256`；`SHA256SUMS` 里没有这个 APK 时退回 `<apk>.sha256`；`SHA256SUMS` 的摘要对不上时下载失败、不留文件。`parseSums` 另测二进制模式、CRLF、只认精确文件名、同名冲突和非十六进制摘要。把「没列这个 APK 时退回」和「读 `SHA256SUMS`」分别改坏，冒烟都会失败。
- `actionlint .github/workflows/release-android.yml` 通过。
