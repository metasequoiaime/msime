# Agent Note: 用系统语音合成朗读英文单词

Status: implemented

## Problem

候选旁已经能显示英文释义和音标，用户也能从候选菜单把释义收进[生词本](2026-11-01-personal-wordbook.md)，背单词卡片上也有词和音标，但哪里都听不到这个词怎么读。仓库里原本没有任何语音合成代码，六个平台的语音都只做识别。

要补的是「点一下，读出这个英文单词」，难处不在播放，而在不引入新东西：sherpa 的语音合成要换带 TTS 的运行库，声音模型是新的第三方数据；espeak-ng 是新的原生依赖。另一个难处是隐私：有的系统语音引擎会联网合成，候选和释义一旦被送出去，就和 PRIVACY.md 里「释义查本机词库、显示时不联网」的说法冲突。

## Decision

每个平台只用操作系统自带的语音合成，不打包任何声音模型，不加任何依赖；只读英文单词或短语，只在用户点了「朗读」时出声。桌面三端和移动三端都接上了。

### 读哪段文字

共享层的 `client-services::vocabulary::read_aloud` 定一条规则，宿主经 C 入口 `msime_client_read_aloud_text`（`{candidate, gloss}` → 要读的文字或 `null`）取得，不各写一份：

- 释义按行（换行或 U+2028）拆开，每行取第一个义项（`;` 或 `；` 之前），第一个「是英文」的就是要读的文字，和「加入生词本」取词头的方式相同；
- 没有这样的释义时，候选本身是英文也读它（混输里的英文候选）；
- 「是英文」指去掉首尾空白后只含 ASCII 字母、空格、`-`、`'`、`.`，以字母开头，不超过 64 个字符（与生词本词头的上限相同）。日文、韩文的释义行和带括号、数字、中文的义项都不读，宁可不出这一项，也不让系统用别的语言的声音念一串怪东西。

背单词卡片读卡片上的词头。React 页面（`packages/ui/src/settings/read-aloud.ts`）为了决定画不画喇叭，在页面里按同一条规则判断一次，不为一个按钮去问宿主；Linux 设置应用真正出声前在 Rust 里按共享层的规则再核对一遍。

### 每个平台用什么

| 宿主 | 候选菜单 | 背单词卡片 |
|---|---|---|
| macOS | 输入法进程里的 `AVSpeechSynthesizer`（`InputController.mm`，AVFoundation 本来就链接着），先找 `en-US` 再找任何英文声音 | Tauri 页面（WKWebView）的 Web Speech `speechSynthesis` |
| Windows 新宿主 | Server 里的 SAPI 5 `ISpVoice`（`src/voice/SystemSpeech.cpp`），与系统听写（`SystemAsrStream.cpp`）同样在代码里写出类 id，不链接 `sapi.lib`；先找 `Language=409`、再找 `809` 的声音，都没有时用默认声音 | Tauri 页面（WebView2）的 `speechSynthesis` |
| Linux（IBus、Fcitx5） | 系统装了 speech-dispatcher 的命令行 `spd-say` 时，`posix_spawnp` 起 `spd-say -l en <词>`（`src/candidates/ReadAloud.h`），另起线程收掉子进程；`PATH` 上没有它时没有这一项 | 设置应用经 Tauri 命令 `read_aloud` 起同一个 `spd-say`（`src-tauri/src/read_aloud.rs`） |
| Android | IME 进程里的 `android.speech.tts.TextToSpeech`（`core/ReadAloudSpeaker.java`，挑声音的规则在 `policy/ReadAloudVoicePolicy.java`），只在装着语音引擎（`queryIntentServices(TTS_SERVICE)` 不为空）时有这一项 | 原生背单词页（`home/VocabularyReviewPage.java`），同一个类，各自一个实例 |
| iOS | 键盘扩展里的 `AVSpeechSynthesizer`（`SharedUI/vocabulary/ReadAloud.swift`），只在开了完全访问时有这一项 | App 的原生背单词页（`VocabularyReviewSettingsView`），同一个类 |
| HarmonyOS | 键盘里的 Core Speech Kit `textToSpeech`（`speech/HarmonyReadAloud.ets`），设备没有 `SystemCapability.AI.TextToSpeech` 时没有这一项 | 设置应用（ArkWeb 里的共享页面）经设置桥的 `read_aloud` 交给同一个模块，设置桥先经共享层核对词头 |

网页那条路（`apps/desktop/src/core/read-aloud.ts` 的 `speakWithLocalVoice`）只用 `SpeechSynthesisVoice.localService` 为真的声音，优先 `en` 开头的，没有本地英文声音时用系统默认的本地声音，总是显式指定声音，不让浏览器替我们挑；一个本地声音都没有时提示「系统没有可用的朗读语音」。第一次 `getVoices()` 可能还是空的，等一次 `voiceschanged`，最多一秒。

