# Agent Note: 移动端按键音：安卓播内置包、鸿蒙触屏接上播放、iOS 说明完全访问

Status: implemented

## Problem

三个移动端都有「按键音」开关，打开后却可能一声不响，原因各不相同。

安卓：偏好 `plugins.key_sound.pack` 没选或选 `default` 时，`ImeKeyFeedback.playSound` 不播 APK 里打包的 `default` 包（「清脆键盘」），而是调用系统的 `AudioManager.playSoundEffect(FX_KEYPRESS_STANDARD, volume)`。[九键反馈](2026-10-23-android-nine-key-feedback.md)把它从单参数接口换成双参数接口，绕过了系统触摸提示音开关的检查，但系统按键音本身仍由系统管：系统设置页关掉「触摸提示音」时会调 `unloadSoundEffects()`，之后音频服务照样收下播放请求却没有样本可播；部分厂商系统还会另外屏蔽这一路。那次的设备测试在断言前主动调了 `loadSoundEffects()`，正好把卸载这种情形排除在外，所以模拟器上通过而真机仍然无声。默认值是 `default`，几乎所有安卓用户走的都是这条路；AI 皮肤页也把 `default` 标成「清脆」。

HarmonyOS：`KeySoundPlayer` 只给 2in1 配置（手机和平板一律 `KEY_SOUNDS_OFF`），触屏按键的反馈入口 `tapFeedback` 只振动，`key-feedback.json` 里的 `sound` 没有任何代码读。手机和平板上的开关能拨、能保存，从来不出声。

iOS：键盘用 `UIDevice.playInputClick()`（输入视图实现 `UIInputViewAudioFeedback`）。Apple 的自定义键盘文档把「播放音频，包括用 `playInputClick` 播放按键音」列为开启 open access 后才有的能力；没给「允许完全访问」时开关打开了也没有声音，键盘和设置页都没说明。系统「键盘反馈 › 声音」关闭或静音模式下同样不响，这与系统键盘一致。

## Decision

安卓：`default` 和其他声音包走同一条路，经 `NativeClient.keySoundPack` 取到校验过的样本路径，由键盘进程自己的 `SoundPool`（`USAGE_ASSISTANCE_SONIFICATION`）按键类播放，音量取 `plugins.key_sound.volume`。`onStartInputView` 调 `ImeKeyFeedback.prepare()`，让包在第一下按键前开始加载。样本还没加载好或加载失败时才退回系统的 `FX_KEYPRESS_STANDARD`，作为兜底而不是默认声音。

HarmonyOS 手机和平板：`KeySoundPlayer` 按 `KeySoundPolicy.touchSettings` 配置，开关是本机 `key-feedback.json` 的 `sound`，包和音量取共享的 `plugins.key_sound`，没选包时是内置 `default`。触屏只播按键音，旋律、上屏音、成就音和连击音仍只在 2in1 上。视图里每个软键按下时调的统计入口改为 `KeyboardSession.softKeyPress`，它记入按键统计，再按 `KeySoundPolicy.touchKeyClass` 播放空格、回车、删除或普通键的样本，所以有统计 id 的软键都会出声，按住删除连删只响一次。按键反馈文件重读或保存后立即重新配置播放器，切换开关当场生效。2in1 的实体键仍走 `hardwareKeySound`。

iOS：继续用 `playInputClick`，这是系统给第三方键盘的按键音接口。没有完全访问时，在键盘功能面板打开按键音会当场提示需要开启「允许完全访问」；App 设置页和共享设置组件的说明写明它依赖完全访问、系统键盘声音和静音模式。

三端的密码框规则一致：不同键的不同声音会泄露按了哪些键，安卓 `default` 包与鸿蒙触屏统一播普通键的样本（与 iOS 系统按键声的单一声音一样），安卓的其他包维持不播。

## Alternatives considered

