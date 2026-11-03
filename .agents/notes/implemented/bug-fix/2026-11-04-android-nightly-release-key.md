# Agent Note: Android nightly 改用正式发布密钥签名

Status: implemented

## Problem

#10709 反馈 Android nightly 各版本包的签名不一致：`release-android.yml` 的 `release` 是六个 edition 的矩阵任务，非发布构建没有正式密钥时，`signing.sh` 在每个 runner 上各自生成 `debug.keystore`，六个包六张证书。#10716 的修法是新增 `development-signing-key` job，从专用的 `ANDROID_NIGHTLY_KEYSTORE_BASE64`、`ANDROID_NIGHTLY_KEYSTORE_PASSWORD` secrets 解出一把固定的 nightly 开发密钥，经 workflow artifact 交给六个矩阵 job。

这个方案上线第二天就让 nightly 变红（#10739）：仓库里从没配置过这两个 nightly secret，`development-signing-key` 在检查处失败，Android nightly 整条失败，六平台的滚动 nightly 发布随之跳过。即使补上 secret，它还留着两个问题：nightly 的证书与正式版不同，装过 nightly 的设备装不上正式版，反过来也一样，这正是 `platforms/android/scripts/signing.sh` 写明「正式版、beta、本地开发构建一律用同一把发布密钥」要避免的局面（0.2.0 时本地构建换了证书，装过本地包的设备只能卸载重装）；签名密钥要先成为 workflow artifact 才能到达矩阵 job，多了一个在 job 之间传递密钥的环节。

## Decision

维护者 2026-11-04 决定：Android nightly 与所有 `publish=false` 的构建一律用正式发布密钥签名，取自已经配置好的 `ANDROID_RELEASE_KEYSTORE_BASE64`、`ANDROID_RELEASE_KEYSTORE_PASSWORD`，不引入、也不要求任何 `ANDROID_NIGHTLY_KEYSTORE_*` secret。

- 删掉 `development-signing-key` job 和矩阵里下载密钥 artifact 的步骤，`release` 只依赖 `source`。签名密钥不再经 workflow artifact 在 job 之间传递，各构建 job 直接从 secret 解出。
- 六个 edition 的 `Build native Android host` 步骤不分 publish 与否，都先检查两个 secret，再把密钥解到 `$RUNNER_TEMP/msime-release.p12`，用 `trap` 在步骤结束时删除，然后照旧经 `signing.sh` 签名。没有任何步骤上传或缓存密钥材料。
- secret 缺失时只在这一处明确失败，错误信息说明正式版、beta、nightly 与 `publish=false` 都要它；不退回开发密钥，免得悄悄打出另一张证书的包。
- 这个 job 本来就留在 GitHub 托管的 runner 上（发版密钥无法更换，不搬到 Depot），注释改回「不论 publish 与否都会解码发版签名密钥」。
- `scripts/test-android-nightly-signing.py` 守住这些约束：不得出现 `ANDROID_NIGHTLY_KEYSTORE` 和 `development-signing-key`；任何 job 的 upload-artifact、download-artifact、cache 步骤都不得带密钥文件、签名目录或 secret；构建步骤没有 `if:`，解码不在 `PUBLISH` 分支里，缺 secret 的检查全文件只有一处，且顺序是检查、解到 `$RUNNER_TEMP`、注册清理、解码、构建；发布密钥 secret 只在这一步读取，步骤里不开 shell 跟踪、不回显密钥或口令；矩阵仍是六个 edition；`nightly.yml` 以 `publish: false` 加 `secrets: inherit` 调用它。

这推翻了[Android nightly 各版本共用临时开发签名](../../archived/bug-fix/2026-11-03-android-nightly-shared-signing-key.md)（#10716）的决定，也取代了[六个平台共用发布构建链的 nightly 产物](../process/2026-10-14-six-platform-nightly-builds.md)里「nightly 的 Android 不使用正式发布密钥、用开发签名」那一项。专用 nightly 密钥被放弃的理由：它从未配置、已经让 nightly 红了；它让 nightly 与正式包永远不能互相覆盖安装，用户在两者之间切换就要卸载重装、丢掉本地词库和设置；它多出一把要长期保管的密钥和一个在 job 之间传递密钥的 artifact。旧决定反对 nightly 碰正式密钥，理由是扩大正式密钥的暴露面；macOS nightly 已经用 Developer ID 签名并公证（见[macOS nightly 用 Developer ID 签名并公证](../process/2026-10-15-macos-nightly-notarized.md)），Android 这里同样只在 GitHub 托管 runner 的单个步骤里解出、步骤结束即删、不进任何 artifact，维护者接受每晚多解码一次的代价，换取 nightly 与正式版同一张证书。

## Alternatives considered

- **补配专用 nightly secret，保留 #10716 的方案**：改动最小，能让 nightly 转绿；没有采用，因为 nightly 与正式版证书不同、不能互相覆盖安装，与 `signing.sh` 的统一证书规则相冲突，还要多保管一把密钥。
- **保留 `development-signing-key`，只把它解出的密钥换成正式密钥**：矩阵 job 仍共享同一份文件；没有采用，因为每个矩阵 job 都能直接读 secret，中间 job 不带来任何好处，却让正式密钥变成 workflow artifact。
- **回到每个 runner 各自生成 `debug.keystore`**：没有采用，这正是 #10709 六个包证书不一致的根因。
- **secret 缺失时退回开发密钥继续出包**：fork 或缺配置的仓库也能跑完；没有采用，因为那样会悄悄发出另一张证书的 nightly，用户装上后只能卸载重装，缺配置应当在 CI 上直接可见。
- **把私钥提交仓库，或每次 workflow 随机生成一把**：沿用旧笔记的否决理由，前者把私钥暴露在源代码历史里，后者下一晚就换证书。

## Consequences

- 正式版、beta、nightly、`publish=false` 构建和有发布密钥的本地构建用同一张证书（别名 `msime-release`）。同一 edition 的 nightly 与正式包具备互相覆盖安装的必要条件，能否覆盖仍受 versionCode 不能降级的约束；不同 edition 的 `applicationId` 仍由版本表决定。
- 没有配置发布密钥 secret 的仓库（例如 fork）运行 `release-android.yml` 会在构建步骤明确失败；此前 nightly 在 `development-signing-key` 同样会失败。本地构建与 `android-device.yml` 的模拟器构建不读这两个 secret，仍按 `signing.sh` 的顺序取密钥或退回开发密钥，行为不变。
- 发布密钥现在每晚都会在 GitHub 托管 runner 上解码一次，`release` job 不能搬到 Depot 的约束因此对 nightly 同样成立。
- 只改 workflow、契约脚本和文档，不涉及 APK 运行时代码。本地验证了 actionlint、契约脚本（含把旧 workflow、注入密钥 artifact 步骤、注入密钥缓存步骤、把解码放回 `PUBLISH` 分支、给构建步骤加 `if:` 这五种变体都判为失败）和 `scripts/run-checks.sh`；没有在本地跑发布 workflow。合并后由维护者重跑 nightly，再核对六个 APK 的证书 SHA-256 与正式版一致，并在真机上用 nightly 覆盖安装正式版、再用正式版覆盖回去。
