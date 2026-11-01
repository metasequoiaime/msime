# Agent Note: 鸿蒙与 iOS 的本地识别停顿时间和判停灵敏度

Status: implemented

## Problem

#10310 要求点按开始（工具栏、功能菜单）的语音输入可以调「判停时间」和「判停灵敏度」，报告者截图里的灵敏度写的是「越高越容易停录」。Android 的 #10363 只做了 Android，灵敏度还是反方向的「本地识别灵敏度」：数值越高 VAD 阈值越低，越容易把声音当作说话，录音反而越难停。鸿蒙的停顿固定 1500 毫秒，而且对所有带中间文字的识别器都按停顿结束，包括豆包流式，这正是 Android 为 #9886 去掉的行为（整句流式说话期间要到 15 秒才回第一段文字，按出字后停顿计时会在用户还在说时掐断长句）。iOS 键盘没有任何按停顿结束的逻辑，点按开始的听写只能点「说完了」。移动端语音设置的改动要三端一起上线。

## Decision

### 语义

滑块叫「判停灵敏度」，10–90、默认 50，越高越容易判定说完、结束录音。宿主把它换成本地整段模型的 Silero VAD 判定阈值：阈值 = 灵敏度 ÷ 100。默认 50 正是原来写死的 0.5，所以不动设置的用户行为不变。「本地停顿时间」500–5000 毫秒、步进 100、默认 1500，与 Android 本机设置同一范围。两项都只作用于点按开始、由停顿结束的本地识别：按住空格或快捷键的录音由松手结束，阈值保持 0.5；系统识别和云端服务不读它们。判停灵敏度只调 Silero VAD，所以只对带语音活动检测的本地整段模型生效，流式本地模型（两端都不给它建 VAD）不受影响，设置页的说明写明这一点，与 #10363 的说明一致。

### 存储

鸿蒙和 iOS 都用共享文档的 `voice_input.pause_millis`、`voice_input.stop_sensitivity`（client-core 的 `VoiceInputPreferences`，默认值不写进文档，范围外或不在步进上的值以 `PreferencesError::InvalidVoicePause` 拒绝）。它们只留在本机：设置文件导出整组去掉 `voice_input`（`settings_document::LOCAL_SECTIONS`），账号同步是逐键导出的白名单，鸿蒙和 iOS 的导出都不含这两项，各有测试钉住导出不随它们变化。Android 仍用 #10363 的 `AndroidLocalSettings`，不读这两个字段。

### 鸿蒙

共享设置页按新的可选能力位 `voice_pause_settings` 在手机语音页「识别」组末尾给出两条滑块和一段说明。只有鸿蒙的设置桥报告它，且只在手机和平板上报告（`SettingsFormFactorCapabilities.voicePauseSettings`）；2in1 的语音面板不按停顿结束，iOS 和 Android 不用共享设置页。能力位只在 TypeScript 的 `HostCapabilities` 上，与 `touch_keyboard_split` 一样由设置桥按设备形态填写，不进 client-core 的 `HostCapabilities::for_platform`。

键盘在 `startVoice` 时固定本次是否由键盘按停顿结束和等多久：触屏上的本地识别和系统识别（`VoiceRecordingBehaviourPolicy.keyboardStopsOnPause`；系统识别包括出厂时一项凭据都没填、实际走系统识别的豆包，`usesSystemRecognizer`）。本地识别等「本地停顿时间」，系统识别沿用固定的 1500 毫秒（`VoiceRecordingBehaviourPolicy.pauseMillis`）。豆包流式和上传式服务不再由键盘的停顿计时结束。系统识别保留键盘计时，是因为自 #5792 起它就是触屏上点按开始的系统识别唯一确定的结束方式：`HarmonyVoiceRecognizer` 用写音频模式加 short 模式，不传 `vadEnd`，仓库里没有任何记录说明 Core Speech Kit 在这种模式下会在停顿后自己结束会话，句子累加器反而说明它可能在 `isLast` 之前返回多句 `isFinal`；出厂配置走的正是这条路，在真机确认之前去掉计时可能让默认用户的录音一直停不下来。聆听面板的提示按 `endsOnPause` 区分：本地和系统识别提示停顿后自动上屏，填了凭据的豆包和上传式服务提示点「说完了」。判停灵敏度经宿主注入的 `local_stop_sensitivity`（`VoiceRecordingBehaviourPolicy.stopSensitivity`，只给点按开始的本地识别）进 `LocalAsrRequest.stopSensitivity`，worker 用 `LocalAsrPolicy.vadThreshold` 换算后建 `SileroVadConfig`；鸿蒙本地识别用的是 sherpa-onnx 的 ArkTS 接口，不经过共享 C++ 的 `LocalAsrOptions`。

