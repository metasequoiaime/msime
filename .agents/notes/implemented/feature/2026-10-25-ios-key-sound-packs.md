# Agent Note: iOS 键盘播放可选的按键音效包

Status: implemented

## Problem

安卓和鸿蒙手机已经按共享偏好 `plugins.key_sound.pack` 播放内置的按键音效包（「清脆键盘」「8 位游戏机」「气泡」「打字机」「木鱼」），iOS 键盘却只有系统按键声：`KeyboardViewController.playInputClick()` 调 `UIDevice.playInputClick()`，选什么包都听不出区别，设置 App 里也没有选包的地方。[移动端按键音](../bug-fix/2026-10-25-mobile-key-sound.md)那一次为 iOS 选择继续用系统按键声，理由是自己播放样本同样要完全访问、还会绕开系统的键盘声音开关；这次产品上要的是三个手机端都能选音效，所以 iOS 的那一部分决定翻转，那篇笔记里安卓和鸿蒙的部分不变。

## Decision

**打包位置。** `stage-resources.sh` 把 `resources/sound-packs` 里 `kind = "sound"`、`mode = "keys"` 的包整目录（样本连同写着许可的 `plugin.toml`）暂存到 `target/ios/sound-packs`，`project.yml` 把它作为文件夹放进键盘扩展和键盘测试宿主，缺了 `default` 就让暂存失败。设置 App 自己不带，`KeySoundPackPolicy.bundledDirectory` 先找本 bundle 的 `sound-packs/`，再找内嵌的 `PlugIns/*.appex/sound-packs/`，与 App 读键盘扩展里的 `EngineResources` 和词书是同一个办法。旋律包和音乐包不打包。

**键盘播放。** `KeyboardSoundPlayer` 在后台队列上经 `msime_client_key_sound_pack`（`state_root` 是 App Group 的共享状态目录，`sound_packs` 是上面的目录）取校验过的样本路径，每个样本建 3 只 `AVAudioPlayer` 并 `prepareToPlay`，按键时主线程只挑一只从头重放。音频会话设为 `.ambient`：与别的应用正在放的音乐混音、不打断它，静音开关打开时不响。键盘每次出现、每次重读共享偏好、在功能面板切换按键音时调 `synchronizeKeySoundPack`，同一个包不重新加载；内存警告时放掉播放器，下次出现再建。

**哪一下播什么**，由不碰音频的 `KeySoundPackPolicy` 决定，单元测试直接测它：

- 按键类别是空格、回车、退格和其他键，与安卓 `ImeKeyFeedback`、鸿蒙 `KeySoundPolicy` 相同；`handleBackspace`、按住退格的两条路径和九键面板删笔画是退格，`handleSpace` 和长按空格说话是空格，`handleReturn` 是回车，其余调用点都是普通键。
- 空格、回车、退格没有自己的样本时用普通键的。
- 密码框（`isCredentialField`：secure 或密码、一次性验证码类型）里每个键都播普通键的样本，与鸿蒙触屏一致。
- 只在按键音开着**而且**有完全访问时加载包（`wantedPack`）。没有完全访问、包还在加载、加载失败、或刚换包还没加载好时退回 `playInputClick()`。没有完全访问时开启按键音仍当场提示「按键音需要开启键盘的“允许完全访问”。」
- 音量取 `plugins.key_sound.volume`，只认 0–100 的整数，其余按 client-core 的默认 50，按振幅线性缩放，与鸿蒙 `AudioVolumePolicy.gain` 相同。

**设置 App。** 键盘页「按键反馈」组在「按键音」下面加一行「音效」，显示当前包名，按键音关着时置灰不可点。列表是 `msime_client_plugins` 的 `catalog` 里 `kind = "sound"`、`mode = "keys"` 的包，按目录顺序（client-services 把内置包排在前面）；选中的包本机没有（例如从桌面同步来的旋律包）时在末尾补一项「id（本机没有）」。选中后经 `updateSharedPreferences` 写 `plugins.key_sound.pack`，`key_sound` 里的其他键原样保留，并试听这个包的普通键样本。`MetasequoiaInputSessionBridge.plugins` 因此改为传随包的 `sound_packs`（以前是 null），辅助码页的目录只取 `kind = "helpcode"`，不受影响。

