# Agent Note: nightly 按平台更新并保留失败平台的上次成功包

Status: implemented

## Problem

nightly `38806414451` 的 Windows 与 Linux 成功，但 Android 设备验收、HarmonyOS 编译和 macOS 票据网络失败，使汇总发布整个跳过。用户明确指出六个平台相互阻塞不合理。各平台的安装文件互不依赖，单个用户也只安装其中一个平台。

## Decision

保留一个 `nightly` 下载入口。汇总在构建结束后无论部分成功或失败都运行，只下载构建和验收成功的平台产物；下载、预检或上传失败也只影响该平台。成功的平台更新全部版本，失败的平台保留上次成功的文件。页面按平台列出本轮结果、现有包的提交和时间，iOS 单独记录 TestFlight 提交状态，不发布模拟器包。

新文件名带本轮 run id 与 attempt，不覆盖旧文件。先上传并核对文件，再更新 Release 正文与版本化校验清单，最后清理已被替代的附件。平台状态与文件哈希存入正文的结构化标记，供下轮读取；旧 nightly 从现有正文的源提交与附件摘要迁移，不重下大包。全程不删除 Release，不用 `--clobber` 替换安装文件。失败平台仍让原构建 job 保持失败。

## Historical note audit

- [六平台共用发布链](../../implemented/process/2026-10-14-six-platform-nightly-builds.md)：部分取代。保留共用构建链，取代六平台全成功才发布的策略。
- [直接分发原生安装文件](../../implemented/simplification/2026-10-15-nightly-native-downloads.md)：部分取代。保留原生格式与 iOS TestFlight，取代要求平台齐全及所有文件同一提交。
- [macOS 签名公证](../../implemented/process/2026-10-15-macos-nightly-notarized.md)：部分重叠。公证失败仍阻止该 macOS 包发布，但不再阻止其他平台更新。
- proposed 与 rejected 检索没有相同方案。

## Alternatives considered

- **六个平台独立的滚动 Release**：发布互不争用，也不用等待最慢平台。但会改变已有统一下载入口和附件布局；本次保留单入口，通过平台结果与文件集合隔离失败。仍需等待本轮构建结束才能更新汇总，这是本方案明确保留的限制。
- **只放宽成功条件，删除旧 Release 后发布成功平台**：代码最少，但失败平台会丢掉最后可用包，上传中断也会让全部平台短暂不可下载，所以不采用。
- **对同名附件直接 `--clobber`**：保留原有名字，但 GitHub 会先删除旧附件再上传，失败时旧包丢失；增加运行标识以允许新旧并存，正文提交成功后才清理。

## Verification

22 项合成发布测试通过，覆盖部分平台失败、首次部分成功、全失败、平台上传中断、摘要不符、产物下载失败、汇总提交失败、清理列表或删除失败、旧布局迁移与连续两轮更新。失败平台文件逐字节保留，成功平台文件与校验清单一致；API 模拟验证仅 404 视为不存在，附件分页完整读取，查找并复用中断遗留的 draft，以及首次 draft 发布后再移动标签。原有 3 项原生分发打包测试通过，actionlint、`pnpm run verify-notes` 与 diff check 通过。macOS 上的 `bash scripts/verify-local.sh --quick` 通过：契约检查、Rust workspace 编译及共享 Apple 桥接编译通过，平台宿主阶段按本次改动范围跳过；没有把它当成六个平台的设备验收。

只读访问现有 nightly 完成旧布局解析：Android 6、macOS 6、Linux 24、HarmonyOS 1、Windows 6 个原生文件，以及 iOS TestFlight 状态，均归属原有提交 `70db2dfb2d13b8269b33271a9bc6097fe00dd5cf`；没有重新下载大包或改动线上 Release。未触发真实发布。

## Consequences

GitHub 的附件和正文不支持跨请求事务；正文提交前中断会留下尚未列为当前版本的新附件，旧包与旧正文保持可用；正文提交后中断时，新正文引用的文件已经完整，只可能多留旧附件。单个平台内部仍以现有完整构建和验收为更新单位，不发布未通过设备验收的 APK。后续若需要最慢平台结束前即可下载，应进一步拆分发布入口或安排独立更新锁。现有 `nightly-builds` 并发组继续串行化不同轮次，避免两个汇总同时覆盖平台状态。完整远端 nightly 和真实网络中断恢复尚未运行，不把模拟发布当作真实分发验证。
