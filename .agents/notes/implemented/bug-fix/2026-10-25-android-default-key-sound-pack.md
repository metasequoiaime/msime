# Agent Note: 安卓默认按键音播放内置清脆键盘包

Status: implemented

## Problem

安卓按键音开着却没有声音。偏好 `plugins.key_sound.pack` 没选或选 `default` 时，`ImeKeyFeedback.playSound` 不播 APK 里打包的 `default` 包（「清脆键盘」），而是调用系统的 `AudioManager.playSoundEffect(FX_KEYPRESS_STANDARD, volume)`。[九键反馈](2026-10-23-android-nine-key-feedback.md)把它从单参数接口换成双参数接口，绕过了系统触摸提示音开关的检查，但系统按键音本身仍由系统管：它走 `STREAM_SYSTEM`，系统流音量为 0 或处于静音、振动模式时不出声；系统设置页关掉「触摸提示音」时会调 `unloadSoundEffects()`，之后音频服务照样收下播放请求却没有样本可播；部分厂商系统还会另外屏蔽这一路。那次的设备测试在断言前主动调了 `loadSoundEffects()`，正好把卸载这种情形排除在外，所以模拟器上通过而真机仍然无声。

默认值是 `default`，所以几乎所有安卓用户走的都是这条路。AI 皮肤页也把 `default` 标成「清脆」，HarmonyOS 与桌面端对 `default` 都播这个包的样本，只有安卓播的是系统按键声。

## Decision

`default` 和其他声音包走同一条路：经 `NativeClient.keySoundPack` 取到校验过的样本路径，由键盘进程自己的 `SoundPool`（`USAGE_ASSISTANCE_SONIFICATION`）按键类播放，音量取 `plugins.key_sound.volume`。键盘开始显示时 `onStartInputView` 调 `ImeKeyFeedback.prepare()`，让包在第一下按键前就开始加载。

样本还没加载好或加载失败时才退回系统的 `FX_KEYPRESS_STANDARD`，作为兜底而不是默认声音。

密码框里不同键的不同声音会泄露按了哪些键：`default` 包统一播普通键的样本，与原先系统按键声的单一声音一样；其他包维持原来的不播。

应用本地的按键音开关（`KeyboardFeedbackStore.soundEnabled`）关闭时两条路都不播，不变。

## Alternatives considered

- 保留系统按键音，只在检测到系统音效不可用时改播样本：保持安卓原来的系统按键声音色，老用户听到的声音不变。但「不可用」查不出来：`playSoundEffect` 没有返回值，音效被卸载、系统流被厂商静音时音频服务照样记录一次播放，应用无从判断有没有出声，只能对所有人改播样本。
- 用 `STREAM_SYSTEM` 之外的流播系统按键声：系统按键声的样本文件不对应用开放，换流也拿不到它。

## Consequences

安卓默认按键音的音色从系统按键声变成「清脆键盘」，与 HarmonyOS 和桌面端、与 AI 皮肤页「清脆」的标注一致；不再受系统触摸提示音开关、`unloadSoundEffects()` 和厂商对系统音效的屏蔽影响。`USAGE_ASSISTANCE_SONIFICATION` 仍映射到系统流，所以系统流音量和静音、振动模式照样作用于按键音，这是有意保留的：手机调成静音时键盘不该出声。代价是每个键盘进程常驻一个装着四个短样本的 `SoundPool`，键盘服务启动后第一次显示（或换包）时多一次后台的声音包校验调用。

服务刚启动、包还在加载时按下的头一两个键仍走系统按键音兜底，在系统音效不可用的设备上这一两下可能无声。

## Verification

- `ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`：272 项 JVM 冒烟与 241 个设备源码编译通过。
- `OptionSheetDeviceSmoke -e key-feedback sound-only` 改为先关掉系统触摸提示音并 `unloadSoundEffects()`，复现真机上系统按键音不出声的条件，再断言默认包已加载、音频服务里属于本进程的 `SoundPool` 播放器出现开始播放记录，应用开关关闭时没有新播放。API 28 把这条记录写成 `state:started`，API 35 写成 `event:started`，播放器列表的格式也不同，测试两种都认。
- API 35 模拟器、开发签名的完整 APK：修复后 `-e key-feedback sound-only` 与 `-e key-feedback true` 通过；同一测试换回 develop 上的 `ImeKeyFeedback` 与 `MSIMEInputService` 时确定失败于「Default key sound pack did not load」。
- 本次没有在 API 28 上跑：编译机没有 API 28 的系统镜像，本机空间不够再起一台模拟器。
- 新版系统只在播放器状态变化时记录，同一个 `SoundPool` 第二次播放不留记录，所以密码框里统一播普通键样本这一支没有设备断言，只经代码审读；真机上的实际音量与音色同样需要装机确认。