移动三端里，Android 的引擎第一次朗读时才连接（`onInit` 之前点的词连上后再读，只读最后点的那一个），键盘服务和背单词页销毁时断开；iOS 的合成器 `usesApplicationAudioSession` 为假，朗读的音频会话交给系统，不去改键盘按键音用的 `.ambient` 会话，键盘收起和页面消失时停下；鸿蒙每个进程一个引擎，用抢占模式（`queueMode` 1），文字前面加 `[h2]` 让引擎当成单词读，扩展销毁和设置页消失时关掉。鸿蒙和 iOS 的读不出来的提示显示在诊断行，Android 键盘用 `notice`，Android 背单词页用 toast。

Windows 的读是同步的：工作线程上初始化 COM、建声音、读完再释放，一个词不到一秒；它和「加入生词本」一样是单槽的 `SingleClickWorker`，正在读时多点的那几下丢掉。macOS 用输入法控制器上一直留着的那个合成器，新的一次先打断上一次。

### 不联网

逐个平台按官方文档核对过哪些接口可能联网，能强制本机的就强制：

- Android：系统允许装第三方引擎，引擎可以提供联网的声音。`Voice.isNetworkConnectionRequired()` 说明「这个声音是否需要网络」，`TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED` 说明「这个声音可能要下载额外数据」，而且下载由 `setVoice` 触发。所以只挑语言是英文、不需要网络、也没有 `KEY_FEATURE_NOT_INSTALLED` 的声音，用 `setVoice` 选定它再读；一个都没有时不读，提示「系统没有可离线使用的英文朗读语音」。Android 11 起要在清单的 `<queries>` 里声明 `TextToSpeech.Engine.INTENT_ACTION_TTS_SERVICE` 才看得见引擎。
- HarmonyOS：`CreateEngineParams.online` 为 1 是离线模式，只用它。文档写明中文的聆小珊（`person` 13）随系统提供，英文的劳拉（`person` 8）和凌飞哲要下载，所以只用 `zh-CN`、13，它支持「中文语境下的英文」；不调 `downloadVoice`，下载弹窗和下载流量都不会出现。
- Windows：SAPI 的声音都是本机安装的。WebView2 去掉了 Edge 里的「Online (Natural)」联网声音（WebView2Feedback #2660 里微软的答复：因为成本在 WebView2 里关掉了这项服务），页面又只用 `localService` 的声音，两道都挡着。
- macOS、iOS：`AVSpeechSynthesizer` 和 WKWebView 的 `speechSynthesis` 用设备上已安装的声音，不经我们的代码发出任何请求。
- Linux：`spd-say` 把文字交给 speech-dispatcher，由用户配置的输出模块合成。常见发行版默认的 espeak-ng 等模块在本机合成，但用户可以自己配一个联网的模块，这一点我们管不了，PRIVACY.md 写明。

所有平台要读的只是那个英文单词，不写日志、不落盘、不进打字统计。剩下管不了的两处（Android 第三方引擎自身的行为、Linux 用户自己配置的 speech-dispatcher 模块）写在 PRIVACY.md 的「朗读」一节。

### 入口

- 候选菜单里的「朗读」，放在已有的「加入生词本」之前、同一组里（最后隔一条线的那组）。有释义能读、或候选本身是英文时出现；只有候选是英文、没有释义时，这一组只有「朗读」。macOS 是标签 31，Windows 是 `CandidateMenuCommand::ReadAloud`，IBus 是 `CandidateReadAloud/<session>/<generation>/<index>`，Fcitx5 是动作 31，Android 是长按菜单的 id 3001（排序 999，「加入生词本」是 1000），iOS 是长按菜单最后那个内联组里的第一项，鸿蒙是 `READ_ALOUD`（打开菜单时记下候选原文，点下去时核对）。它不经过 Engine，不报告指针，不改组字。
- 背单词卡片上，卡片下面一个带喇叭图标的「朗读」按钮（共享页面是 `VocabularyReviewClient.speak`，iOS 和 Android 是原生页面上同样位置的按钮），点一下读词头，不翻开卡片、不作答；读不出来时页面上显示宿主给的那句话。
- 不加全局快捷键，也不加组字时的快捷键：快捷键看不见，桌面端组字时 Ctrl 组合键一律交还应用，要接住就得在四个前端各改一遍按键路由，理由和[桌面候选窗的「加入生词本」](2026-11-01-desktop-candidate-personal-wordbook.md)不做快捷键相同。

### 能力位和开关

