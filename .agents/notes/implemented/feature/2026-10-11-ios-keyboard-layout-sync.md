# Agent Note: iOS 账号同步带上触屏键盘布局，上传按服务端字段表过滤

Status: implemented

## Problem

iOS 的账号设置里，`input.schema` 那个方案用哪种触屏键盘只有一个布尔键 `platform.ios.nine_key`。[全拼 14 键](2026-10-10-fourteen-key.md) 加进来以后，14 键只能上传成 `input.schema=quanpin`、`nine_key=false`，和全拼 26 键分不开：别的 iOS 设备落成 26 键，本机「应用云端设置」时也会把 14 键改回 26 键。那篇笔记用 `scheme(keeping:)` 打了补丁（云端是全拼、不是九键时保留本机的 14 键），代价是另一台设备上特意选的 26 键覆盖不了这台的 14 键，并记下「`platform.ios.keyboard_layout` 落地后要重访」。手写同样上传成全拼 26 键。

msime-cloud #98（backend v0.54.0，2026-10-11 发布）给账号设置加了 `platform.ios.keyboard_layout`，不超过 32 字节的字符串，服务端不限取值。

原生 iOS 客户端还有一个问题：`BackendAccountClient.mergedPreferences` 见到服务端字段表（`/v1/users/me/preferences/schema`）里没有的键就抛错，整次上传失败。客户端比服务端先发布、多上传一个新键时（这一次就是），iOS 用户的「上传本机设置」会全部失败。Android（`android_account.rs`）和 Tauri 公共组件的 iOS 工程（`ios_account.rs`）早就按字段表过滤后再合并。

## Decision

- **上传按字段表过滤。** `mergedPreferences` 跳过字段表里没有的键，其余照常合并；字段表里有、类型对不上的值仍然拒绝；一个已知键都没有时也拒绝，不把一份什么也没改的文档当成上传成功。这与 Android、Tauri 的 iOS 工程一致，服务端还没部署 v0.54.0 时 `keyboard_layout` 留在本机，其他设置照常上传。
- **上传 `platform.ios.keyboard_layout`，同时照旧上传 `platform.ios.nine_key`。** 取值与 Android 的 `platform.android.keyboard_layout` 同一套词：`twenty_six_key`、`nine_key`、`fourteen_key`、`handwriting`。全拼的四个入口（全拼 26 键、9 键、14 键、手写）分别对应这四个值，日语 9 键是 `nine_key`，其余方案是 `twenty_six_key`。和九键开关一样只随 `input.schema` 上传，云端装不下的方案（粤拼、注音、越南文、藏文、笔画）两个键都不写。旧版本只认 `nine_key`，所以 14 键和手写在那里照旧落成全拼 26 键。原生 App 的映射是 `ChineseInputScheme.cloudKeyboardLayout`，Tauri 的 iOS 工程在 `local_account_preferences` 里。
- **下载时 `keyboard_layout` 优先。** 有认得的 `keyboard_layout` 就按它落成方案，`nine_key` 只在没有它时才读；两个键说法不一时听 `keyboard_layout` 的。不认识的取值（服务端不限取值，以后可能多出新布局）当作没有这一项，退回 `nine_key`，不拒收整份文档。原生 App 的 `IOSPreferencePlan` 和 Tauri 的 `IosPreferencePlan` 是同一条规则。
- **保留 14 键的补丁只对旧文档生效。** 云端写着 `keyboard_layout` 时，那就是那台设备选的布局，`twenty_six_key` 也会把本机的 14 键换成 26 键；没有这一项的文档来自还不上传它的版本，仍按原来的规则在云端是全拼、不是九键时保留本机的 14 键（`IOSPreferencePlan.scheme(keeping:)`、`IosPreferencePlan::requested_native`）。
- **版本过滤把它当作随方案同步的键。** client-core 的 `drop_unsynced_account_scheme`、原生 App 的 `IOSPreferencePlan.dropUnsyncedScheme` 和鸿蒙的 `SCHEME_COMPANION_KEYS` 都加上 `platform.ios.keyboard_layout`：单方案版本不上传也不应用它，否则 full 那边选的 14 键会让五笔版换布局。

