# Agent Note: macOS 设置应用在后台检查并下载更新包

Status: implemented

## Problem

#11681 希望 macOS 用户不必每次手动点「检查更新」，设置应用能自己发现新版本并把更新包准备好。[设计提案](../../proposed/feature/2026-11-20-macos-automatic-application-updates.md)列出了待产品决定的几项，维护者在 [#11681 的评论](https://github.com/metasequoiaime/msime/issues/11681#issuecomment-7215082122)里定了第一步：默认关闭、由用户开启；只自动检查、下载和校验，安装仍由用户点「现在安装」并经过 Sparkle 正常的管理员授权；不做下次启动时安装，不扩展 root 更新助手；开关放在原生设置里，不往 `packages/ui` 加。

## Decision

共享 Rust 偏好新增 `update_auto_download`（`crates/client-core/src/preferences.rs`），默认 `false`，关闭时不写进文档。它和 `update_channel` 一样只属于本机：列进 `settings_document::LOCAL_SECTIONS`，导出设置文件不写、导入时保留本机的值；账号设置同步按白名单上传，不收录它。

设置应用的 Sparkle 后台模式由设置窗口的主实例按这个偏好打开：

- 主实例在启动时（`macos_settings_launch` 为真，设置窗口在 macOS 上是单实例）声明自己负责后台更新（`update_check::own_macos_background_updates`）。面板进程从不声明，也就从不打开后台检查，同时开着几个面板也只有一处在检查。
- 偏好监视（`desktop_preferences_monitor`）每次轮询读到偏好时交给 `update_check::sync_macos_background_updates`，选择（开关、通道）和上次不同时才回主线程调用 `msime_host_macos::configure_settings_background_updates`。所以 MCP、`msime-mcp config apply` 或以后的原生设置页改了偏好，几百毫秒内生效。开发构建和设置了 `MSIME_DISABLE_UPDATE_CHECK` 时按关闭处理，与 Linux 的后台检查一致。
- 原生端（`crates/host-macos/native/updater.mm`）把选择写成 Sparkle 的 `automaticallyChecksForUpdates` 和 `automaticallyDownloadsUpdates`：打开时先开检查再开下载（Info.plist 没有 `SUAllowsAutomaticUpdates`，Sparkle 用检查开关代替它），关闭时反过来。两项存在本应用（`app.msime.macos`）的用户默认值里，盖过 `settings_updater.py` 写进 Info.plist 的 false；包里的默认值不变。关闭且更新器还没创建时不加载 Sparkle；之后无论哪条路径创建更新器，启动前都先按当前选择设好这两项，以前留在用户默认值里的打开状态不会让这次启动先排一次后台检查。
- 后台检查按偏好里保存的 `update_channel` 取 appcast（`MSIMEUpdateFeedURLForChannel`，与手动检查同一条规则），周期是 Info.plist 的 `SUScheduledCheckInterval`（一天）。

### 为什么不会静默安装

这一点靠 Sparkle 2.9.6 自己的流程，不需要另写拦截：

1. 后台检查打开自动下载时，Sparkle 用 `SPUAutomaticUpdateDriver`，它以 `requiresSilentInstall:YES` 运行。
2. 下载完、要启动安装器时，`/Applications/MSIME.app` 归 root，需要授权；静默模式不允许弹授权，安装器启动器返回 `SUInstallerLauncherAuthorizeLater`，驱动以 `SUInstallationAuthorizeLaterError` 结束，并保留已下载的更新（`SPUCoreBasedUpdateDriver` 对这个错误不清理下载）。
3. `SPUUpdater` 随即排一次立即检查，用带界面的 `SPUScheduledUpdateDriver` 恢复这份下载，标准界面提示新版本已可安装。用户点安装才弹管理员授权；安装器在解压前校验归档的 EdDSA 签名（`SUVerifyUpdateBeforeExtraction`），再替换应用并重启。

后台阶段已校验的是 appcast 的签名（`SURequireSignedFeed`）；更新包本身的签名在用户授权后、解压前由 Sparkle 的安装器校验，没通过就不替换。取消授权时 Sparkle 删掉这份下载，旧版本照常可用，下一次后台检查重新下载。