`HostCapabilities.read_aloud`：宿主能在背单词卡片上读词时为真，桌面的 `withReadAloud` 和鸿蒙设置页据此给背单词客户端补上 `speak`，页面据此画喇叭。除 Linux 以外都为真；Linux 由设置应用在 `fill_linux_environment` 里按 `PATH` 上有没有 `spd-say` 填。iOS 和 Android 的背单词页是原生的，不读这一位，Android 另外看装没装语音引擎。TS 一侧这一项可以缺省，缺省按不能读处理。原生候选菜单不读这一位，各自按本机引擎在不在决定出不出「朗读」（Linux 找 `spd-say`，Android 看有没有引擎，iOS 看完全访问，鸿蒙看系统能力），因为输入法进程手里没有设置页的能力文档。

不加设置开关：它只在用户点了菜单项或喇叭时出声，平时什么都不做；候选菜单里的这一项又只在有英文释义显示着（「显示英文释义」默认关）或候选是英文时出现，没开释义也不打英文的用户看不到任何变化。仓库没有「新行为一律带开关」的规则，加一个默认关的开关等于让想用的人多找一层设置。

## Alternatives considered

- **sherpa-onnx 的语音合成**：最强的理由是六个平台同一套声音、同一个音质，完全离线，而且语音识别已经在用 sherpa-onnx。不用：现在随包的是只带识别的运行库，Linux x64 和 Windows 要换成带 TTS 的构建，等于换一份新依赖；声音模型（VITS、Kokoro 一类，每个几十 MB）是新的第三方数据，许可要逐个过，体积要进安装包或资源包；为「读一个单词」付这个代价不划算。
- **espeak-ng**：小，离线，几乎每个 Linux 都有。不用：它是 GPL-3.0-or-later 的原生库，链接它就是在六个平台各加一个新依赖和它自带的语音数据；声音明显是机器音，比各平台系统自带的声音差；Linux 上真想用它，经 speech-dispatcher 的默认模块已经能用上，不必我们链接。
- **Linux 链接 libspeechd 或走 D-Bus**：比起起一个进程，调用更轻，也能拿到「读完了」的回调。不用：libspeechd 是构建时要链接的新库；speech-dispatcher 给客户端的接口是它 Unix 套接字上的 SSIP 协议，绕开 libspeechd 就得自己实现一遍协议，又违背「有成熟实现就不手写」。`spd-say` 是 speech-dispatcher 自带的命令行，没装就不出这一项，一点不影响没装的人。
- **Windows 用 WinRT 的 `Windows.Media.SpeechSynthesis`**：新系统上声音更多、接口更现代。不用：它只合成出一段音频流，还要再接 `MediaPlayer` 播放；Server 现在不用 C++/WinRT，Win7 版又没有这套接口；SAPI 在 Server 里已经有现成的用法（系统听写），Win7 到 Win11 都有。
- **Linux 的背单词卡片也用 WebKitGTK 的 `speechSynthesis`**：和 macOS、Windows 走同一条路。不用：WebKitGTK 有没有这个接口、背后接的是 Flite 还是 libspiel，取决于发行版的编译选项，用户装没装都说不准；同一台机器上候选菜单已经认 `spd-say`，卡片走同一个程序，两处的表现才一致。
- **macOS 的背单词卡片也经 Tauri 命令起 `/usr/bin/say`**：三个桌面平台都走命令，网页里不用分平台。不用：Linux 走命令是因为网页那条路靠不住，macOS 的 WKWebView 本来就接着系统声音，再起一个进程只是多一层；Windows 也没有现成的命令行可起。
- **鸿蒙用英文的劳拉声音**：英文更地道。不用：它要先 `downloadVoice`，会弹下载窗、走网络下载声音模型；随系统的中文声音能读「中文语境下的英文」，够读一个单词。以后要用，可以在 `listVoices` 报它已经 `INSTALLED` 时再选它。
- **每个宿主自己判断要读哪段文字**：少一个 C 入口，移动端少一道 JNI、NAPI 转接。不用：七个前端各写一份就有七份规则，生词本的词头规则已经在共享层（`personal_entry_from_gloss`），朗读的规则跟着放在一起，改一次全平台一起变。React 页面的那一份只决定画不画按钮。
- **加一个默认关闭的「朗读」开关**：最保守。不用：理由见上「能力位和开关」。

## Consequences

