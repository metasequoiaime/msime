# Agent Note: 不碰长期凭据的 job 默认跑在 Depot 上

Status: implemented

## Problem

组织的 GitHub 托管 runner 同时最多跑 20 个 job，Windows、Linux、macOS 共用这份额度。2026-10-27 一次 Win7 发版加几个 PR 同时推送时，GitHub 托管的 job 跑满 19 个、另有 8 个排队，而 Depot 上同时只有十来个 job：`Windows native settings`（windows-2025，一次 17 分钟以上，每个动到 Windows 或共享代码的 PR 和每次 develop、main 推送都跑）一项就占了 7 个，其余是 Android 模拟器和一批一两分钟的仓库检查。这些 job 留在 GitHub 上没有记录过理由。

## Decision

job 默认跑在 Depot 上，只有下面几类留在 GitHub 托管的 runner，并在 `runs-on` 旁写明原因：

- **持有长期凭据**：引用了 `GITHUB_TOKEN` 以外的 secret（发版令牌、签名证书与密钥、公证与 App Store Connect 凭据、各发行渠道的上传凭据、跨仓库触发令牌）。这是既有约定，见 [macOS 发版打包](2026-10-20-macos-release-faster-packaging.md) 与 [Xcode 26](2026-10-09-macos-ci-and-release-on-xcode-26.md)，`release-android.yml`、`release-ios.yml`、`release-macos.yml`、`release-harmony.yml` 的注释也写着。`GITHUB_TOKEN` 每次运行临时签发，不算长期凭据，拿着它写标签、评论、取消运行的 job（`conflict-label`、`pr-triage`、`welcome`、`stale`、`cancel-closed-pr-runs`）照样上 Depot。
- **用 OIDC 签发身份**：npm 可信发布和 provenance、Pages 部署（`publish-web-engine-npm`、`release-script-sdk` 的 `publish`、`web-engine-demo` 的 `deploy`）。
- **需要 Depot 没有的硬件或环境**：Android 模拟器要 KVM（[Android 设备门禁](../testing/2026-10-08-android-device-gates.md)），x86_64 macOS（`macos-15-intel`）和 Windows on Arm（`windows-11-arm`）Depot 没有对应机型，Windows 候选皮肤截图要交互式桌面。
- **工具链即验收对象**：Win7 发版留在 `windows-2022`，Win7 二进制的兼容性是在那个镜像的 MSVC 和 Windows SDK 上核对的，Depot 的 Windows runner 只有 2025 镜像。
- **SignPath**：开源项目签名要求签名请求之前的每个 job 都在 GitHub 托管 runner 上（[SignPath 签名](2026-10-08-windows-signpath-ci-signing.md)）。

规格按负载选：一两分钟的检查用 `depot-ubuntu-24.04`，编 wasm 插件用 `-4`，编 Rust、前端或打 vendor 包用 `-8`，Windows 用 `depot-windows-2025-8`，macOS 构建用 `depot-macos-15`。新标签要同时加进 `.github/actionlint.yaml`。

Windows 包定义的离线契约检查同样接受 `depot-windows-*` 和 GitHub 托管的 `windows-*` runner 标签；两者都在 Windows 上执行 `Get-AuthenticodeSignature`。断言匹配完整的 `runs-on` 行，非 Windows runner 和注释中的标签均不能满足契约。

PR 分流检查按事件种类使用两个并发组：`edited` 的标题检查只取代上一次编辑检查，`synchronize`、打开和重开等完整检查只取代上一轮完整检查。推送后更新正文不会取消还在执行的 `Path labels`、`Size label`；编辑事件仍按原有规则跳过这两项，只检查标题。2026-10-27 的 #9990 与 #9906 在推送后改正文，原本共用 PR 号的并发组把 head 上的分流检查取消，而后续编辑检查又不运行路径和大小任务，留下取消状态；拆开并发组消除这条取消链。

组织里的其他仓库照同一规则改，各自一个 PR。

## Alternatives considered

- **只挪 `Windows native settings`**：它是最大的占用，改一处就能缓解大半。但几十个一两分钟的小检查每个也占一个名额，一波推送时照样排队；它们留在 GitHub 上又没有理由，不如按同一规则一次划清。
- **连持有凭据的发版 job 也挪**：Depot 更快。不采用：长期凭据只交给 GitHub 托管 runner 是项目早就定下的边界，凭据泄露的代价（例如 Android 签名密钥无法更换）远大于发版快几分钟。

## Consequences

Depot 的 `depot-windows-2025-8` 与 GitHub 的 `windows-2025` 不是同一个镜像：Depot 的装的是 Visual Studio 18（MSBuild 18），GitHub 的是 Visual Studio 2022；Depot 的镜像也不导出 `VCPKG_INSTALLATION_ROOT`，vcpkg 仍在 `C:\vcpkg`，`Windows native settings` 因此在开头补上这个变量。Depot 的 Windows 桌面只有约 800×600（GitHub 是 1024×768），设置界面验收需要按实际可用行宽判断布局，见[剪贴板布局验收](../feature/2026-10-20-windows-settings-page-tabs.md)。不把调用 `EnumDisplaySettings` 调大桌面作为构建前置：该 API 在 Depot 的当前会话中返回失败，会让原本能运行的编译和自适应布局验收直接跳过。挪过去的 Windows CI 换了编译器，与本来就在 Depot 上的 `release-windows.yml` 反而一致了。

GitHub 托管的额度只剩 Android 模拟器、发版里持有凭据的 job、少数特殊机型和截图，平时 PR 不再因为额度排队；Windows 设置编译从 4 核换到 8 核。代价是这些 job 依赖 Depot 的可用性，Depot 出故障时 PR 检查会卡住而不是变慢；Depot 的 macOS 并发本来就紧，`release-pack-tool` 的 macOS 构建挪过去后发 pack 工具时可能和 macOS CI 抢机器。

## Verification

`python3 scripts/test-windows-package-managers.py` 通过；合成 runner 标签回归接纳 `windows-2025` 和 `depot-windows-2025-8`，拒绝 Ubuntu、畸形标签以及注释中的 Windows 标签。`actionlint` 通过。PR 上会触发的检查（quality、contracts、ci-docs 等）由本 PR 的 CI 在 Depot 上实际跑一遍；`Windows native settings`、发版工作流和定时任务里挪过去的 job 要到下一次被触发时才实际跑到，`pull_request_target` 的几个要等下一个 PR。

分流并发组核对 `edited` 与四种完整检查事件的组名不同，同类事件组名相同，并用 `actionlint` 校验表达式；编辑事件的标题 job 保持执行，路径和大小 job 的条件不变。已有取消状态不会被修改工作流追溯改写，待后续提交或重跑完整检查恢复。
