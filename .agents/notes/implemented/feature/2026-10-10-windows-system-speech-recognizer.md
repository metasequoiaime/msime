# Agent Note: Windows 系统识别用 SAPI 进程内听写，读本次录音的音频

Status: implemented

## Problem

macOS 的语音识别服务里有「macOS 系统识别」（`asr_provider = "system"`）：不需要 API Key，不用下载模型，音频不出设备（`platforms/macos/src/voice/VoiceInputService.mm` 的 `startTranscriptionWithLanguage:`，macOS 26 起走 SpeechAnalyzer，更早要求 SFSpeechRecognizer 设备端识别）。Windows 的 `VoiceInputSession` 只认豆包、本地模型和云端 HTTP 三类；存着 `system` 的配置会掉进云端分支，提示去填 API Token。共享设置页的 `nativeVoicePlatform` 不含 Windows，下拉里只显示「系统识别（当前平台不可用）」。Windows 上唯一的离线选项是本地 sherpa-onnx 模型，得先下载几百 MB。

## Decision

识别服务新增「Windows 系统识别」，实现是 SAPI 5.4 的进程内识别器（`CLSID_SpInprocRecognizer`）加听写语法（`ISpRecoGrammar::LoadDictation`），代码在 `platforms/windows/src/voice/SystemAsrStream.cpp`，不需要 Win32 的判定在 `SystemAsrPolicy.h`。

- **音频来源**：识别器不自己开麦克风。`QueueAudioStream` 实现 `ISpStreamFormat`（16 kHz 单声道 16 位 PCM），从这次录音的 `LocalAsrAudioQueue` 读；采集回调把浮点采样推进队列，识别器线程在 `Read` 里阻塞到凑够请求的字节或队列关闭，读到的比请求的少就是流结束。所选录音设备、电平、静音其他声音、录音中断的判定都和其他识别服务走同一条采集路径。
- **流式**：和本地模型共用 `OnDeviceAsrStream` 接口（`push` / `finish` / `cancel`），`VoiceInputSession` 里本地模型和系统识别都放在 `local_stream_` 这一格，分派只在 `start()` 建对象、起识别任务，以及 `finish()` 选失败文案这两处区分。`SPEI_HYPOTHESIS` 的中间结果拼在已确认的句子后面，经 `show_partial` 送到内联预编辑或浮层；`SPEI_RECOGNITION` 追加到已确认文字；`SPEI_END_SR_STREAM` 结束。录音停止后最多等 `system_asr_drain_timeout`（20 秒）让识别器发出流结束，超时就用已有文字上屏。之后的润色、上屏、打字统计与其他服务相同。
- **语言**：`system_asr_language_id` 把 `voice_input.language` 换成识别器令牌的 `Language` 属性（十六进制 LANGID，zh-CN → `804`、zh-TW → `404`、en-US → `409`、ja → `411` 等）；空值和 `auto` 按 zh-CN，和 macOS 没给语言时一样。粤语一律拒绝：`yue`、`zh-yue` 以及地区子标签是 `HK` / `MO` 的 `zh` 标签（共享设置把 `zh-HK` 显示为粤语）都返回空，开始录音前提示不支持，不悄悄交给台湾普通话或大陆普通话识别器。同一语言有多个识别器时优先 `Vendor=Microsoft`。
- **缺识别器**：`start()` 在开始录音前调 `system_asr_start_problem`，只读注册表里的识别器令牌，不加载引擎。语言不在表里（粤语、韩语）、这台电脑没装该语言的语音识别、SAPI 组件不可用各有一句中文提示，显示在浮层上，录音不开始。听写语法加载失败也按「没装语音识别」说明。其余失败显示通用的「语音识别失败」。注册表里有识别器令牌、真正建识别器时才失败（`LoadDictation`、`SetRecognizer`、`SetInput`、`SetRecoState`）的，识别任务把异常存下并关掉音频队列；`OnDeviceAsrStream::failure()` 不阻塞地交出它，`maintain()` 在录音中每轮查一次，查到就 `cancel_session(true)` 并立即在浮层报出原因，不让人锁定录音说完一整段才看到失败。本地模型的 `LocalAsrStream` 走同一条：模型加载或识别出错后同样关掉队列、在录音中报出 `voice_local_failure`，不再攒到队列溢出、被误报成录音中断。
- **不需要凭据**：`voice_start_verdict` 的 `system` 分支直接放行，不检查 Token、接口地址和模型名；设置页切到系统识别时 `asrProviderUpdate` 本来就清掉 Token 槽位。`finish()` 里系统识别有单独的分支：录音没有本机识别器接手时只报「语音识别失败」，不会落进后面的云端上传分支，所以「音频不出设备」不依赖采集回调一定拿到识别器这一前提。
- **GUID 与头文件**：接口声明用 `sapi.h`（MinGW 和 Windows SDK 都带）；类 id、接口 id 和 `SPDFID_WaveFormatEx` 在 `.cpp` 里直接写出，不链接 `sapi.lib` / `libsapi.a`，因为 MinGW 的 `libsapi.a` 没有 `SPDFID_WaveFormatEx`。`SPFEI` 和 `SP_GETWHOLEPHRASE` 在 MinGW 的 `sapi.h` 里没有，也在 `.cpp` 里算。

