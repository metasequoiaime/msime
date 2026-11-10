# Agent Note: 鸿蒙应用市场提审必须显式确认资料

Status: implemented

## Problem

`Release HarmonyOS` 在 #11173 中把 `appgallery` 的手动默认值改成 `auto`。从 `main` 正式发版时，工作流会自动上传 `.app` 并提交公开测试，但 Connect API 这条链路只更新包和测试版本，不上传开发者中心里的图标、截图、版本特性或审核账号。测试版本又默认继承华为控制台已有资料，因此 CI 可以在资料没有随版本核对的情况下直接送审。

0.4.0 的 main 发版运行（`42357552634`）没有执行 AppGallery 步骤；0.4.1 的 main 发版运行（`42595718956`）在 #11173 合入后解析成 `public-test`，上传包并提交了新的测试版本，随后审核暴露了旧图标、版本说明、审核凭据和应用内下载入口问题。

## Decision

`release-harmony.yml` 的 `appgallery` 默认值保持 `none`，选项只保留 `none`、`upload` 和 `public-test`，删除按分支和 `publish` 自动解析 `auto` 的路径。需要送华为时，发布者必须显式选择模式，并在 AGC 控制台确认本次版本的图标、截图、版本特性、审核账号和备案资料后再运行。`upload` 与 `public-test` 仍复用现有 API 上传本次签名 APP；资料不由这条 CI API 自动生成或覆盖。

离线测试拒绝 `auto`，并保留 `none`、`upload`、`public-test` 的输入校验和签名门禁。README 与发布技能同步说明显式提审流程。

## Alternatives considered

- 保留 `auto`，只把 `HARMONY_AGC_TEST_CONFIG_JSON` 改成必填：它只能约束公开测试配置，仍不能保证图标、截图和审核账号已更新，因此不能阻止同类误提审。
- 继续自动提交并在 CI 中生成截图和图标：应用市场资料还包含后台主体、备案和审核账号，且截图需要真实宿主验收；CI 无法安全替代 AGC 资料核对。
- 完全删除 AppGallery API 上传：会丢掉已经验证的上传、解析、恢复路径；保留显式 `upload/public-test` 可以继续自动传包，同时把资料确认留在正确的控制台步骤。

## Consequences

正式发版不再因为 `publish=true` 自动触发华为审核，发布者多一次显式选择和资料核对；误用旧提审资料的风险下降。没有变的是 GitHub release 与华为上传仍相互独立，华为步骤失败时仍可按 README 使用 `source_release` 或 `test_package_id` 恢复。真实审核账号、备案状态和控制台资料仍需在 AGC 中人工确认，离线测试不代替这一步。

