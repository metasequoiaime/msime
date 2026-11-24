# Agent Note: TestFlight 构建写上「What to Test」

Status: implemented

## Problem

iOS nightly 每天把一个新构建送进 TestFlight（内部组，版本号已发布时也送外部组，见 [nightly 只送已发布版本号](2026-10-21-ios-nightly-external-only-released-versions.md)），但 `release-ios.yml` 从来没有写构建的「What to Test」（App Store Connect 的 `betaBuildLocalizations.whatsNew`）。测试者每天收到一个说明为空的新构建，不知道这一版改了什么、该测什么。发布说明生成器 `scripts/generate-release-notes.py` 早就有，但只在 `publish=true` 的正式发布里跑，nightly 是 `publish=false`，从不生成。

## Decision

`release-ios.yml` 在上传 TestFlight 后、分发之前，用 `generate-release-notes.py --platform ios` 生成这一版的说明，`submit_testflight_review.py --whats-new-file` 在构建处理完成后、加入外部组和提交审核之前，把它写进这个构建的 `whatsNew`。

- **起点**：nightly 取滚动 release `nightly` 正文状态里的 `platforms.ios.source`，即上一个送进 TestFlight 的 iOS nightly 的源码提交。整轮 nightly 因别的平台失败时这个字段照样更新；前一天 iOS 失败时它不动，今天的说明自动累积两天的改动。字段缺失或提交不在仓库里时，退回最近的 `ios-v*` 标签。正式发布直接用与「Publish independent iOS release」同一条规则选标签。
- **格式**：TestFlight 不渲染 Markdown，`testflight_text` 去掉标题井号、粗体星号和反引号；上限 4000 按 UTF-16 码元计（分组标题的 🐛 占两个），超出时在整行处截断，页脚（构建号、源码提交、GitHub compare 链接）始终保留。
- **语言**：写进应用主语言和每个已有的测试信息语言（`betaAppLocalizations`）；Apple 已替构建建好的语言条目用 PATCH 改，没有的 POST 新建。
- **失败**：生成步骤 `continue-on-error`；写入失败（文件缺失、API 报错）时打 `::error::`，照常分发和提交审核，最后让这一步以 1 退出。测试者拿不到构建比拿到没写说明的构建更糟，但说明不能再悄悄空着。

同一次改动修好了 `TestFlightSubmissionTests.py` 里 7 个早已过期的用例（脚本改成每次请求现签 token、用绝对 deadline 后测试没跟上），并让 `ci-ios.yml` 在独立 venv 里装与发布作业同版本的 `pyjwt[crypto]==2.10.1` 再跑这套测试。之前 CI 没有 pyjwt，整套测试一直被跳过，所以没人发现。

## Alternatives considered

- **起点取上一次 nightly 工作流运行的提交（`gh run list`）**：不用解析 release 正文。但 nightly 几乎每天因为某个平台红而整轮记为失败，按「上一次成功的运行」会一路退回很久以前；按「上一次运行」又会在 iOS 自己失败的那天漏掉改动。`nightly` 状态里的 `ios.source` 记的正是上一个真正送到测试者手里的 iOS 构建，最准确。
- **起点固定取最近的 `ios-v*` 标签**：最简单，与正式发布一致。但 nightly 是每天一版，从上一个正式版累积起来的列表会越来越长、很快触到 4000 字符上限，测试者看不出「今天这一版」新改了什么。它只作为拿不到 nightly 状态时的退路。
- **写入失败时直接让步骤失败、不分发**：最容易被注意到。但外部组就拿不到当天的构建，用一个说明问题换掉了整个分发。

## Consequences

测试者每天在 TestFlight 里看到这一版相对上一个测试版改了什么，以及 GitHub compare 链接。nightly 多一次 EveryAPI 调用（拿不到 token 或模型失败时，生成器退回按 PR 标题分组的确定性版本）和几次 App Store Connect 请求。没有新提交的日子写的是「本次版本没有检测到该平台相关的代码改动」。正式发布会生成两次说明（TestFlight 一次，GitHub release 一次），起点规则相同，内容可能因模型输出而措辞不同。

## Verification

`TestFlightSubmissionTests.py` 在装了 pyjwt 的 venv 里 15 项全过：新增纯文本转换、按 UTF-16 截断保留页脚、主语言与测试信息语言的 PATCH/POST、说明先于加入外部组和提交审核、写入失败照常分发但返回 1 五个用例，旧的 7 个用例修复后通过。生成步骤的 shell 在本机按 nightly 路径跑过：从真实 `nightly` release 取到 `ios.source`，输出文件和页脚正确；以 11-14 的提交为起点生成了 33 个 iOS 提交、32 项改动的说明，转换后 1343 个码元。`actionlint` 检查 `release-ios.yml` 与 `ci-ios.yml` 通过。没有在真实 App Store Connect 上跑过写入，下一次 nightly 是第一次。
