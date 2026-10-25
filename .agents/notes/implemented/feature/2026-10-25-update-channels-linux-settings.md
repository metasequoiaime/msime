# Agent Note: Linux 设置应用与共享设置页的更新通道

Status: implemented

## Problem

core 切片（[更新通道与后台检查下沉到共享 Rust](../architecture/2026-10-25-update-channels-core.md)）给出了偏好 `update_channel`、带通道的检查请求、结果里的 `channel` 与 `build_run`，以及共享的后台检查。用户要能在设置里选通道、看到按通道检查的结果，Linux 设置应用的后台检查和提醒要按选的通道走。共享 React 设置页同时被 macOS、Linux 和 HarmonyOS 加载，鸿蒙的发布不分通道；Linux 的 Tauri 设置应用里还留着一份和共享层重复的记录、节流与「只提醒一次」逻辑。

## Decision

### 共享设置页（`packages/ui`）

- 偏好类型 `Preferences.update_channel?: "stable" | "beta" | "nightly"`；`update-manifest.ts` 导出 `UpdateChannel`，`updateChannelOf` 把不认识的值读成 `stable`，与 client-core 读文档的规则一致。
- 「关于 › 版本与更新」在「当前版本」下面加一行「更新通道」（稳定版 / 测试版 / 每日构建），改动写进草稿，走正常的自动保存。是否显示由 `settingsCapabilities` 的 `showUpdateChannel = macos || linux` 决定：鸿蒙也加载这份页面但不显示；Windows 的设置界面是原生 WinUI，那一行由 Windows 切片画；没有宿主（网页预览）也不显示。
- `useUpdateCheck` 新增 `channel` 选项。页面模型在显示通道选择、且偏好已读出时传 `updateChannelOf(draft.update_channel)`，否则不传：
  - `checkUpdate` 的请求带上 `channel`；不传时请求里没有这个键，宿主按稳定版检查，鸿蒙和 Windows 的请求形状不变。
  - `pendingUpdateCheck(channel)` 只要按这个通道找到的版本。
  - `channel` 和平台、版本号等一样是检查的归属：换了通道时清掉当前状态和结果、作废进行中的检查，再按新通道读一次后台记录。
- 检查用的是草稿里的通道，不是已保存的：用户刚改了通道马上点「检查更新」，不必等自动保存。
- 结果显示通道：`describeUpdateVersion` 对稳定版仍是 `v1.2.0`（原有文案不变），测试版是「测试版 v1.2.0」，每日构建是「每日构建 v1.2.0（构建 123）」，没有构建号时省掉括号。宿主结果里的 `channel` 缺失或不认识时按稳定版，`build_run` 不是非负安全整数时按没有。
- 官网下载页只列正式版，所以测试版和每日构建的结果把「前往下载」换成「前往发布页」，打开结果自己的 GitHub 发布页（`release_url`，已校验在仓库发布页下）。国内镜像链接照旧按 tag 与附件名拼，每日构建的 tag 是 `nightly`。
- macOS 设置应用有 Sparkle 时「检查更新」走 `application_update_check`，和 `checkUpdate` 一样带上页面当前选的通道：`SettingsClient.checkForApplicationUpdate` 与 `UseUpdateCheckOptions.checkForApplicationUpdate` 改为 `(channel?) => Promise<void>`，命令收到通道时调 macOS 切片提供的 `msime_host_macos::check_settings_update_on_channel`，读该通道的 `appcast-settings-…` feed；不传时仍调 `check_settings_update()`（稳定版 feed），旧页面行为不变。不认识的通道名原生侧不检查，命令报 `application_update_unavailable`，页面显示「无法打开自动更新」。
- 后台记录里的测试版、每日构建只显示版本，不显示安装包细节。每日构建每天 18:00 UTC 重新上传，`publish-nightly.py` 删掉新 state 不再引用的附件；测试版重跑会原地换掉同名附件、摘要随之变化；而后台记录最多一天才刷新一次。记录里的安装包名和 SHA256 因此常常已对不上发布页：镜像链接 404，用户按页面的值核对发布页上的新包会对不上，看起来像被篡改。`useUpdateCheck` 返回 `installerDetailsStale`（显示的正是从后台记录读到的那个结果、且通道不是稳定版），这时页面不给镜像链接、校验值和签名提示，改为说明「这是之前在后台检查时找到的版本，发布页上的安装包可能已经更新」并请用户点「检查更新」；「前往发布页」照常给。手动检查得到的结果照常显示全部细节。稳定版的发布不会被替换，记录里的细节照常显示。

