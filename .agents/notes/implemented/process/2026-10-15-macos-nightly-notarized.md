# Agent Note: macOS nightly 用 Developer ID 签名并公证

Status: implemented

## Problem

[六平台 nightly](2026-10-14-six-platform-nightly-builds.md) 接入时，`release-macos.yml` 把 Developer ID 证书和三项公证凭据都改成只在 `inputs.publish` 为真时读取，nightly 以 `publish: false` 调用它，于是每晚的 DMG 都是 ad-hoc 签名、未公证。ad-hoc 包不只是被 Gatekeeper 隔离、右键打开一次就好：macOS 不会把其中的输入法注册成输入源，普通用户下载后根本打不了字，只有用自己的 Developer ID 重签（`platforms/macos/scripts/install.sh`）的开发者能装。nightly 发布页上的 macOS 下载因此等于不存在，而 `README.txt` 只写了「当前 nightly 未公证，可能被系统拦截」，比实际后果轻得多。

当时的依据是 nightly 不碰正式发布密钥，但同一个 nightly 里 iOS 照样用正式签名上传 TestFlight 并提交外部审核；从 develop 手动触发的 beta 也一直带 Developer ID 签名和公证，同一个 workflow、同一类 GitHub 托管 runner。macOS nightly 读取这组 secrets 并不扩大它们的暴露面。

## Decision

`release-macos.yml` 的签名与公证跟着 secrets 走，不再看 `publish`：

- job 级环境变量 `MACOS_CERTIFICATE_P12_BASE64`、`MACOS_CERTIFICATE_PASSWORD`、`MACOS_SIGNING_IDENTITY` 直接取对应的 `MACOS_DEVELOPER_ID_*` secret；「Import Developer ID certificate」只判断证书是否配置。
- 「Build macOS package」的 `APPLE_ID`、`APPLE_TEAM_ID`、`APPLE_APP_SPECIFIC_PASSWORD` 直接取 `MACOS_NOTARY_*` secret。三项齐全且有签名身份时，`platforms/macos/package-release.sh` 公证并 staple 每个 DMG（各版本先全部提交、再统一取结果和按顺序装订，见[发布打包提速](2026-10-20-macos-release-faster-packaging.md)）；它以 `set -Eeuo pipefail` 运行，任何一个版本公证或 staple 失败就让构建失败，不会发出一个半公证的包。
- 对外的动作仍各有门控，不随签名放开：发布 `macos-v` release 的步骤 `if: inputs.publish`；更新 Homebrew tap 的步骤 `if: inputs.publish && !inputs.prerelease && github.ref_name == 'main'`，并另外用 `stapler validate` 核对。nightly 的 DMG 只进它自己的滚动 `nightly` prerelease。
- `scripts/package-nightly.py` 写进 `README.txt` 的 macOS 说明改为「DMG 已用 Developer ID 签名并公证，打开后把应用拖进「应用程序」，再打开一次即可安装输入法」。

没有配置这些 secrets 的仓库（fork）照旧退回 ad-hoc，`package-release.sh` 打印未公证的警告；这时 `README.txt` 的说法不成立，但 fork 不会发布这个仓库的 nightly。

`notarytool` 获批后，票据装订交给 `platforms/macos/scripts/staple-ticket.sh`：仅当 `stapler` 返回 68 且同时报告 `NSURLErrorDomain Code=-1005` 连接中断或 `CloudKit's response is inconsistent`（stapler 原文拼作 expections）时，依次在 15、30、60、90、120 秒后重试同一个 DMG，最多 6 次、合计约 5 分钟。Nightly `38806414451` 已出现这一下载断线，先前单次装订会丢弃已经完成构建和公证的整轮产物。重试不重复提交公证；其他错误及最终失败保留原退出码，打包仍然失败。

Nightly 不再向可复用 macOS workflow 传入完整的 GitHub `run_id`；复用 workflow 在 shell 中把未提供的构建号按 `999999999` 取模。`CFBundleVersion` 的构建号字段由 CMake 限制为 1 到 999999999；直接传完整 run ID 会在资源、签名都完成后才让 CMake 配置失败。取模得到 0 时仍映射为上限值，手动传入的构建号继续经过原有范围校验。

