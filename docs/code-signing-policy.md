# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

**Current status.** This repository's Windows releases are not signed through SignPath yet. The SignPath Foundation open source program has accepted the project `msime-windows`, whose current Windows product is built from [metasequoiaime/msime-windows](https://github.com/metasequoiaime/msime-windows) under [its own policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md). This repository holds the next-generation client of the same product, and we have asked SignPath Foundation to add it to that project. Until they do, the release workflow publishes unsigned installers, and a maintainer may replace them with installers signed on their machine with the project's Certum Open Source Developer certificate through SimplySign (`platforms/windows/installer/Package-SimplySign.ps1`). Once SignPath signing is enabled, installers signed with the SignPath Foundation certificate are built and signed only by the CI process described below, never on a local machine.

## What gets signed

Only Windows binaries the project builds from source in this repository, for each of the six editions (full, pinyin, wubi, japanese, vietnamese, tibetan):

- the Server (`MetasequoiaImeServer.exe`), the watchdog, the MCP server, the settings window and the other executables built with their debug symbols;
- the TSF text service (`MetasequoiaImeTsf.dll`, Win32, x64 and Arm64X) and the edition's host DLL (`msime_host_api*.dll`) beside it;
- the installers `MetasequoiaIME-<Edition>_Setup_v<version>.exe`.

Third-party binaries are shipped as their upstream built them and are never submitted for signing: the Windows App SDK runtime, ONNX Runtime, sherpa-onnx, and the libraries vcpkg builds from upstream sources. The selection rule is in [`SignPath-PackageBinaries.ps1`](../platforms/windows/installer/SignPath-PackageBinaries.ps1), and the matching SignPath artifact configurations are in [`platforms/windows/installer/signpath/`](../platforms/windows/installer/signpath/). The other platforms are signed through their own platform mechanisms and never through SignPath.

## Source repository

<https://github.com/metasequoiaime/msime>, licensed under GPL-3.0-only. The default branch is `develop`; `main` is the release branch.

## Team roles

| Role | Members | What the role may do |
| --- | --- | --- |
| Committers and reviewers | [fanlusky](https://github.com/fanlusky), [houko](https://github.com/houko), [J0ey2ou](https://github.com/J0ey2ou), [jsfaint](https://github.com/jsfaint), [linyanm](https://github.com/linyanm), [luojiyin1987](https://github.com/luojiyin1987), [Neptrue-Lin](https://github.com/Neptrue-Lin), [spectrumzero](https://github.com/spectrumzero), [Xibeilius](https://github.com/Xibeilius) | Push branches and merge pull requests into `develop` |
| Approvers | [houko](https://github.com/houko) | Approve signing requests in SignPath |

Approvers are the people configured as approvers of the `release-signing` policy in SignPath. They are the same for `msime-windows`, since both repositories sign through the same SignPath project. The organization account `metasequoiaime-dev` is used only by release automation and is not a person; it holds no role above. How roles are granted is described in the organization's [GOVERNANCE.md](https://github.com/metasequoiaime/.github/blob/main/GOVERNANCE.md).

Contributors without write access can still propose changes through pull requests. Their changes are reviewed and merged by a committer before they can reach a release.

## Build and release process

This is the process every SignPath-signed release goes through.

1. Every change lands in `develop` through a pull request. A branch ruleset on `develop` and `main` requires a pull request and the required CI checks, and blocks force pushes and branch deletion. Repository administrators can bypass the ruleset.
2. A release is cut from `develop` on a `release/<version>` branch and merged into `main` by pull request; `develop` itself produces beta releases.
3. A maintainer dispatches the [Windows release workflow](../.github/workflows/release-windows.yml) on `main` (final release) or `develop` (beta prerelease). Runs on any other branch build unsigned test installers and never submit a signing request. The workflow builds every edition from source at the dispatched commit. Every job that leads up to a signing request runs on a GitHub-hosted runner.
4. The workflow submits the project's binaries of all six editions as one signing request, compiles the installers around the signed binaries, and submits the six installers as a second signing request. An approver must approve each request in SignPath. The installers are then installed and uninstalled on clean x64 and Windows on Arm runners before the workflow publishes them with their SHA-256 digests.

## Privacy policy

Some features of the program send data over the network. What each feature sends, its default, and how to turn it off are documented in [PRIVACY.md](../PRIVACY.md).

## Reporting a problem

To report a signed binary that you believe was not built by this process, or any other security issue, see the [security policy](../SECURITY.md).