共享设置页：`settings-capabilities.ts` 的 `nativeVoicePlatform` 加上 Windows，识别服务下拉显示「Windows 系统识别」，`systemVoiceHostName` 为 `Windows`，介绍分组标题「Windows 系统语音」，正文改说本机识别和去「语言和区域」安装语音识别，不提 macOS 那句授予语音识别权限。WinUI 设置窗口不画识别服务（它跳到共享应用的语音页），不用改。

与 [Windows 原生语音对齐 macOS](2026-10-10-windows-native-voice-mac-parity.md) 互补：那篇管录音、焦点、投递和反馈，这篇只加一个识别服务，两者的上屏和失败提示路径相同。

## Alternatives considered

- **WinRT `Windows.Media.SpeechRecognition.SpeechRecognizer`（连续听写）** — 新一代引擎，识别质量和自动标点都比 SAPI 桌面识别器好，MinGW 也带 `windows.media.speechrecognition.h`，用原始 ABI 能编。不选它有三条硬原因：听写主题依赖「联机语音识别」隐私开关，音频送到微软的云端，做不到 macOS 那句「音频不出设备」，开关关着时直接报错；它只从默认麦克风录音，不接受外部音频流，所选录音设备、电平和审阅面板的录音都得另起一路；没有 C++/WinRT 时要手写 `IAsyncOperation` 完成回调和事件委托，Server 里没有别处这样做，出错面比 COM 的 SAPI 大得多。
- **SAPI 共享识别器（`SpSharedRecognizer`）** — 少写一个音频流；但它属于 Windows 语音识别这个系统进程，会弹出它的麦克风栏、用默认麦克风、和其他应用的语音命令共享状态，不适合输入法在后台听写。
- **录完再整段交给 SAPI（`SHCreateMemStream`）** — 不需要阻塞式的音频流，实现最稳；但说话时没有中间结果，浮层和内联预编辑要等录音结束才出字，与豆包、本地模型和 macOS 系统识别的手感都不一样。阻塞读取的实时流正是 System.Speech `SetInputToAudioStream` 接实时音频的做法，SAPI 支持它。
- **只在识别任务里发现缺语言包** — 少一次注册表查询；但人要先说完一整段才看到「没装语音识别」，浪费一次录音。开始前查令牌只读注册表，代价可以忽略。

## Consequences

- **收益**：Windows 有了不需要 Key、不用下载、音频不离开本机的识别服务，开箱即用；边说边出字；录音设备、静音、焦点核对、润色和统计全部沿用现有路径；Linux 等没有系统识别器的宿主仍把存下的 `system` 显示为不可用。
- **代价与已知上限**：SAPI 桌面听写的准确率不如云端服务和本地大模型，也不自动加标点（可以用润色补）。微软已经弃用 Windows 语音识别这个应用，较新的 Windows 11 上桌面识别器可能不随语言包安装，那时开始录音就会提示没装该语言的语音识别，需要改用其他服务。进程内识别器第一次使用默认的识别配置文件，没有做过语音训练。以上都没有在真机上验证：识别器令牌的 `Language` 属性匹配、阻塞流的 `Read` / `Seek` / `Stat` 是否满足当前 Windows 版本的 SAPI、中间结果的到达节奏、20 秒收尾上限是否够用。

## Verification

- `platforms/windows/tests/voice/system_asr_policy.cpp`（`windows-system-asr-policy`）：两种语言写法到 LANGID 的映射、`auto` 和空值按 zh-CN、粤语（`yue`、`zh-HK`、`zh-MO`、`zh-Hant-HK`、`zh-yue`）和韩语被拒、缺识别器和不支持语言的文案、`SystemAsrError` 与其他异常的文案、中英文逐句拼接、PCM 截断。在主机上用 clang++ 编译运行通过。
- `platforms/windows/tests/voice/voice_session_policy.cpp`：`system` 不需要任何凭据即放行，关掉语音时仍静默。主机上编译运行通过。
- `SystemAsrStream.cpp` 与 `VoiceInputSession.cpp` 用 `x86_64-w64-mingw32-g++` 和 `i686-w64-mingw32-g++` 以 `-std=c++17 -Wall -Wextra -Werror -fsyntax-only` 检查通过；MSVC 未编译。
- `apps/desktop/tests/settings/voice-input-core-section.test.tsx`、`voice-input-intro-section.test.tsx`：Windows 下拉出现「Windows 系统识别」且不显示不可用项，非原生宿主仍显示不可用；Windows 介绍写本机识别和语言包、不提语音识别权限。