### 界面入口

开关的界面这次不做。macOS 设置应用的「关于」页是 `packages/ui` 的共享 React 页，迁移期只修缺陷、不加新设置；IMK 进程里的 AppKit 设置窗口是冻结的回退（[五笔入口的迁移边界](../../proposed/feature/2026-11-26-issue-11863-macos-native-migration.md)同样因此没有加行），而且它驱动的是输入法自己的 Sparkle。开关随 macOS 原生设置迁移接入；在那之前可以用 `msime-mcp config apply` 写合并补丁 `{"update_auto_download": true}`。`scripts/test-preferences-field-parity.py` 的 `RUST_ONLY` 记着这一项。

## Alternatives considered

- **自己在设置应用里下载并校验更新包，再交给 Sparkle 安装。** 能在下载完就验 EdDSA 签名、显示摘要，不必等授权。但要自己解析 appcast、下载、验签，等于重写 Sparkle 已有的一段，还要和 Sparkle 的恢复流程对接；Sparkle 的后台驱动已经做到「下载、不静默安装、提示用户」，签名也在替换前验，所以不选。
- **在 Info.plist 里把 `SUEnableAutomaticChecks`、`SUAutomaticallyUpdate` 改成 true。** 最省事，但这就成了默认开启，违背维护者「默认关闭」的决定；而且同一个 bundle 的面板进程一旦创建更新器也会各自检查。
- **用文件锁保证只有一个进程检查。** 提案里写了共享状态目录的独占锁。实际只有设置窗口的「关于」页会创建更新器，而设置窗口已经由 `tauri_plugin_single_instance` 保证单实例，后台模式又只在声明了主实例的进程里打开，锁不再挡住任何情形，所以不加。
- **把开关加进冻结的 AppKit 回退窗口或 React 关于页提前交付。** 用户马上就能在界面里打开，但违反迁移期规则（#11902 就因此关闭），维护者也明确说不往 `packages/ui` 加。

## Consequences

- 收益：打开开关的用户在设置窗口开着时，新版本会在后台下好，只剩点安装和输入管理员密码；稳定版用户不会因此被换到测试版或每日构建，通道仍由 `update_channel` 决定。默认关闭，老用户的行为不变。
- 代价：macOS 的设置进程随窗口关闭而退出，后台检查只在设置窗口开着时进行；不开设置窗口的用户得不到这个功能，直到原生容器 App 或别的常驻宿主接手。
- 代价：这一步没有界面入口，普通用户还打不开它。
- Sparkle 的两项设置存在 `app.msime.macos` 的用户默认值里，偏好是唯一来源：主实例每次启动和偏好每次变化时按偏好重写它们。
- 提案里的「安全时机」一段（设置应用不再 `pkill` 正在组字的输入法，交给输入法自己在空闲时退出）不在这一步里：安装仍是用户主动点的，和今天手动更新的路径相同，自动化只到下载为止。它和原生设置迁移一起留在[提案](../../proposed/feature/2026-11-20-macos-automatic-application-updates.md)里。
- 它部分改变了[设置应用在应用内更新完整安装包](2026-10-18-macos-settings-in-app-updates.md)里「只在用户主动检查时启动更新器」的决定：默认仍是如此，用户打开 `update_auto_download` 后由主实例在后台检查。

## Verification

- `msime-client-core`：默认关闭、关闭时不写进文档、类型不对时报错、保存后读回、恢复默认回到关闭；设置文件导出不写、导入保留本机的值。整个 crate 的测试在 Ubuntu 上通过。
- `msime-desktop` 的 `update_check` 测试在 Mac Studio 上通过：选择跟随偏好与通道、关掉后台检查时按关闭、只有选择变化时才交给 Sparkle。`msime-host-macos` 的 clippy（含 `updater.mm` 的编译）通过。
- 没有在真实的发布包上验证一次后台下载到提示安装的全过程：需要签名的 feed 和一个更新的已发布构建，等带这项改动的版本进入测试版通道后再验。