### Linux 设置应用（`apps/desktop/src-tauri/src/update_check.rs`）

- `update_check` 命令接受可选的 `channel`，原样放进 `UpdateCheckRequest`。
- 删掉本地的 `BackgroundRecord`、`background_check_due`、`background_check_disabled`、`background_notification`、`pending_update` 及其常量与测试，改调 `msime_client_services::update_check::background`：`BackgroundUpdateState` 存状态目录本身（记录和锁都在里面，文件名仍是 `update-check.json`）；后台线程调用 `run_background_check`，返回 `notify` 时发通知；`pending_update_check` 调 `pending_update`。
- 通道：后台检查读 `PreferencesStore` 里的 `update_channel`（读不出时按稳定版）；`pending_update_check` 优先用页面传来的通道（页面上可能已改、还没保存），不传时读偏好，传了不认识的通道名返回 `None`。后台检查和关于页用同一个 `linux_update_request` 拼请求（平台 `linux`、本机架构、版本 id、通道），记录才对得上。
- 开关：仍先看 `tauri::is_dev()`，再看共享层的 `automatic_checks_disabled()`（开发构建、构建时或运行时的 `MSIME_DISABLE_UPDATE_CHECK`），关掉时不起线程；Linux 发行版用 release 配置自编的包照旧检查。
- 保留 Linux 专有的部分：notify-send 通知、「查看」动作打开「关于」页、按安装包版本取应用名和图标、同时只跑一个检查线程。通知标题带通道：「水杉输入法有新版本 v1.2.0」「…有新的测试版 v1.2.0」「…有新的每日构建 v1.2.0（构建 123）」；测试版和每日构建的正文改为说明它们不经包管理器分发、要从发布页下载。
- 以前的 Linux 记录没有 `channel` 和 `notified_run`，共享层读成稳定版的记录：升级后选稳定版的用户不会对已提醒过的版本再收到一次通知，关于页照样显示记录里的版本。

### 宿主接线（`apps/desktop/src/main.tsx`）

Linux 的 `pendingUpdateCheck` 把页面传来的通道作为 `pending_update_check` 的参数；`checkUpdate` 照旧把整个请求对象交给 `update_check`，`channel` 随之传过去。鸿蒙的 `checkUpdate`（`apps/harmony/src/main.tsx`）只转发版本号、版本 id 和架构，页面在鸿蒙上也不会给出通道，不需要改。

### 文档

`platforms/linux/README.md` 的检查更新段落同步写明三个通道、按（版本号，运行 ID）比较、通知标题带通道、记录里存通道和换通道立刻重查。PRIVACY.md 的「资源与更新下载」已随 core 切片补上每日构建通道多出的 `releases/tags/nightly` 请求。

## Alternatives considered

- **由宿主报告能力（`HostCapabilities` 新增 `update_channels`）决定是否显示**：最「正统」，能力跟着宿主走。但 `HostCapabilities` 由 client-core 的 `host_surface.rs` 生成，属于 core 切片；而是否有通道完全由平台决定（三个桌面平台有、移动和鸿蒙没有，Windows 用原生页面），在页面里按平台判断已经精确，不值得为它扩一次跨语言契约。
- **页面只写偏好，`pending_update_check` 和检查一律由 Rust 读已保存的偏好**：请求形状不变，通道只有一个来源。但自动保存有延迟，用户改完通道立刻点检查会按旧通道查、关于页也会先显示旧通道的结果；页面本来就持有草稿，把它放进请求最直接，Rust 只在页面没给时读偏好。
- **在 Tauri 里保留一份记录逻辑、只加通道字段**：改动最小。但 Windows 常驻进程要用同一份记录规则，core 已经把它下沉；两份并存时 Linux 和 Windows 对「换通道」「同一构建只提醒一次」的理解迟早分叉。
- **打开关于页时自动重查非稳定版的记录**：用户一打开就拿到当前的安装包。但这让「打开设置页」本身多出一次联网请求，隐私说明里的「点了才查、后台每天一次」不再成立；让用户点一下「检查更新」代价很小。
- **后台记录里只去掉 SHA256、保留镜像链接**：镜像链接拼的是记录里的附件名，每日构建换了一次就 404，留着只会让用户下到失败。
- **测试版和每日构建也打开官网下载页**：入口统一。但官网只列正式版，用户点进去找不到刚提醒的那个构建；发布页上有附件和校验值，与页面显示的 SHA-256 是同一个文件。

