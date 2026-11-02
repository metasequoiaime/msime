# Agent Note: macOS 版本发布页只放 DMG

Status: implemented

## Problem

0.54.0 的版本发布页 `macos-v0.54.0` 上有 20 个附件：六个 DMG、六个 `*-input-method.zip`、六个 `*-settings.zip`、一个 dSYM zip 和 SHA256SUMS。维护者的原话是「打包的时候不要分开的这种垃圾！分开下没意义！」，下载的人看着一长串附件不知道该选哪个。

这些 zip 不是给人下载的。它们是 Sparkle 的更新包：`macos-appcast` 上的 `appcast-<edition>.xml` 和 `appcast-settings-<edition>.xml` 的 `<enclosure url>` 指向 `releases/download/macos-v0.54.0/<名字>.zip`（[输入法本体经 Sparkle 自己更新](../feature/2026-10-15-macos-sparkle-input-method-updates.md)、[完整设置应用在应用内更新](../feature/2026-10-18-macos-settings-in-app-updates.md)）。直接删掉它们，所有 macOS 用户的应用内更新都会下载失败。地址也不能就地改：应用设了 `SURequireSignedFeed`，appcast 末尾带 Sparkle 的 EdDSA 签名（`<!-- sparkle-signatures: ... -->`），改一个字节都要用只存在于仓库 secret `SPARKLE_ED_PRIVATE_KEY` 里的私钥重新签。dSYM 用户同样用不上，但要长期保留来符号化崩溃报告，workflow artifact 90 天后过期，不能只放在那里。

## Decision

版本发布页 `macos-vX.Y.Z` 只放每个版本一个的 DMG 和只列这些 DMG 的 `SHA256SUMS`，发布说明开头是一段生成的下载指引。安装包格式从 0.57.0 起由 DMG 换成 pkg，见[macOS 只发 pkg 安装包](2026-11-02-macos-pkg-only.md)；发布页的这条标准不变。用户用不到的东西各有自己的滚动预发布（都不设为 latest，不进应用内更新检查，也不进官网的版本挑选：两者都只认 `<平台>-v<版本>` 的 tag）。

### Sparkle 更新包在 macos-appcast

- `package-release.sh` 的 `sparkle_download_prefix` 缺省是 `https://github.com/metasequoiaime/msime/releases/download/macos-appcast/`，更新包和 appcast 放在一起。zip 名本来就带版本号，不会互相覆盖。`SHA256SUMS` 只列 DMG；更新包的摘要写进单独的 `sparkle-SHA256SUMS`，随更新包一起放在 workflow artifact 和 `macos-appcast` 上。
- 「Update the Sparkle appcasts」（只在 main 的正式版之后）调用 `platforms/macos/scripts/publish-sparkle-feed.sh`，顺序固定：本地用公钥核对 appcast 签名、每个 zip 的长度与 `sparkle:edSignature` 和 `sparkle-SHA256SUMS` → 已发布的 appcast 构建号更高就停下（沿用原来的「不倒退」检查）→ 上传 zip 和 `sparkle-SHA256SUMS` → 按 appcast 里的公开地址逐个下载、核对摘要 → 上传 appcast、下载回来逐字节比对 → 删掉 `macos-appcast` 上没有任何 appcast 指向的 `*-input-method.zip` / `*-settings.zip`。清理按 appcast 实际指向的文件判断，不按版本号：这次没有发布的版本（例如从 `MACOS_EDITIONS` 里去掉的）仍指着自己的 zip，不能删。任何一步失败，在线的 appcast 和它指向的 zip 都保持原样；zip 先于 appcast 上线，旧 zip 在新 appcast 上线之后才删，中间没有 appcast 指向不存在文件的时刻。
- beta、alpha 和 nightly 的 zip 不进任何 release，只在 workflow artifact 里，与它们不更新 appcast 的规则一致。这一条已由[macOS 的更新通道](../feature/2026-10-25-macos-update-channels.md)部分取代：develop 的测试版发布和每日构建现在把各自通道的 appcast 和更新包（文件名带通道和构建号）传到 `macos-appcast`，`publish-sparkle-feed.sh` 只清理自己通道的更新包；alpha 仍只在 workflow artifact 里。
- 验签不需要私钥：`platforms/macos/scripts/sparkle_feed.py` 用 OpenSSL 3 的 Ed25519 按 `sparkle-public-key.txt` 核对。Sparkle 自带的 `sign_update --verify` 要读私钥（钥匙串或 `--ed-key-file`），用不上。appcast 的签名覆盖签名注释之前的 `length` 个字节，更新包的签名覆盖整个文件。