### iOS

语音页「识别」分组末尾加两条滑块，位置与 Android、鸿蒙相同，标题和说明与鸿蒙逐字相同，分组说明写适用范围，值存在共享文档（`VoicePauseSettings`）。键盘在开始命令里带上 `tapStarted`（`VoiceSessionCommand` 的可选字段，旧键盘写的命令没有它，按按住处理）。主 App 的 `VoiceSessionService` 对点按开始、用本地模型识别的听写（`VoicePauseSettings.forDictation`）建一个 `VoicePauseStop`：出字后开始计时、每次出新字从头计，VAD 还判定在说话时到点顺延，说话状态变化时重新计，到点后像到时长上限一样停止录音，识别器读完剩下的音频交出结果，键盘照常收到 `recognizing` 和 `finished`。建好计时后 App 在聆听状态里带上 `endsOnPause`（`VoiceSessionState` 的可选字段，旧版本 App 写的状态没有它，按不会处理），键盘的聆听面板据此把提示换成与 Android、鸿蒙相同的「停顿后自动上屏，也可点「说完了」」。`LocalSpeechRecognizer.transcribe` 增加 `vadThreshold` 和 `speech` 两个参数，缺省时（按住说话、App 内录音）行为不变。系统识别、豆包和上传式服务在 iOS 上仍由「说完了」结束，所以 iOS 分组说明的第二句与鸿蒙不同。

### 与 Android #10363 的关系

Android #10363 已合入，但灵敏度方向与报告者要求相反。本 PR 同步修正共享 `LocalAsr` 的阈值映射、Android 设置页文案、设备专项夹具、README 和回归断言：三端统一为「数值越高越容易判定说完、结束录音」，默认 50 仍为原来的 0.5。停顿时间的说明也改成与 iOS、鸿蒙相同的「本地识别出字后停顿这么久就结束录音；按住说话时由松手结束」，Android 多一句「锁定录音时由发送或取消结束」：只有 Android 能上滑锁定录音，锁定后的录音由发送或取消结束。Android 现有的本机键名和范围不迁移，只改变 10–90 值进入 VAD 的方向；没有用户改设置时行为不变。

Android 的作用范围同时收窄到与鸿蒙、iOS 一致：判停灵敏度只给键盘内点按开始的本地识别。长按空格经 `ImeVoiceEntry.startHeld` 开始，创建会话时就传 0（阈值保持 0.5），不等 `hold` 设置 `held`，因为工作线程调用 `localSpeechStart` 时会读这个值，事后再改有竞态；松手那一刻才补判出的长按仍按点按开始，保留存下的灵敏度。`VoiceRecognitionActivity` 和 `OfflineVoiceRecognitionActivity` 的本地识别由「完成」结束、不按停顿判停，同样传 0。三端的真实麦克风质量验收仍待设备环境，不能由纯策略测试替代。

该设置还没有随任何发布出去，不需要迁移已存的值。

## Existing notes audit

- [鸿蒙手机语音转写完成后直接上屏](../bug-fix/2026-10-24-harmony-voice-direct-insert.md)：部分翻转。那篇写「流式识别保留停顿自动结束」，现在只有本地识别和系统识别由键盘按停顿结束，豆包流式改由用户结束；在那篇的结果一节加了指向本篇的说明。
- [Android 豆包流式双向接收](../bug-fix/2026-10-29-android-doubao-streaming-receive.md)：部分重叠，本篇把同一条「豆包不按停顿结束」的约束带到鸿蒙。
- [Android 与 iOS 的语音入口直接开始听写](../bug-fix/2026-10-24-mobile-voice-entry.md)：部分重叠，保留点按入口、「说完了」和「取消」，iOS 点按开始的本地识别多了停顿自动结束。
- [本地语音准确度](2026-10-17-local-voice-accuracy.md)：部分重叠，新参数只调 VAD 判定阈值，不改词表、热词、段首补偿和 `min_silence_duration`。

## Alternatives considered

