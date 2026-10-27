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

组织里的其他仓库照同一规则改，各自一个 PR。

## Alternatives considered

- **只挪 `Windows native settings`**：它是最大的占用，改一处就能缓解大半。但几十个一两分钟的小检查每个也占一个名额，一波推送时照样排队；它们留在 GitHub 上又没有理由，不如按同一规则一次划清。
- **连持有凭据的发版 job 也挪**：Depot 更快。不采用：长期凭据只交给 GitHub 托管 runner 是项目早就定下的边界，凭据泄露的代价（例如 Android 签名密钥无法更换）远大于发版快几分钟。

## Consequences

GitHub 托管的额度只剩 Android 模拟器、发版里持有凭据的 job、少数特殊机型和截图，平时 PR 不再因为额度排队；Windows 设置编译从 4 核换到 8 核。代价是这些 job 依赖 Depot 的可用性，Depot 出故障时 PR 检查会卡住而不是变慢；Depot 的 macOS 并发本来就紧，`release-pack-tool` 的 macOS 构建挪过去后发 pack 工具时可能和 macOS CI 抢机器。

`Windows native settings` 第一次在 Depot 上跑就在「宽窗口的剪贴板行」断言上失败。原因不是显示缩放（两边滚轮一格都是 81 px），而是桌面大小：Depot 的 Windows 机型主屏是 1024×768，设置应用把窗口限在屏幕的八成，冒烟要的 1000×780 只撑到 820×620（截图产物里的尺寸），窄窗口的排法把操作收进了「更多操作」；GitHub 的 windows-2025 截图是完整的 1000×780。处理是在冒烟之前加一步「Enlarge the desktop for the settings smoke」，用 `ChangeDisplaySettings` 把主屏调到 1920×1080 并把前后分辨率写进日志，冒烟里新增 `Assert-WindowSize`，窗口撑不到要的尺寸时直接报屏幕太小。Depot 的虚拟显示器不接受这个分辨率时，把这一个 job 退回 `windows-2025`：它需要一块够大的交互桌面，属于上面「要交互桌面」的例外。

## Verification

`actionlint` 通过。PR 上会触发的检查（quality、contracts、ci-docs 等）由本 PR 的 CI 在 Depot 上实际跑一遍；`Windows native settings`、发版工作流和定时任务里挪过去的 job 要到下一次被触发时才实际跑到，`pull_request_target` 的几个要等下一个 PR。