- 安卓保留系统按键音，只在检测到系统音效不可用时改播样本：保持原来的系统按键声音色，老用户听到的声音不变。但「不可用」查不出来：`playSoundEffect` 没有返回值，音效被卸载、被厂商屏蔽时音频服务照样记录一次播放，应用无从判断有没有出声。
- 鸿蒙触屏按键音挂在 `tapFeedback` 上，与振动同一处：改动最小。但 `tapFeedback` 只在组字的字母键、九宫格和滑行上触发，空格、删除、回车、符号键和不组字的输入框都不经过它，声音会只在一部分键上响。`recordKey` 的调用点正是每个软键按下的一次，按键统计已经靠它覆盖全部软键。
- 鸿蒙触屏用遵守静音模式的音频用途：手机静音时按键不该出声，安卓和 iOS 都是这样。DevEco 公开 SDK（API 21）的 `StreamUsage` 里没有系统音用途；通知、铃声类用途各有自己的音频焦点策略，有可能在每次按键时打断或压低正在播放的音乐，没有真机验证前不采用。触屏沿用 2in1 已有的 `STREAM_USAGE_MUSIC`，由媒体音量控制，设置说明照实写「按键音跟随媒体音量」。
- iOS 改用自己的样本（`AVAudioPlayer` 或系统声音 id）播放「清脆键盘」：音色与其他平台统一。但在键盘扩展里播放音频同样需要完全访问，解决不了没有权限时无声的问题，还会绕开用户在系统里关掉的键盘声音。

## Consequences

安卓默认按键音的音色从系统按键声变成「清脆键盘」，不再受系统触摸提示音开关、`unloadSoundEffects()` 和厂商对系统音效的屏蔽影响；`USAGE_ASSISTANCE_SONIFICATION` 仍映射到系统流，系统流音量和静音、振动模式照样作用于按键音。代价是每个键盘进程常驻一个装着四个短样本的 `SoundPool`，服务启动后第一次显示（或换包）时多一次后台的声音包校验。服务刚启动、包还在加载时按下的头一两个键仍走系统按键音兜底，在系统音效不可用的设备上这一两下可能无声。

鸿蒙手机和平板第一次真正有按键音，出厂仍是关（`KeyboardFeedback.DEFAULTS.sound`），没有改默认值。它跟随媒体音量，静音模式下照样出声，这是与安卓、iOS 不同的地方。开关打开后才创建 `SoundPool` 并加载包，关着时没有额外开销。

iOS 行为不变，只是没有完全访问时用户能知道为什么没有声音。

## Verification

- 安卓 `check-host.sh`：272 项 JVM 冒烟与 241 个设备源码编译通过。
- 安卓 `OptionSheetDeviceSmoke -e key-feedback sound-only` 改为先关掉系统触摸提示音并 `unloadSoundEffects()`，再断言默认包已加载、音频服务里属于本进程的 `SoundPool` 播放器出现开始播放记录，应用开关关闭时没有新播放；API 28 把这条记录写成 `state:started`，API 35 写成 `event:started`，播放器列表格式也不同，测试两种都认。API 35 模拟器、开发签名的完整 APK 上修复后 `sound-only` 与 `-e key-feedback true` 通过；换回 develop 上的 `ImeKeyFeedback` 与 `MSIMEInputService` 时确定失败于「Default key sound pack did not load」。
- 鸿蒙 `tests/run.sh`：新增 `touchSettings` 与 `touchKeyClass` 的检查（触屏开关独立于 2in1 插件开关、取共享包和音量但不开插件效果、按键 id 分类、密码框统一），全部通过。
- 共享设置组件：`mobile-keyboard-feedback-section` 与 `mobile-keyboard-feedback-settings-copy` 的 vitest 通过。
- 没有覆盖：安卓 API 28 没跑（编译机没有 API 28 镜像）；安卓与鸿蒙密码框统一播普通键样本没有设备断言；鸿蒙触屏出声、iOS 无完全访问时的提示都没有在设备上验证；三端真机的音量与音色需要装机确认。