## Consequences

- **收益**：macOS、Linux 用户能在共享设置页选通道，检查结果、后台提醒和关于页都按选的通道；Linux 设置应用少了一份与共享层重复的记录与节流代码，换通道立刻重查、同一构建只提醒一次与 Windows 一致。
- **行为变化**：结果 JSON 多出的 `channel`、`build_run` 页面都读；稳定版的文案和「前往下载」不变。Linux 的 `pending_update_check` 现在按（版本号，运行 ID）判断是否仍比运行的构建新（共享层的 `is_newer_build`），以前只比版本号；没有 `MSIME_BUILD_RUN` 的构建两者相同。
- **代价与上限**：
  - 选测试版或每日构建的 Linux 用户只收到提醒，安装仍要手动从发布页下载；包管理器装的稳定版不会因此换包。
  - 换了通道以后，Linux 后台要等下一次打开设置窗口才重查（共享层看到通道不同会立刻查）；设置窗口开着时，用户可以直接点「检查更新」。
  - 后台记录里的测试版、每日构建在关于页上只显示版本，要看校验值得再点一次「检查更新」。
  - 鸿蒙与 Windows 的隐藏靠平台判断：以后哪个平台也要通道，要改 `showUpdateChannel`。
- 相关笔记：[更新通道与后台检查下沉到共享 Rust](../architecture/2026-10-25-update-channels-core.md)（本篇的契约来源）；[Linux 面板与设置对齐 macOS](2026-10-20-linux-panels-and-setup-mac-parity.md)（部分重叠，其中的后台检查一条已改为指向共享层）；[检查更新移到 Rust](../architecture/2026-10-09-update-check-in-rust.md)（部分重叠，页面怎样展示结果）。

## Testing

- `apps/desktop/tests/settings/update-channel.test.tsx`：Linux 与 macOS 的关于页给出三个通道、缺省稳定版，检查请求带上选的通道（改了通道不等保存就按新通道查），偏好随之保存；偏好里的 `nightly` 读进选择框，不认识的值显示为稳定版并按稳定版查；鸿蒙与 Windows 不给通道、请求里没有 `channel`；每日构建结果写明通道与构建号、显示附件的校验命令、「前往发布页」打开 nightly 发布页；测试版写明通道，没有通道字段的旧结果照旧按正式版显示；后台记录按页面选的通道读，换通道后清掉旧结果并按新通道再读。
- 同一文件：macOS 有原生更新时，「检查更新」把页面选的通道（稳定版、改选后的测试版和每日构建，不等保存）传给 `checkForApplicationUpdate`，不走 `checkUpdate`；后台记录里的每日构建不显示校验命令、摘要和「国内镜像下载」，显示重查提示，「前往发布页」照常打开 nightly 发布页，手动检查之后显示新构建的校验命令和镜像链接；后台记录里的正式版照常显示校验命令和镜像链接。
- `update-check-action.test.tsx`：给了通道时请求带上、没给时不带；换通道作废进行中的检查。`update-manifest-edition.test.ts`：`channel` 与 `build_run` 的解析、非法值的回落、`describeUpdateVersion` 的三种文案、每日构建的镜像链接。
- `apps/desktop/src-tauri/src/update_check.rs` 的单测：通知应用名与图标、带通道的标题与正文、后台检查与关于页的请求形状、以前 Linux 写下的记录经共享层仍读成稳定版的记录且换通道后不再显示。记录节流、换通道重查、只提醒一次等规则的测试已随逻辑移到 `crates/client-services/src/update_check/background/tests.rs`。