### dSYM 在 macos-symbols

`platforms/macos/scripts/publish-dsym.sh` 把每个发布（正式版、beta、alpha）的 dSYM 传到 `macos-symbols`，文件名带发布名：正式版是 `msime-macos-0.55.0-dsym.zip`，预发布是 `msime-macos-0.55.0-beta-dsym.zip`，免得同一版本号的 beta 与正式版互相覆盖。上传后用 GitHub 为资产记录的 SHA-256（`digest`）与本地文件比对，这就是它的校验记录。不清理，每个发布一份长期保留。

### 发布说明开头的下载指引

`edition_bundle.py download-guide` 从 `shared/contracts/editions.json` 生成一张表：每个版本的中文名、DMG 链接，正式版另加 `brew install --cask metasequoiaime/tap/<cask>`（Homebrew 只在 main 的正式版之后更新）。完整版排第一、标为推荐，说明大多数人下载它。名字、DMG 前缀和 cask 都取自版本表，加一个版本不用改文案。整段夹在 `<!-- msime-download-guide -->` 与 `<!-- /msime-download-guide -->` 之间，GitHub 上不显示，读发布说明做更新记录的地方可以据此跳过。

### 已发布的 0.54.0

`relocate-macos-release-extras.yml`（手动触发，缺省 `dry_run`）调用 `platforms/macos/scripts/relocate-release-extras.sh`：从 `macos-v0.54.0` 下载 12 个 zip、dSYM 和 SHA256SUMS 并核对；用公钥核对现行 appcast 和它们记录的长度与 edSignature；把每个 zip 的修改时间设回原来的 `pubDate`（`generate_appcast` 按修改时间写 `pubDate`），在 `TZ=UTC` 下用钉住的 Sparkle 2.9.6 的 `generate_appcast` 以新前缀重新生成；`sparkle_feed.py compare` 核对新 appcast 除地址外逐项相同；交给 `publish-sparkle-feed.sh` 上线，再从 `macos-appcast` 重新下载核对；dSYM 交给 `publish-dsym.sh`。只有这些全部通过，才把 `macos-v0.54.0` 的 SHA256SUMS 换成只列 DMG 的版本，然后删掉 13 个附件。Ed25519 签名是确定性的，同一个 zip 重新签出的 `sparkle:edSignature` 不变。脚本可以重跑：已经搬过的部分跳过。0.54.0 已经发布的发布说明不由这里改写。

## Alternatives considered

- **只把附件从发布页删掉，让 appcast 继续指向它们** — 最强的理由是不碰签名，零风险地满足「发布页干净」。否定：删掉附件就是删掉更新包，应用内更新全部失败；GitHub 也没有「隐藏附件」的办法。
- **更新包放进一个专门的 release（例如 `macos-updates`），与 appcast 分开** — 最强的理由是 appcast 所在的 release 只放小文件，清理规则更简单。否定：多一个滚动 release 就多一个要创建、要授权、要在文档里解释的地方；放在 `macos-appcast` 上，appcast 与它指向的文件在同一个地方，「先传 zip 再传 appcast、再按引用清理」在一个 release 里就能做完。
- **把更新包和 appcast 搬到官网镜像 dl.msime.app** — 最强的理由是国内下载快，GitHub 在国内常常只有几十 KB/s。否定：镜像在另一个仓库（msime-web），这个 workflow 没法原子地同时更新它；它的缓存从不重新校验，只适合内容不变的地址，而 appcast 每次发版都变。Sparkle 只认 appcast 里的一个地址、没有换源，镜像出错就没有退路。
- **用 `sign_update` 就地改 appcast 的地址再重新签名** — 最强的理由是不需要重新解包更新包。否定：`generate_appcast` 是发版时生成这些 appcast 的同一个工具，用它重新生成、再逐项比较，能证明除了地址什么都没变；手工改 XML 再签，证明不了这一点。
- **dSYM 留在版本发布页，只把它放到列表末尾** — 最强的理由是不多一个 release。否定：GitHub 按名字排序附件，没法控制位置；维护者要求发布页只有 DMG。

