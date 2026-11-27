# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

**当前状态。** 本仓库支持两条独立的 Windows 签名路径。`WINDOWS_SIGNING_PROVIDER=simplysign` 使用维护者的 Certum Open Source Developer 证书，经专用签名 runner 自动签名；`signpath` 使用下文描述的 SignPath 流程。SignPath Foundation 已接受 `msime-windows` 项目，本仓库在完成同一项目的来源登记之前不启用其生产签名。变量未设置时保留未签名构建。

## SimplySign 自动发布

维护者在 `main` 上的提交推送 `windows-vX.Y.Z`（或 `windows-vX.Y.Z-beta.N`）tag 触发 Windows 发布工作流，也可以对已有的 tag 手动派发。构建固定到 tag 指向的源码提交；专用 `windows-signing` runner 分组只允许本仓库的发布工作流在发版 tag 上使用。TOTP seed 用签名用户的 Windows DPAPI 加密保存在该机器，不进入源码、artifact、命令行或 GitHub secret。

工作流复用固定摘要的社区客户端 `ssign`，它不是 Certum 官方客户端。签名机串行处理六个版本：保留包内已有有效上游签名，签署尚未签名的 EXE/DLL，编译并签署安装器，核对可信签名、发布证书指纹与时间戳，最后计算摘要。安装器随后在临时 x64 和 ARM64 runner 运行装卸和并存验证，全部通过才发布；配置和验证失败不会退回未签名发布。这条路径不使用 SignPath Foundation 证书，也不适用其人工请求审批流程。配置入口见 [安装器文档](../platforms/windows/installer/README.md#ci-自动签名发布)。

## What gets signed through SignPath

Only Windows binaries the project builds from source in this repository, for each of the six editions (full, pinyin, wubi, japanese, vietnamese, tibetan):

- the Server (`MetasequoiaImeServer.exe`), the watchdog, the MCP server, the settings window and the other executables built with their debug symbols;
- the TSF text service (`MetasequoiaImeTsf.dll`, Win32, x64 and Arm64X) and the edition's host DLL (`msime_host_api*.dll`) beside it;
- the installers `MetasequoiaIME-<Edition>_Setup_v<version>.exe`.

Third-party binaries are shipped as their upstream built them and are never submitted for signing: the Windows App SDK runtime, ONNX Runtime, sherpa-onnx, and the libraries vcpkg builds from upstream sources. The selection rule is in [`SignPath-PackageBinaries.ps1`](../platforms/windows/installer/SignPath-PackageBinaries.ps1), and the matching SignPath artifact configurations are in [`platforms/windows/installer/signpath/`](../platforms/windows/installer/signpath/). The other platforms are signed through their own platform mechanisms and never through SignPath.

## Source repository

<https://github.com/metasequoiaime/msime>, licensed under GPL-3.0-only. The default and only long-lived branch is `main`; releases are tags on `main`.

## Team roles

| Role | Members | What the role may do |
| --- | --- | --- |
| Committers and reviewers | [fanlusky](https://github.com/fanlusky), [houko](https://github.com/houko), [J0ey2ou](https://github.com/J0ey2ou), [jsfaint](https://github.com/jsfaint), [linyanm](https://github.com/linyanm), [luojiyin1987](https://github.com/luojiyin1987), [Neptrue-Lin](https://github.com/Neptrue-Lin), [spectrumzero](https://github.com/spectrumzero), [Xibeilius](https://github.com/Xibeilius) | Push branches and merge pull requests into `main` |
| Approvers | [houko](https://github.com/houko) | Approve signing requests in SignPath |

Approvers are the people configured as approvers of the `release-signing` policy in SignPath. They are the same for `msime-windows`, since both repositories sign through the same SignPath project. The organization account `metasequoiaime-dev` is used only by release automation and is not a person; it holds no role above. How roles are granted is described in the organization's [GOVERNANCE.md](https://github.com/metasequoiaime/.github/blob/main/GOVERNANCE.md).

Contributors without write access can still propose changes through pull requests. Their changes are reviewed and merged by a committer before they can reach a release.

## Build and release process

This is the process every SignPath-signed release goes through.

1. Every change lands in `main` through a pull request. A branch ruleset on `main` requires a pull request and the required CI checks, and blocks force pushes and branch deletion. Repository administrators can bypass the ruleset.
2. The version number is bumped on `main` by a pull request like any other change. A maintainer then pushes a tag on that `main` commit: `windows-vX.Y.Z` for a final release, `windows-vX.Y.Z-beta.N` for a beta prerelease.
3. The tag triggers the [Windows release workflow](../.github/workflows/release-windows.yml); a maintainer can also dispatch it on an existing tag. Before building, the workflow checks that the version in the tag equals the version in the tagged commit and that the commit is on `main`, and fails otherwise. Runs dispatched on a branch build unsigned test installers and never submit a signing request. The workflow builds every edition from source at the tagged commit. Every job that leads up to a signing request runs on a GitHub-hosted runner.
4. The workflow submits the project's binaries of all six editions as one signing request, compiles the installers around the signed binaries, and submits the six installers as a second signing request. An approver must approve each request in SignPath. The installers are then installed and uninstalled on clean x64 and Windows on Arm runners before the workflow publishes them with their SHA-256 digests.

## Privacy policy

Some features of the program send data over the network. What each feature sends, its default, and how to turn it off are documented in [PRIVACY.md](../PRIVACY.md).

## Reporting a problem

To report a signed binary that you believe was not built by this process, or any other security issue, see the [security policy](../.github/SECURITY.md).