窗口最初是 15、30 秒后共三次。Release macOS 0.54.0（run `39267138324` 第 1 次尝试）的五笔版在 15:22:41 公证获批，15:22:49、15:23:11、15:23:48 三次装订都向 `api.apple-cloudkit.com` 的 `com.apple.gk.ticket-delivery` 查询时断线（`NSURLErrorDomain Code=-1005`，随后 `CloudKit's response is inconsistent with expections: (null)`，`Error 68`），一分钟内用完重试，已经编好、公证过的整轮产物作废，job 在 1 小时 7 分后失败，只能整个重跑。断线持续的时间比原窗口长，所以把窗口放宽到分钟级；只在这一组合上等待，签名或票据本身的错误仍立即失败。

`staple-ticket.sh --validate` 用同一个窗口执行 `stapler validate`，`release-macos.yml` 更新 Homebrew cask 之前用它核对每个 DMG。0.54.0 第 2 次尝试里，这一步直接调用的 `stapler validate` 对 full 版同样报 `NSURLErrorDomain Code=-1005` 和 `Error 68`，被当成未公证，msime cask 停在 0.53.0，而 job 照样成功，维护者事后手工更新了 tap。现在断线重试用完返回 75，workflow 提示「连不上票据服务、需手工更新」，与真正没有公证（stapler 的原退出码，cask 照旧跳过）分开。

## Alternatives considered

- **保持 ad-hoc，只把 README 写准确** — 最强的理由是 nightly 不需要公证的等待时间，也不让日常定时任务碰长期凭据。否定：写准确之后的内容就是「macOS 包普通用户装不上」，等于明说这个下载没有用；而凭据暴露面的理由在 iOS nightly 和 develop beta 面前不成立。
- **给可复用 workflow 加一个独立的 `sign` 输入，nightly 显式传 `sign: true`** — 最强的理由是签名与发布两个意图都显式写在调用方。否定：没有哪个调用方想要「有证书却不签」的包；多一个输入只是把 secrets 是否存在这件事重复声明一遍，`publish: false` 的手动构建（用来验证一个分支能不能打包）也应该产出能装的包，这正是 nightly 接入前的行为。
- **对全部装订错误重试**：实现简单，也能覆盖尚未观察到的服务波动，但会重复执行签名或票据错误等无法靠等待解决的操作。只重试日志已证实的退出码与连接中断组合，其余错误立即返回。

## Consequences

- **收益**：nightly 的 macOS DMG 与正式版一样，下载、拖进应用程序、打开一次就能打字；任何 `publish: false` 的手动构建也产出可安装的包，恢复到 nightly 接入前的行为。
- **代价**：每晚六个版本的 DMG 都要等 Apple 公证，macOS nightly 变长，`release-macos` 并发组被占用的时间随之变长；按[平台独立更新策略](2026-10-16-nightly-partial-platform-updates.md)，公证服务故障只让 macOS 保留上次成功包，其余成功平台继续更新。最初六平台任一失败都会阻止整个滚动 Release，该条件已被取代。如果公证排队频繁拖过 job 的 120 分钟超时，或 Apple 对公证次数设限，就要重访：例如只公证 full 版本，其余版本在 README 里说明。

## Verification

- `actionlint .github/workflows/release-macos.yml .github/workflows/nightly.yml` 通过；`python3 scripts/test-nightly-packaging.py` 通过。
- 以合成 run ID `40024049755` 计算出的构建号为 `24049795`，满足 CMake 的九位上限。
- 落地后第一次 nightly 的 macOS 任务日志里应出现 `notarytool submit` 与 `stapler staple`，且没有 `warning: not notarized`；下载其中一个 DMG 运行 `xcrun stapler validate` 应通过。
- `test-macos-stapler-retry.py` 用合成 `xcrun` 与 `sleep` 检查成功、断线后恢复、只报 CloudKit 应答异常时同样重试、6 次重试耗尽（等待 15/30/60/90/120 秒）和永久错误，核对同一 DMG 参数、等待间隔及最终退出码；不访问 Apple 服务或读取签名凭据。完整签名打包和服务恢复仍需实际 nightly 运行验证。
