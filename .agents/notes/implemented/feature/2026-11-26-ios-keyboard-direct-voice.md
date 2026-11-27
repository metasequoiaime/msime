# Agent Note: iOS 联网语音服务在键盘里直接录音，不再跳到 App

Status: implemented

## Problem

iOS 的语音由主 App 在后台语音会话里录音识别（[按住空格说话](2026-10-21-ios-hold-to-talk-voice-session.md)），会话不在时长按空格要跳到 App、再点「◀︎ 返回」。为了[低端机的发热和耗电](../bug-fix/2026-10-27-ios-keyboard-low-end-power.md)，会话空闲 60 秒就关，所以隔一分钟没说话，下一次长按空格又要跳一次。选了说完整段上传的联网服务（OpenAI 兼容的转写接口、百炼）时，App 在这条路上只负责录音和转发，每次跳转换来的只是麦克风。

## Decision

App 发布了说完整段上传的联网服务时，键盘自己录音、自己上传，不打开 App。

- **录音**：`KeyboardMicrophone` 在一个隐藏的 `WKWebView` 里跑 `getUserMedia`。键盘扩展自己的音频输入用不了，WebKit 在它自己的进程里、按主 App 的麦克风权限采集，网页降到 16 kHz 单声道 16 位再交给原生一侧。这个网页只采集声音，不显示界面，不加载远程内容（`loadHTMLString`，基础地址只为拿到 `getUserMedia` 要求的安全上下文）；原生一侧每秒发一次心跳，心跳断了网页自己关麦克风。键盘扩展的 Info.plist 带上麦克风用途说明。
- **发布**：`KeyboardVoicePublisher` 在 App 回到前台、保存语音服务、删除密钥和改语音设置后，把服务地址、模型、请求格式、换算好的 `language` 和提示音写进 App Group（`KeyboardVoiceService`），密钥写进以 App Group 为访问组的钥匙串，与键盘 AI 的 `KeyboardAIService` 同一做法。本地模型、系统识别、豆包，以及打开了识别后润色时撤下发布。
- **听写**：`KeyboardDirectDictation` 报的事件与 `KeyboardVoiceDictation` 相同（两者都实现 `KeyboardDictation`），聆听面板、上屏、语音标点和语音交接不分来源。请求格式与 App 的 `CustomServiceClient.makeRequest` 相同，WAV 封装移到两边共用的 `AppServicesBridge.wav`。一次最长 60 秒，提示音和语音时长统计与 App 一致。
- **先后**：App 的后台会话开着时仍交给 App，行为不变；会话不在时有发布的服务就在键盘里录，都没有才打开 App。麦克风一开始就打不开（主 App 还没拿到过麦克风权限等）时报 `reopensSession`，退回打开 App 的老路，由 App 请求权限。

## Alternatives considered

- **延长后台会话的空闲时限**：改动最小，跳转次数会变少。但跳转仍在，而且音频引擎一直开着的发热和耗电，正是当初把时限从 300 秒降到 60 秒的原因。
- **键盘扩展自己用 AVAudioSession / AVAudioEngine 录音**：最直接，也是[按住空格说话](2026-10-21-ios-hold-to-talk-voice-session.md)里判断做不到的那条路：扩展里拿不到输入，或者被宿主打断。WebKit 的采集不走扩展自己的音频会话，所以改走 WebKit。
- **本地模型和润色也搬进键盘**：本地模型装不进键盘扩展的内存；润色要用 App 钥匙串里的润色密钥，再发布一份就会把 AI 和润色两套密钥都放进键盘。这次不做，这两种情况整条听写留在 App。

## Consequences

- **收益**：选了联网整段上传服务时，长按空格不再跳转；状态栏的麦克风标记只在说话时出现，不再在会话期间一直亮着。
- **代价与已知上限**：键盘里多了一个只采集声音的 WebView。[设置界面按平台归属](../architecture/2026-10-15-settings-ui-ownership-by-platform.md)「iOS 没有 WebView」的核对命令排除了 `KeyboardMicrophone.swift`；它不在按键路径上，也不画任何界面，[全部迁到原生](../../proposed/architecture/2026-11-03-all-native-ui.md)说的「按键路径上没有 WebView」仍然成立。录音时网页的音频输出一直运行（音量为零），灵动岛会出现一个播放标记。WebKit 能在键盘扩展里采集是现在的系统行为，系统不再允许时键盘自动退回打开 App 录音，不比改动前差；到那时要重访这个决定。

## Verification

- `KeyboardTests/voice/KeyboardDirectDictationTests.swift`：麦克风打开前不报在听，整段录音的 WAV 交给服务；打不开麦克风退回 App；松手早于麦克风打开时交回空结果；中途被打断时照样识别已经录到的；服务出错不重开 App；取消以后不再报事件。
- `ServiceTests/voice/KeyboardVoicePublisherTests.swift`：只发布说完整段上传的联网服务；打开润色时不发布；App 不接受的地址不发布；发布的请求字段与 App 自己发的一致。
- iOS 27.0 模拟器（Xcode 27）：`MSIMEKeyboardTests` 的 `KeyboardDirectDictationTests`、`VoiceSessionTests`、`SpaceVoicePanelTests`、`VoicePunctuationTests`、`KeyboardServiceFallbackTests`、`VoiceTextHandoffTests`、`KeyboardAppLauncherTests` 共 64 项，`MSIMEServiceTests` 的 `KeyboardVoicePublisherTests`、`CustomServiceTests`、`VoicePolishTests`、`VoiceLevelTests` 共 52 项，`MSIMESharedTests` 的 `VoiceTextHandoffTests`、`CustomServiceTests` 共 15 项，全部通过，0 失败。
- 真机（iOS 27.2）：用 OpenAI 兼容的自建转写服务验收。第一次长按空格因为主 App 还没拿到麦克风权限，跳到 App 一次；之后在键盘里录音上屏，不再跳转，服务端记录的请求都来自键盘扩展。
- `python3 platforms/ios/tests/settings/ProjectConfigurationTests.py`：35 项通过。