- 桌面三端开着英文释义、或者打出英文候选的用户，候选菜单最后一组多一项「朗读」；Linux 只在装了 speech-dispatcher 时有。没开释义也不打英文的用户菜单不变。
- macOS、Windows 和装了 `spd-say` 的 Linux 上，背单词卡片下面多一个「朗读」按钮；卡片上的词不是英文（导入的日文词表等）时没有。
- Windows 的 Server 多一个单槽工作线程；中文系统常常只装了中文的 SAPI 声音，这时用中文声音读英文单词，听起来不地道但能听懂；一个声音都没有时提示「系统没有可用的朗读语音」。
- Linux 的 `spd-say` 是异步的，读没读出来我们不知道；speech-dispatcher 没起来时它会自己拉起服务，第一次会慢一点。
- Windows 的 Win7 背单词面板（`settings/win7`）是原生画的，这次没有喇叭；Win7 版的候选菜单照样有「朗读」。
- 移动三端开着英文释义、或者打出英文候选的用户，长按菜单最后一组多一项「朗读」；iOS 要开完全访问，Android 要装着语音引擎。背单词页多一个「朗读」按钮。
- Android 清单多一条对 `TTS_SERVICE` 的查询。语音引擎是系统里另一个应用，声音需不需要网络、要不要下载是它自己报的，报错了我们只能照着挑；没有满足条件的英文声音时（例如只装了中文引擎的国产 ROM）只提示「系统没有可离线使用的英文朗读语音」。
- 鸿蒙用中文声音读英文单词，不如英文声音地道。
- 真机都没有核对：iOS 键盘扩展在开了完全访问时经 `AVSpeechSynthesizer` 能不能出声、iOS 静音开关下读不读、鸿蒙输入法扩展里能不能创建 `textToSpeech` 引擎、国产 Android ROM 有没有装语音引擎。iOS 键盘扩展里出不了声时，菜单项仍然在，点了没有声音；那时要改成不出这一项，只留 App 背单词页的按钮。

## Verification

共享层：`cargo test -p msime-client-services --lib vocabulary::read_aloud` 5 项、`cargo test -p msime-client-core --lib host_surface` 38 项（新增各平台 `read_aloud` 的取值）、`cargo test -p msime-host-api --lib read_aloud` 1 项（空指针、超长请求、多余字段、缺字段、日文释义、英文候选）通过，三个 crate 的 `cargo clippy --lib --tests -- -D warnings` 无警告。设置应用的 `read_aloud` 命令（`spd-say` 只认绝对目录里可执行的文件、非英文拒读）的单测只在 Unix 上编译，macOS 上设置应用的构建脚本要先暂存词库资源，所以交给 PR 上 Linux 的 msime-desktop 作业（clippy 和全部单测）。React：`apps/desktop/tests/settings/vocabulary-review.test.tsx` 新增 2 项（喇叭只在宿主能读且词头是英文时出现、点了读的是词头且不翻卡；读不出来时显示原因），`apps/desktop/tests/core/read-aloud.test.ts` 4 项（只挑本机声音、优先英文、没有本机声音时拒绝、等 `voiceschanged`、Linux 走 `read_aloud` 命令），桌面与鸿蒙设置页 `tsc --noEmit` 通过。macOS：`shortcut-test` 在编译机上构建、拉回本机跑通，新增的断言覆盖「朗读」的位置、带的文字、日文释义不出、英文候选只出它。Windows：`candidate_menu_layout.cpp` 本机编译运行通过（新增「朗读」排在「加入生词本」前、单独成组、不映射到 Engine 动作），改动的候选窗、Server 和 `SystemSpeech.cpp` 用 MinGW 做了语法检查，完整构建交给 PR 的 CI。Linux：`fcitx5_contract.py` 新增两个前端菜单顺序和出现条件的断言，原生构建交给 PR 的 CI。都没有在真机上听过。

移动三端：`cargo test -p msime-client-core --lib host_surface` 38 项通过（三个移动平台的 `read_aloud` 改为真）。Android：`platforms/android/check-host.sh` 通过，`CandidateManagementSmoke` 新增「朗读」的 id 不与释义、词库操作和「加入生词本」相撞，排在「加入生词本」前、所有词库操作后，只在有能读的英文且装着引擎时出现；新的 `ReadAloudVoicePolicySmoke` 覆盖联网和要下载的声音从不选、美式英语优先、同一档按音质、没有能用的时不读；`scripts/test-android-jni-names.py` 确认新 JNI 方法与 `client_jni.cpp`、`verify-native.sh` 一致，`check-host.sh` 新增清单里 `TTS_SERVICE` 查询的检查。iOS：`VocabularyReviewTests` 新增朗读文字的用例，由 PR 的 iOS CI 运行；键盘菜单里这一组的顺序没有用例钉住，要实例化键盘控制器和整套候选状态。鸿蒙：`platforms/harmony/tests/run.sh` 和全部 `scripts/test-harmony-*.py` 通过，`keyboard-logic.test.ts` 覆盖两种菜单里「朗读」的位置、带的文字和候选、英文候选只出它，新的 `read-aloud-policy.test.ts` 覆盖离线模式和随系统的声音、`[h2]`、共享层应答和设置页请求的解析，`run.sh` 钉住视图、会话和设置桥的接线；本机用 DevEco 命令行工具跑了一次 `hvigorw assembleHap`，`CompileArkTS` 报出的错误都在 `workers/LocalAsrWorker.ets`（本机没有暂存 `sherpa_onnx.har`），改动的文件没有错误。三端都没有在真机或模拟器上听过。