- **沿用 #10363 的方向，只把文案改清楚。** 最强的理由是不用改 Android，共享 C++ 的 `vad_sensitivity_percent` 名实相符（越高 VAD 越灵敏）。不用它，是因为报告者要的就是「越高越容易停录」，用户调高这个值是想让录音早点结束，给一个方向相反的滑块只会让人以为设置坏了；维护者已经定了按报告者的语义。
- **鸿蒙和 iOS 各用本机存储，与 Android 的 `AndroidLocalSettings` 对齐。** 好处是三端存法一样，不碰 client-core。不用它，是因为鸿蒙的设置页就是共享 React 页，它只读写共享文档，另起一套本机存储要新加设置桥的读写通道；共享文档的 `voice_input` 本来就不进导出、账号同步又是白名单，放在共享文档里同样只留本机，范围还能由 client-core 统一校验。
- **判停灵敏度作用于所有本地识别，包括按住说话。** 好处是一个设置处处生效，鸿蒙的快捷键按住后 LOCK 改为点按对待的会话也能用上。不用它，是因为按住说话由松手结束，阈值改高只会让轻声的字被当成停顿丢掉，换不来任何好处；维护者对 iOS 也定了按住说话不变。LOCK 的会话因此沿用 0.5 的阈值，只有停顿时间生效。
- **鸿蒙的系统识别也交给 Core Speech Kit 自己判停，与 Android 由系统识别服务判停对齐。** 好处是键盘只给本地识别计时，规则最简单，系统识别也不会被 1500 毫秒的停顿切掉还没说完的话。不用它，是因为写音频加 short 模式会不会自己结束会话从没在真机上确认过，而这是出厂配置的路径；猜错的代价是默认用户点按开始的录音停不下来，保留计时的代价只是维持现状。真机确认它会自己结束之后可以再去掉。
- **鸿蒙保留豆包流式的停顿结束，只把停顿时间做成可调。** 好处是鸿蒙用户的手感不变。不用它，是因为说明里写「只用于本地识别」在鸿蒙上就不成立了，而且长句会在用户还在说时被掐断（#9886），Android 已经因此去掉了这条路。

## Consequences

- **收益**：三端对点按开始的本地识别给出同一组设置、同一个方向和同一个默认值；鸿蒙不再在豆包流式中途掐断长句；iOS 的点按听写用本地模型时停顿后自己结束。
- **代价与已知上限**：鸿蒙用豆包流式的用户以前停顿后自动结束，现在要点「说完了」。鸿蒙系统识别仍按固定 1500 毫秒的停顿结束，不跟「本地停顿时间」走。iOS 键盘的聆听面板在 App 报来 `endsOnPause` 之前（本地模型加载完之前）显示的仍是「说完后点「说完了」」。判停灵敏度调到 90 时阈值 0.9，轻声说话可能整段不被当作语音、一直不出字；调到 10 时噪声容易被当作说话，录音可能要等时长上限才结束。两端都没有用真实麦克风和本地模型验证识别质量和极端值。

## Verification

- client-core：`voice_pause_settings_default_to_the_old_fixed_values_and_follow_the_android_bounds` 钉住默认值、不写进文档、范围和步进；`settings_document` 的往返用例钉住导出不含这两项、导入保留本机值。client-services：鸿蒙与 iOS 各有 `voice_pause_settings_never_reach_the_account`。
- 鸿蒙：`platforms/harmony/tests/run.sh` 的 `keyboard-logic.test.ts` 覆盖 `pauseMillis`、`vadThreshold`、`keyboardStopsOnPause`（本地、系统、没填凭据的豆包由键盘计时，填了凭据的豆包和上传式服务不计时）、`endsOnPause`、`stopSensitivity` 和能力位；同一脚本用 grep 钉住视图、识别器、worker 和设置桥的接线。
- 共享设置页：`apps/desktop/tests/settings/harmony-voice-pause.test.tsx` 覆盖能力位、2in1 不显示、缺键时的默认值与范围、保存和读回。
- iOS：`ServiceTests/voice/VoicePauseTests.swift` 覆盖读法、阈值换算、`forDictation`（只有点按开始的听写有停顿设置）和停顿计时；`KeyboardTests/voice/VoiceSessionTests.swift` 覆盖开始命令的 `tapStarted` 和旧命令的兼容、`endsOnPause` 只报一次、App 自己结束录音后键盘照常上屏，以及聆听面板只在点按开始时换成停顿提示。
- Android：`scripts/test-android-local-voice-pause-wiring.py` 钉住键盘内本地识别按 `startingHeld` 选 0 或存下的灵敏度、长按空格经 `startHeld` 开始，以及两个识别窗口传 0。
- macOS 偏好覆盖、偏好字段对照和参考配置覆盖脚本通过。