## Consequences

- **收益**：版本发布页从 20 个附件减到 7 个，发布说明第一段告诉人该下哪个。更新通道多了两道过去没有的检查：上线之前按公开地址把每个更新包下载回来核对摘要；appcast 和更新包的签名在发布前用仓库里的公钥核对，不再只是 grep 有没有 `sparkle:edSignature`。
- **代价与已知上限**：
  - `macos-appcast` 和 `macos-symbols` 两个预发布出现在 Releases 列表里。
  - 每次正式版发布多下载一遍全部更新包（六个版本约 1.1 GB），macOS runner 上多一两分钟。
  - 发布页的 SHA256SUMS 不再列更新包和 dSYM：更新包的摘要在 `macos-appcast` 的 `sparkle-SHA256SUMS`（只有当前版本）和 workflow artifact 里，dSYM 的摘要在 GitHub 的资产 `digest` 里。
  - `macos-appcast` 只保留当前版本的更新包，旧版本的更新包随下一个正式版删掉。回退到旧版本的 appcast 需要从 workflow artifact 或重新打包取回它们。
  - 同一版本重跑发布时，新 zip（字节不同）先于新 appcast 上线，中间几十秒里旧 appcast 的签名对不上新 zip，这段时间检查更新的客户端会验签失败，下次检查时恢复。
  - 镜像：dl.msime.app 的放行名单包含 `https://github.com/metasequoiaime/msime/releases/download/`，`macos-appcast` 上的更新包经镜像也取得到；但没有客户端这样取，Sparkle 直接从 appcast 写的 GitHub 地址下载。要让国内用户经镜像更新，得把 appcast 的下载地址换成镜像地址，那是另一个决定。appcast 和 `sparkle-SHA256SUMS` 的内容会变，不能经镜像取（镜像不重新校验）。
  - 官网下载页的更新记录（msime-web 的 `release-notes.ts`）会把下载指引当成正文显示，要按上面那对注释跳过，改在 msime-web。

## Verification

- `python3 scripts/test-macos-release-extras.py`：用一次性密钥和模拟的 `gh`、`curl` 跑完整流程。dry run 不发出任何上传或删除；正式运行后版本发布页只剩 DMG 和只列 DMG 的 SHA256SUMS，`macos-appcast` 上的 appcast 指向它自己、用公钥验签通过、与原来只差地址，dSYM 在 `macos-symbols`；第一次删除发生在 appcast 上传和 dSYM 上传之后；按公开地址下载到的更新包被篡改时，版本发布页和在线 appcast 一个字节都不变；SHA256SUMS 对不上时什么都不上传；`publish-sparkle-feed.sh` 拒绝倒退，只删没有 appcast 指向的更新包。设置 `MSIME_SPARKLE_ROOT` 时改用真的 `generate_appcast`，同样通过，说明重新生成只改了地址。
- `sparkle_feed.py verify` 对线上 0.54.0 的 12 份 appcast 用 `sparkle-public-key.txt` 验签通过，`msime-macos-japanese-0.54.0-input-method.zip` 的长度与 `sparkle:edSignature` 一致；篡改一个字节的 appcast 和更新包都被拒绝。
