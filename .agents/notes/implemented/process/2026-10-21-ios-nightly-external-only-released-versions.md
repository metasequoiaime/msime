# Agent Note: iOS nightly 只把已发布的版本号送外部 TestFlight

Status: implemented

## Problem

官网 msime.app 的 iOS 版本号取自最新的 `ios-v*` GitHub release（msime-web 的 `src/data/platforms.ts`），外部测试者在 TestFlight 里看到的是上传时的 `MARKETING_VERSION`。[六平台 nightly](2026-10-14-six-platform-nightly-builds.md) 让 iOS nightly 每天从 develop 构建、送外部测试组并提交审核，但不建 `ios-v` release。2026-10-05 `platforms/ios/version.txt` 随 0.52.0 改到 0.52.0，iOS 的发布工作流却一直没跑（最后一次是 10-02 的 0.51.5）。nightly 从 10-14 起把 0.52.0 送给外部测试者，官网到 10-21 仍显示 0.51.5，发布页和更新日志里也没有 iOS 0.52.0。

## Decision

`release-ios.yml` 在不发布（`publish=false`，nightly 和不发布的手动运行）时，只有这个版本号已经有 `ios-v<版本>` 或 `ios-v<版本>-<后缀>` 标签，才把构建送外部组并提交审核；没有就只交给内部组，并留一条警告。发布运行（`publish=true`）照旧送外部组，它自己会在最后建出那个标签。工作流把实际去向作为 `testflight_group`（`external` / `internal`）输出，`publish-nightly.py` 据此在 nightly 页上写「已提交 TestFlight，待 Apple 审核」或「仅上传 TestFlight 内部组：这个版本号尚未发布」，不再对没送审的构建说已送审。

结果是外部测试者看到的版本号一定已经出现在发布页和官网上。nightly 在同一个版本号下继续给外部测试者新构建（构建号递增），所以 #7767 要的「正式版之间也能装到新提交」只在 develop 改了版本号、iOS 还没正式发版的这段时间暂停。

## Alternatives considered

- **官网直接读 App Store Connect 里外部组的最新构建**：最接近用户实际能装到的东西，也算上了 Apple 审核是否通过。但 msime-web 要另存一份 App Store Connect API 私钥，官网的 iOS 版本号会出现发布页和更新日志里不存在的版本，只是把不一致挪到了另一处。
- **nightly 完全不送外部组**：一致性同样成立，规则更简单。但外部测试者在两个正式版之间再也拿不到新构建，直接推翻 #7767 对 iOS 的那部分，而版本号已经发布过的大多数日子里并不存在不一致。
- **nightly 送外部组时顺手建 `ios-v<版本>-nightly.N` 之类的预发布标签**：[六平台 nightly](2026-10-14-six-platform-nightly-builds.md) 已经否掉过：每晚一个版本标签会挤满发布列表，重跑会与正式标签冲突。

## Consequences

develop 改了 iOS 版本号之后，外部测试者要等 iOS 正式发版（main 发正式版，或从 develop 发 `-beta`）才能拿到新版本号的构建，这段时间 nightly 只到内部组。这把「改了版本号却忘了跑 Release iOS」从一个没人察觉的官网错版本，变成一个外部测试者拿不到新构建、nightly 页写明原因的可见状态。发布运行在上传送审之后才建标签，如果送审成功而建标签失败，仍会短暂不一致，重跑发布即可修正。

判定只看标签，不看 release 是否为草稿或已删除；`ios-v` 标签由 `scripts/publish-release.sh` 与 release 一起建，平时不单独存在。

## Verification

`scripts/test-nightly-partial-publishing.py` 23 项通过，新增内部组状态的渲染用例，以及 `NEEDS_RESULTS` 里的 `outputs.testflight_group` 传到 nightly 页正文的用例。`actionlint` 检查 `release-ios.yml` 与 `nightly.yml` 通过。标签判定在临时仓库里对已发布、未发布、只有 `-beta` 标签三种版本号，分别按发布与不发布运行核对：只有「未发布且不发布」一种组合改送内部组。没有在真实 nightly 上跑过内部组路径。