## 试听为什么用系统声音

App 里试听用 `AudioServicesCreateSystemSoundID` 加 `AudioServicesPlaySystemSoundWithCompletion`，不用 `AVAudioPlayer`。App 的音频会话属于替键盘录音的语音服务（`VoiceSessionService` 设成 `playAndRecord`），试听一下去改它的类别可能打断正在进行的录音；系统声音不碰会话，跟随静音开关，也和音乐混音。代价是试听音量是系统提示音的音量，不按 `plugins.key_sound.volume` 缩放。

## Alternatives considered

- 继续只用 `playInputClick`（上一篇笔记的选择）：系统按键声跟随「声音与触感 › 键盘反馈 › 声音」，用户在系统里关掉键盘声音就真的安静，也不必在扩展里管音频会话。但它只有一种声音，`plugins.key_sound.pack` 在 iOS 上形同虚设，三个手机端里只有 iOS 选不了音效；完全访问这个前提两条路都一样，换成自己播放并不多要权限。代价是系统的键盘声音开关不再管我们的音效，设置页的说明照实写出来，按键音开关仍在本 App 里。
- 键盘也用系统声音（`AudioServicesPlaySystemSound`）播样本：延迟最低，不用配会话。但它的音量是系统提示音音量，没法按 `plugins.key_sound.volume` 缩放，同一个包在 iOS 上的响度会和其他平台对不上；快速连打时同一个声音 id 也不能叠放。
- `AVAudioEngine` 加几个 `AVAudioPlayerNode` 预先排好缓冲：延迟比 `AVAudioPlayer` 更稳。但要自己管引擎的启动、配置变化和中断恢复，键盘扩展随时被挂起，出错面更大；四个不到一百毫秒的样本用预热过的 `AVAudioPlayer` 已经够用，等真机上测出明显延迟再换。
- 包放进 App Group 容器，由 App 首次启动时解包，两边读同一份：只存一份。但扩展可能在 App 从没打开过时就被用上（系统设置里直接加键盘），那时目录还不存在；放在键盘扩展的 bundle 里则随安装就绪、随升级替换，App 读内嵌扩展的目录已有先例。

## Consequences

开了完全访问的 iOS 用户第一次能听到所选的音效包；默认包是「清脆键盘」，所以默认的按键声从系统按键声变成「清脆键盘」，与安卓上一次的改变相同，按键音开关和出厂默认（开）不变。新声音跟随静音开关和媒体音量，不再跟随系统「键盘反馈 › 声音」，设置页的说明已改。键盘扩展多了约 400 KB 的样本，播放时常驻十几只 `AVAudioPlayer`；键盘出现后第一次（或换包后）包还在后台加载的那一两下按键是系统按键声。

host-api 的宿主能力 `key_sound`（`crates/client-core/src/host_surface.rs`）这次没有给 iOS 打开：它管的是共享 React 设置页给不给那组开关，iOS 的设置是原生的，打开它不改变 iOS 上的任何东西，而它的说明里的旋律、上屏音和成就音 iOS 都没有接。

## Verification

- 编译机上 `stage-resources.sh` 暂存出 `default`、`msime-8bit`、`msime-bubble`、`msime-typewriter`、`msime-woodblock` 五个目录，`xcodegen generate` 后 `xcodebuild build-for-testing`（`MSIMEClientTests` 与 `MSIMEClientUITests` 两个 scheme）通过。
- `KeyboardTests/settings/KeySoundPackPolicyTests.swift` 覆盖选包与音量的解析、写回时保留其他键、目录过滤与「本机没有」、样本退回与密码框统一、播放退回系统按键声的各个条件，另有三项对着测试宿主随包的 `sound-packs/` 跑真的 `msime_client_plugins` 和 `msime_client_key_sound_pack`，以及播放器只加载一次、失败后重试。编译机上没有可用的模拟器，这些用例这次只编译、没有运行。
- 没有覆盖：真机上的出声、延迟、静音开关与混音，没有完全访问时的退回，设置页的试听，都需要装机确认。
