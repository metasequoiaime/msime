# Agent Note: macOS 的 CI、打包检查和发版统一用 Xcode 26.3

Status: implemented

## Problem

`macOS 15 arm64`（必需检查）、Sanitizers 和 macOS 打包检查跑在 `depot-macos-15` 上，用镜像默认的 Xcode 16.4（macOS 15 SDK）；`release-macos.yml` 跑在 GitHub 的 `macos-15` 上，也没钉 Xcode，同样是 16.4。2026-10-09 一波七八个 PR 同时推送时，`depot-macos-15` 上同时跑着 9 个 job，后来的 `macOS 15 arm64` 排队 4 到 5 分钟（p90 230 秒），同一时段 `depot-macos-26` 的 p90 是 4 秒。

不能只把 CI 的 runner 换成 `depot-macos-26`：那个镜像只有 Xcode 26.0 到 26.5，没有 16.4（`depot-macos-probe.yml` 2026-10-08 的探测结果）。只换 CI，CI 就用 26 编、发版仍用 16.4 编，CI 测的不再是发出去的构建。另一方面，开发机早已是 Xcode 27，本机构建链接的是新 SDK，发出去的包却一直链接 macOS 15 SDK。

## Decision

- macOS 的 CI、打包检查和发版都用 `DEVELOPER_DIR=/Applications/Xcode_26.3.app/Contents/Developer`，与 `ci-ios.yml`、`codeql.yml`、`release-ios.yml` 已经钉的版本相同。
- `ci-macos.yml` 的 arm64 腿和 Sanitizers 改到 `depot-macos-26`（`depot-macos-15` 最高只有 26.1.1）；x86_64 腿留在 GitHub 的 `macos-15-intel`，那个镜像有 26.3（`ci-ios.yml` 的手写 job 已在用）。
- `ci-macos-package.yml` 的未签名 universal 包改到 `depot-macos-26`。
- `release-macos.yml` 仍在 GitHub 托管的 `macos-15` 上（它用 Developer ID 证书和公证凭据，不上 Depot），只加上同样的 `DEVELOPER_DIR`。
- check 名字保留 `macOS 15 arm64` / `macOS 15 x86_64`：分支保护按名字等检查，改名要同时改 ruleset。工作流里写明了它实际跑在哪里。

## Alternatives considered

- **只把 CI 的 runner 换到 `depot-macos-26`，发版不动**：改动最小，排队立刻缓解。不采用，因为 CI 和发版会用不同的编译器和 SDK，CI 绿而发版编不过、或者发出去的行为和 CI 测的不一样，这正是 CI 要防的事。
- **CI 留在 `depot-macos-15`，钉镜像里最新的 Xcode 26.1.1，发版也钉 26.1.1**：不用换 runner 也能统一工具链。但 iOS 已经钉 26.3，macOS 再钉一个不同的版本，仓库里就有两个 Apple 工具链要维护；而且不缓解 `depot-macos-15` 的排队。
- **改 check 名字为 `macOS 26 arm64`**：名字和实际一致。但要同时改 develop 和 main 的 ruleset，中间任何一步不同步，所有 PR 都会卡在等一个不会来的检查上。名字不对的代价只是读起来别扭。

## Consequences

- **收益**：CI、打包检查和发版用同一个编译器和 SDK；macOS 与 iOS 同一个 Xcode；`macOS 15 arm64` 不再挤在 `depot-macos-15` 上。
- **代价**：发出去的 App 改为链接 macOS 26 SDK。在 macOS 26 及以上，AppKit 和 SwiftUI 的系统控件（设置窗口、菜单、按钮）按新设计（Liquid Glass）绘制，和以前发出去的包外观不同；开发机上用 Xcode 27 编的本机构建一直就是这个样子。macOS 15 系统上的 arm64 运行测试没有了，部署目标仍是 13.0；x86_64 腿还在 macOS 15 上，但它只在发往 main 的 PR、定时运行和手动触发时跑。
- **未确定**：Depot 文档没说 macOS 15 和 26 是否共用同一份并发额度。如果共用，`macOS 15 arm64` 搬过去只是和 iOS 的 job 在 26 上抢机器，排队不会少。
- **以后换 Xcode**：`ci-macos.yml`（两处）、`ci-macos-package.yml`、`release-macos.yml` 和 iOS 那几处要一起改。

## Verification

- `actionlint` 通过。
- 本 PR 的 `macOS 15 arm64` 在 `depot-macos-26` 上用 Xcode 26.3 构建、跑 ctest、校验 bundle。
- 在本分支上手动触发 `ci-macos.yml`（跑 x86_64 和 Sanitizers）和 `ci-macos-package.yml`（打未签名 universal 包，与发版同一个 `package-release.sh`）。