## Alternatives considered

- **只上传 `keyboard_layout`，不再写 `nine_key`。** 最强的理由是一个事实只存一处，两键不会说法不一。但同一账号下还没升级的 iOS 设备只认 `nine_key`，不写它，那些设备收到的就是一份没有九键信息的文档，九键用户在旧设备上会被改成 26 键；下载时以 `keyboard_layout` 为准已经消除了不一致的影响。
- **不认识的 `keyboard_layout` 取值整份拒收。** 这与文档里其他字符串键（主题、震动强度）的做法一致，坏值不会被悄悄忽略。但服务端没有给这个键限定取值，Android 的同名键也在加布局（14 键就是后加的）；整份拒收意味着某个平台每加一种布局，所有旧 iOS 设备就再也应用不了云端设置。不认识时退回 `nine_key`，最多是布局不对，其余设置照常。
- **服务端部署之前先不上传 `keyboard_layout`，部署后再发版。** 不碰上传路径。但客户端和服务端的发布时间不受同一方控制，任何一次客户端先于服务端都会让 iOS 的上传整个失败；按字段表过滤是 Android 和 Tauri 早就有的规则，原生 iOS 漏了它才是缺陷。

## Consequences

- 同一账号下的新 iOS 设备之间，14 键、手写和 9 键都能同步，「应用云端设置」不再把本机的 14 键改回 26 键；另一台设备特意选的 26 键也能覆盖这台的 14 键。
- 旧 iOS 版本不读 `keyboard_layout`，在那里 14 键、手写仍落成全拼 26 键；旧版本上传的文档没有这个键，新版本按旧规则保留本机的 14 键。
- 服务端字段表里没有的键从此都留在本机、不报错。代价是：服务端退回旧版本或字段表漏了某个键时，用户看到「已上传」，那一项却没有上云，没有任何提示。
- 「应用云端设置」时，如果本机没有启用云端要求的入口（例如没打开 14 键或手写），照旧退回本机第一个启用的入口，与九键一样。

## Verification

- `swift test`（`shared/backend`）：`testKeysTheServerDoesNotKnowYetStayLocal`（旧字段表时 `keyboard_layout` 留在本机、其余合并，新字段表时一起上传）、`testKeyboardLayoutDecidesTheTouchSchemeWhenPresent`（四种布局、与 `nine_key` 冲突时听布局、26 键覆盖本机 14 键、不认识的取值退回 `nine_key` 并保留 14 键、日语与五笔、类型不对拒收）、`testMergePreservesOtherPlatformsAndReadRevision`（全是未知键或类型不对仍拒绝），以及版本过滤去掉这个键。
- `cargo test -p msime-desktop --lib -- platform::ios`：`upload_names_the_touch_layout_of_each_quanpin_entry`、`applying_cloud_prefers_the_ios_keyboard_layout`，原有的 `applying_cloud_quanpin_keeps_a_local_fourteen_key_choice` 不变。`cargo test -p msime-client-core --lib -- edition` 的版本过滤用例把这个键加进了夹具。
- iOS 27 模拟器 `MSIMEClientTests` 全套：`testEveryEntryNamesItsCloudKeyboardLayout` 和 `testCloudUploadCarriesTheKeyboardLayoutBesideTheNineKeySwitch`（`IOSCloudSettings.snapshot` 对 14 键、9 键、26 键分别带上布局和九键开关）。
- 没有对真实账号服务做上传、下载：backend v0.54.0 的发布（`gh -R metasequoiaime/msime-cloud release view backend-v0.54.0`）已经存在，但这次没有确认线上字段表里已经有 `platform.ios.keyboard_layout`；客户端两种情况都能工作（见第一条）。
