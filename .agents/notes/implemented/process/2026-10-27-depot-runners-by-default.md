# Agent Note: 不碰长期凭据的 job 默认跑在 Depot 上

Status: implemented

## Problem

组织的 GitHub 托管 runner 同时最多跑 20 个 job，Windows、Linux、macOS 共用这份额度。2026-10-27 一次 Win7 发版加几个 PR 同时推送时，GitHub 托管的 job 跑满 19 个、另有 8 个排队，而 Depot 上同时只有十来个 job：`Windows native settings`（windows-2025，一次 17 分钟以上，每个动到 Windows 或共享代码的 PR 和每次 develop、main 推送都跑）一项就占了 7 个，其余是 Android 模拟器和一批一两分钟的仓库检查。这些 job 留在 GitHub 上没有记录过理由。

## Decision

job 默认跑在 Depot 上，只有下面几类留在 GitHub 托管的 runner，并在 `runs-on` 旁写明原因：

- **持有长期凭据**：引用了 `GITHUB_TOKEN` 以外的 secret（发版令牌、签名证书与密钥、公证与 App Store Connect 凭据、各发行渠道的上传凭据、跨仓库触发令牌）。这是既有约定，见 [macOS 发版打包](2026-10-20-macos-release-faster-packaging.md) 与 [Xcode 26](2026-10-09-macos-ci-and-release-on-xcode-26.md)，`release-android.yml`、`release-ios.yml`、`release-macos.yml`、`release-harmony.yml` 的注释也写着。`GITHUB_TOKEN` 每次运行临时签发，不算长期凭据，拿着它写标签、评论、取消运行的 job（`conflict-label`、`pr-triage`、`welcome`、`stale`、`cancel-closed-pr-runs`）照样上 Depot。
- **用 OIDC 签发身份**：npm 可信发布和 provenance、Pages 部署（`publish-web-engine-npm`、`release-script-sdk` 的 `publish`、`web-engine-demo` 的 `deploy`）。
- **需要 Depot 没有的硬件或环境**：x86_64 macOS（`macos-15-intel`）和 Windows on Arm（`windows-11-arm`）Depot 没有对应机型，Android 模拟器要 KVM，普通 Depot GitHub Actions runner 不提供该设备（见[设备 runner 选择](2026-10-30-android-device-kvm-runner.md)），Windows 候选皮肤截图要交互式桌面，`Windows native settings` 要比 800×600 大的桌面。
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

GitHub 托管的额度只剩 Android 模拟器、发版里持有凭据的 job、少数特殊机型和截图，平时 PR 不再因为额度排队。代价是这些 job 依赖 Depot 的可用性，Depot 出故障时 PR 检查会卡住而不是变慢；Depot 的 macOS 并发本来就紧，`release-pack-tool` 的 macOS 构建挪过去后发 pack 工具时可能和 macOS CI 抢机器。Android 模拟器恢复在 GitHub `ubuntu-24.04` 上运行：先前单台 Depot 探测可用，后续新 runner 却没有 `/dev/kvm`，且普通 GitHub Actions runner 产品不提供该能力，见[设备 runner 选择](2026-10-30-android-device-kvm-runner.md)。

`Windows native settings` 第一次在 Depot 上跑就在「宽窗口的剪贴板行」断言上失败。原因不是显示缩放（两边滚轮一格都是 81 px），而是桌面大小：Depot 的 Windows 机型主屏只有 800×600（`GetSystemMetrics` 读到的），冒烟要的 1000×780 窗口停在设置窗口的最小尺寸 820×620（截图产物里的尺寸），窄窗口的排法把操作收进了「更多操作」；GitHub 的 windows-2025 截图是完整的 1000×780。直接换 1920×1080 时 `ChangeDisplaySettings` 返回 -2（这一档不支持），再用 `EnumDisplaySettings` 列出显卡支持的全部分辨率，只有 800×600 一档，桌面调不大。所以这一个 job 退回 `windows-2025`，归到上面「需要 Depot 没有的硬件或环境」：它要一块至少 1000×780 的交互桌面。冒烟里新增 `Assert-WindowSize`，以后窗口撑不到要的尺寸时直接报屏幕太小，不再在不相干的断言上失败。这也意味着 Problem 里占额度最多的这一项仍留在 GitHub 上，额度的缓解主要来自其余挪走的 job。

两个 Windows runner 的工具链也不同：Depot 使用 Visual Studio 18（MSBuild 18），GitHub 使用 Visual Studio 2022；Depot 没有导出 `VCPKG_INSTALLATION_ROOT`，已知安装在 `C:\vcpkg` 时由设置编译步骤补全变量。保留这项工具链适配不代表两者的交互桌面能力相同。剪贴板行的验收按实际可用宽度判断，见[剪贴板布局验收](../feature/2026-10-20-windows-settings-page-tabs.md)，完整宽窄覆盖仍以窗口达到目标尺寸为前提。


## Verification

`python3 scripts/test-windows-package-managers.py` 通过；合成 runner 标签回归接纳 `windows-2025` 和 `depot-windows-2025-8`，拒绝 Ubuntu、畸形标签以及注释中的 Windows 标签。`actionlint` 通过。PR 上会触发的检查（quality、contracts、ci-docs 等）由本 PR 的 CI 在 Depot 上实际跑一遍；`Windows native settings`、发版工作流和定时任务里挪过去的 job 要到下一次被触发时才实际跑到，`pull_request_target` 的几个要等下一个 PR。

分流并发组核对 `edited` 与四种完整检查事件的组名不同，同类事件组名相同，并用 `actionlint` 校验表达式；编辑事件的标题 job 保持执行，路径和大小 job 的条件不变。已有取消状态不会被修改工作流追溯改写，待后续提交或重跑完整检查恢复。

#9990 的 GitHub Windows 原生作业 `121957430356` 已验证浅、深色的 1000×780 和 800×780 全部设置页、候选数量真实拖动与保存恢复、画布两种布局及原生语音录键；上传截图的 PNG 尺寸与目标一致。合入该变更时保留 GitHub runner 和窗口尺寸前置断言，不用自适应行布局替代宽窄窗口覆盖。
