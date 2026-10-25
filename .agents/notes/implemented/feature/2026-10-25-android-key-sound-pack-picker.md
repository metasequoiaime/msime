# Agent Note: Android 键盘设置可选按键音效包

Status: implemented

## Problem

[移动端按键音](../bug-fix/2026-10-25-mobile-key-sound.md)之后，Android 键盘按共享偏好 `plugins.key_sound.pack` 播音效包，APK 也带了五个按键音包（清脆键盘、8 位游戏机、气泡、打字机、木鱼），但设置里没有地方选：唯一的路是在自定义皮肤里挑音效，再应用那套皮肤（`KeyboardSheets.applyDesign`）。用户问「有音效切换吗」时，答案只能是「去做一套皮肤」。

## Decision

三端用同一个交互：屏幕键盘的「按键反馈」组里，「按键音」开关下面一行「音效」，显示当前包名，按键音关着时置灰；点开列出按键音包，选中后写入 `plugins.key_sound.pack` 并试听一声。可选的包来自 `msime_client_plugins` 的 `catalog`，只取 `kind = sound`、`mode = keys` 的包（内置加用户安装的），旋律包和背景音乐不列。iOS 与鸿蒙各自的做法见 [iOS 按键音效包](2026-10-25-ios-key-sound-packs.md) 与 [鸿蒙触屏选按键音效包](2026-10-25-harmony-touch-key-sound-pack-picker.md)。

Android 的部分：

- `KeyboardOptionsPage` 的「反馈」分组加「音效」行，`OptionSheet` 列出包；目录和试听在 `KeySoundPacks` 里，读的是与键盘（`ImeKeyFeedback`）相同的两个根：`<filesDir>/sound-packs` 的内置包，和 `HostStore.directory` 偏好目录下用户安装的包。
- 过滤与排序在不依赖 Android 的 `KeySoundPackPolicy`：内置默认包排第一，其余按目录顺序，没有名字的包按 id 显示；偏好里的包不在列表里时行上照实显示 id（键盘那边会退回系统按键音）。
- 试听在设置进程里用一路 `SoundPool` 播该包的普通键样本，音频用途和键盘一样是 `USAGE_ASSISTANCE_SONIFICATION`（静音、勿扰时同样不响），音量取 `plugins.key_sound.volume`，播完两秒后释放。
- 键盘不需要改：偏好重读后 `ImeKeyFeedback.refreshPreferences` 发现换了包就重新加载，下一次按键起用新包。
- 设置搜索索引（`PageId`）加上「音效」。

## Alternatives considered

- 只把皮肤编辑里的音效选择挪个位置，不读插件目录，写死五个包：最省事。但用户导入的音效包就选不到，而且列表要跟着 `resources/sound-packs` 手工同步；目录本来就由 client-services 校验并给出，三端共用一份。
- 选中时不试听，等用户回键盘再听：少一段播放代码。但几个包名字差不多抽象（「气泡」「8 位游戏机」），不听一声很难挑。

## Consequences

音效从「做皮肤时顺带选」变成键盘设置里的一行，三端位置一致。代价是设置页每次打开键盘页时多一次插件目录读取（工作线程上，读几个 `plugin.toml`）。试听和键盘一样受静音、勿扰影响，在静音时选音效听不到试听声，这与键盘的实际表现一致。

## Verification

- `ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`：通过，新增 `KeySoundPackPolicySmoke`（只列按键音包、默认包排第一、无名按 id、未设置按默认、缺失按 id 显示），`SettingsSearchIndexSmoke` 覆盖新行。
- Pixel 3 XL（Android 12）装发布签名的完整 APK：键盘页「反馈」里有「音效：清脆键盘」，列表是五个按键音包且清脆键盘排第一；选「木鱼」后音频服务记录设置进程的试听播放，行上变成木鱼；回到键盘打字，输入法进程在第一次按键时加载木鱼的 SoundPool 并逐键播放。测完已改回清脆键盘。
