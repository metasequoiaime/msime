# Agent Note: 网页引擎的 npm 发布改用 trusted publishing

Status: implemented

## Problem

`@msime/web-engine` 原本用仓库 secret `NPM_TOKEN` 发布。web-engine-v0.4.1 的 GitHub release 正常发出，但 npm 那一步在 PUT 时得到 `E404 Not Found`（run 39598625484），npm 上停在 0.4.0。npm 对没有写权限的 scope 包就返回 404，token 在场却被拒，说明它已过期或被撤销；同一份日志里 npm 还提示绕过 2FA 的 token 正在被限制直接发布。只换一个新 token，过一阵还会以同样的方式坏掉，而且坏的时候 release 已经发出、不能替换，只能事后补发。

## Decision

`publish-web-engine-npm.yml` 不再读任何 npm token，`npm publish` 通过 GitHub OIDC 走 trusted publishing，`id-token: write` 同时供 provenance 使用。`release-web-engine.yml` 调用它时不再 `secrets: inherit`。

npm 校验的是最外层调用方的 workflow 文件名，所以 npmjs.com 上 `@msime/web-engine` 的 Trusted Publisher 配两条：`metasequoiaime/msime` 的 `release-web-engine.yml`（发版时调用）和 `publish-web-engine-npm.yml`（单独手动补发）。新配置要在两天内完成第一次成功发布，否则需要重新配置。

trusted publishing 要求 npm CLI 11.5.1 以上，发布步骤先检查版本，低了直接失败，不让它退回 token 认证后报出误导人的 404。原先没有 `NPM_TOKEN` 就警告并成功退出的分支一起去掉：发版时 npm 没发出去应当让 workflow 变红，而不是只留一条容易被忽略的警告。

## Alternatives considered

换一个新的 granular token 写回 `NPM_TOKEN`：改动最小，几分钟就能补发 0.4.1。但写权限的 granular token 有有效期，到期后下一次发版又会在 release 已发出之后才失败，绕过 2FA 的 token 也在被 npm 收紧。trusted publishing 没有需要轮换的长期凭据，仓库里也就不再有能发布这个包的 secret。

## Consequences

npm 包能否发布取决于 npmjs.com 上的 Trusted Publisher 配置。改 workflow 文件名，或者新增一个直接发布的 workflow，都要同步改那边的配置，否则发布会在 PUT 时失败。`NPM_TOKEN` secret 不再被读取，确认 trusted publishing 发布成功后可以从仓库删掉。

## Related notes audit

按 `NPM_TOKEN`、`@msime/web-engine`、`npm publish`、`trusted publish` 检索活跃笔记：`2026-10-08-web-engine-shuangpin-helpcode.md` 与 `2026-10-09-candidate-skin-light-dark-slots.md` 只提到包名，讲的是引擎能力和皮肤语义，与发布凭据无关，原地保留。
