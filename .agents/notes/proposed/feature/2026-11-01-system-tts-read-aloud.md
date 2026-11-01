# Agent Note: 用系统语音合成朗读英文单词

Status: proposed

## Problem

候选旁已经能显示英文释义和音标，用户也能从候选菜单把释义收进[生词本](../../implemented/feature/2026-11-01-personal-wordbook.md)，背单词卡片上也有词和音标，但哪里都听不到这个词怎么读。仓库里没有任何语音合成代码，六个平台的语音都只做识别。

要补的是「点一下，读出这个英文单词」，难处不在播放，而在不引入新东西：sherpa 的语音合成要换带 TTS 的运行库，声音模型是新的第三方数据；espeak-ng 是新的原生依赖。另一个难处是隐私：有的系统语音引擎会联网合成，候选和释义一旦被送出去，就和 PRIVACY.md 里「释义查本机词库、显示时不联网」的说法冲突。

## Proposal

每个平台只用操作系统自带的语音合成，不打包任何声音模型，不加任何依赖；只读英文单词或短语，只在用户点了「朗读」时出声。

### 读哪段文字

共享层定一条规则，所有宿主经同一个 C 入口 `msime_client_read_aloud_text`（`{candidate, gloss}` → 要读的文字或 `null`）取得，不各写一份：

- 释义按行（换行或 U+2028）拆开，每行取第一个义项（`;` 或 `；` 之前），第一个「是英文」的就是要读的文字；
- 没有这样的释义时，候选本身是英文也读它（混输里的英文候选）；
- 「是英文」指只含 ASCII 字母、空格、`-`、`'`、`.`，以字母开头，不超过 64 个字符（与生词本词头的上限相同）。日文、韩文的释义行和带括号、带中文的义项都不读，宁可不出这一项，也不让系统用中文声音念一串怪东西。

背单词卡片读卡片上的词头，过同一条规则。

### 每个平台用什么

| 宿主 | 候选菜单 | 背单词卡片 |
|---|---|---|
| macOS | 输入法进程里的 `AVSpeechSynthesizer`（AVFoundation 本来就链接着） | Tauri 页面（WKWebView）的 Web Speech `speechSynthesis` |
| Windows 新宿主 | Server 里的 SAPI 5 `ISpVoice`，与系统听写（`SystemAsrStream.cpp`）同样在代码里写出类 id，不链接 `sapi.lib` | Tauri 页面（WebView2）的 `speechSynthesis` |
| Linux（IBus、Fcitx5） | 系统装了 speech-dispatcher 的命令行 `spd-say` 时，用 `posix_spawnp` 起它，参数是 `-l en` 和这个词；没装时没有这一项 | 设置应用经 Tauri 命令起同一个 `spd-say` |
| Android | IME 进程里的 `android.speech.tts.TextToSpeech` | 原生背单词页，同一个类 |
| iOS | 键盘扩展里的 `AVSpeechSynthesizer`，只在开了完全访问时有这一项 | App 的原生背单词页，同一个类 |
| HarmonyOS | 键盘里的 Core Speech Kit `textToSpeech` | 设置应用（ArkWeb 里的共享页面）经设置桥交给 ArkTS 侧的同一个接口 |

Web Speech 只用 `SpeechSynthesisVoice.localService` 为真的声音，优先 `en` 开头的，没有本地英文声音时用系统默认的本地声音；一个本地声音都没有时提示「系统没有可用的朗读语音」。

### 不联网

逐个平台按官方文档核对过哪些接口可能联网，能强制本机的就强制：

- Android：系统允许装第三方引擎，引擎可以提供联网的声音。`Voice.isNetworkConnectionRequired()` 说明「这个声音是否需要网络」，`TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED` 说明「这个声音可能要下载额外数据」，而且下载由 `setVoice` 触发。所以只挑语言是英文、不需要网络、也没有 `KEY_FEATURE_NOT_INSTALLED` 的声音，用 `setVoice` 选定它再读；一个都没有时不读，提示「系统没有可离线使用的英文朗读语音」。Android 11 起要在清单的 `<queries>` 里声明 `TextToSpeech.Engine.INTENT_ACTION_TTS_SERVICE` 才看得见引擎，这一条随实现加进 `AndroidManifest.xml` 已有的 `<queries>`。
- HarmonyOS：`CreateEngineParams.online` 为 1 是离线模式，只用它。文档写明中文的聆小珊（`person` 13）随系统提供，英文的劳拉（`person` 8）和凌飞哲要下载，所以只用 `zh-CN`、13，它支持「中文语境下的英文」；不调 `downloadVoice`，下载弹窗和下载流量都不会出现。
- Windows：SAPI 的声音都是本机安装的。WebView2 去掉了 Edge 里的「Online (Natural)」联网声音（WebView2Feedback #2660 里微软的答复：因为成本在 WebView2 里关掉了这项服务），页面又只用 `localService` 的声音，两道都挡着。
- macOS、iOS：`AVSpeechSynthesizer` 和 WKWebView 的 `speechSynthesis` 用设备上已安装的声音，不经我们的代码发出任何请求。
- Linux：`spd-say` 把文字交给 speech-dispatcher，由用户配置的输出模块合成。常见发行版默认的 espeak-ng、RHVoice 等模块在本机合成，但用户可以自己配一个联网的模块，这一点我们管不了，PRIVACY.md 写明。

所有平台要读的只是那个英文单词，不写日志、不落盘、不进打字统计。剩下管不了的两处（Android 第三方引擎自身的行为、Linux 用户自己配置的 speech-dispatcher 模块）写进 PRIVACY.md。

### 入口

- 候选菜单里的「朗读」，放在已有的「加入生词本」之前、同一组里（最后隔一条线的那组）。有释义能读、或候选本身是英文时出现；只有候选是英文、没有释义时，这一组只有「朗读」。四个桌面前端（macOS、Windows、IBus、Fcitx5）和三个移动键盘放在同样的位置。
- 背单词卡片上，词头旁边一个喇叭按钮，点一下读词头。
- 不加全局快捷键，也不加组字时的快捷键：快捷键看不见，桌面端组字时 Ctrl 组合键一律交还应用，要接住就得在四个前端各改一遍按键路由，理由和[桌面候选窗的「加入生词本」](../../implemented/feature/2026-11-01-desktop-candidate-personal-wordbook.md)不做快捷键相同。

### 能力位和开关

`HostCapabilities` 加 `read_aloud`：宿主能在背单词卡片上读词时为真，设置页和背单词页据此决定画不画喇叭。macOS、Windows 为真；Linux 由设置应用在 `fill_linux_environment` 里按 `PATH` 上有没有 `spd-say` 填；三个移动端在接上的那次改动里打开，之前为假。原生候选菜单不读这一位，各自按本机引擎在不在决定出不出「朗读」（Linux 找 `spd-say`，Android 看有没有引擎，iOS 看完全访问），因为输入法进程手里没有设置页的能力文档。

不加设置开关：它只在用户点了菜单项或喇叭时出声，平时什么都不做；候选菜单里的这一项又只在释义显示着（「显示英文释义」默认关）或候选是英文时出现，没开释义的用户看不到任何变化。仓库没有「新行为一律带开关」的规则，加一个默认关的开关等于让想用的人多找一层设置。

### 分三步

1. 本笔记。
2. 桌面：共享层的规则和 C 入口、能力位、macOS、Windows、Linux 两个前端的菜单项，Tauri/React 背单词卡片的喇叭，PRIVACY.md。
3. 移动：iOS、Android、鸿蒙的长按菜单和各自的背单词页，同一个 PR。

## Alternatives considered

- **sherpa-onnx 的语音合成**：最强的理由是六个平台同一套声音、同一个音质，完全离线，而且语音识别已经在用 sherpa-onnx。不用：现在随包的是只带识别的运行库，Linux x64 和 Windows 要换成带 TTS 的构建，等于换一份新依赖；声音模型（VITS、Kokoro 一类，每个几十 MB）是新的第三方数据，许可要逐个过，体积要进安装包或资源包；为「读一个单词」付这个代价不划算。
- **espeak-ng**：小，离线，几乎每个 Linux 都有。不用：它是 GPL-3.0-or-later 的原生库，链接它就是在六个平台各加一个新依赖和它自带的语音数据；声音明显是机器音，比各平台系统自带的声音差；Linux 上真想用它，经 speech-dispatcher 的默认模块已经能用上，不必我们链接。
- **Linux 链接 libspeechd 或走 D-Bus**：比起起一个进程，调用更轻，也能拿到「读完了」的回调。不用：libspeechd 是构建时要链接的新库；speech-dispatcher 给客户端的接口是它 Unix 套接字上的 SSIP 协议，绕开 libspeechd 就得自己实现一遍协议，又违背「有成熟实现就不手写」。`spd-say` 是 speech-dispatcher 自带的命令行，没装就不出这一项，一点不影响没装的人。
- **Windows 用 WinRT 的 `Windows.Media.SpeechSynthesis`**：新系统上声音更多、接口更现代。不用：它只合成出一段音频流，还要再接 `MediaPlayer` 播放；Server 现在不用 C++/WinRT，Win7 版又没有这套接口；SAPI 在 Server 里已经有现成的用法（系统听写），Win7 到 Win11 都有。
- **Linux 的背单词卡片也用 WebKitGTK 的 `speechSynthesis`**：和 macOS、Windows 走同一条路。不用：WebKitGTK 有没有这个接口、背后接的是 Flite 还是 libspiel，取决于发行版的编译选项，用户装没装都说不准；同一台机器上候选菜单已经认 `spd-say`，卡片走同一个程序，两处的表现才一致。
- **鸿蒙用英文的劳拉声音**：英文更地道。不用：它要先 `downloadVoice`，会弹下载窗、走网络下载声音模型；随系统的中文声音能读「中文语境下的英文」，够读一个单词。以后要用，可以在 `listVoices` 报它已经 `INSTALLED` 时再选它。
- **每个宿主自己判断要读哪段文字**：少一个 C 入口，移动端少一道 JNI、NAPI 转接。不用：七个前端各写一份就有七份规则，生词本的词头规则已经在共享层（`personal_entry_from_gloss`），读音规则跟着放在一起，改一次全平台一起变。
- **加一个默认关闭的「朗读」开关**：最保守。不用：理由见上「能力位和开关」。

## Acceptance criteria

- 共享层：规则的单测覆盖英文释义、多行里第二行才是英文、带中文或括号的义项、候选本身是英文、超长、空串；C 入口的边界用例覆盖空指针、超长请求、多余字段。
- 能力位：`HostCapabilities` 的单测钉住各平台的取值，Linux 的取值随 `spd-say` 在不在变。
- 每个前端：菜单测试钉住「朗读」只在能读时出现、排在「加入生词本」之前、和它在同一组；背单词卡片的测试钉住喇叭只在 `read_aloud` 为真时画、点了读的是词头。
- PRIVACY.md 写明朗读用系统语音、不联网的做法，以及 Android 第三方引擎和 Linux speech-dispatcher 模块这两处剩下的情况。
- 不新增任何 crate、npm、Swift、Gradle、ohpm 依赖，不新增构建时链接的库。

## Risks

- 都没在真机上听过：iOS 键盘扩展在开了完全访问时能不能经 `AVSpeechSynthesizer` 出声、鸿蒙输入法扩展里能不能创建 `textToSpeech` 引擎、国产 Android ROM 有没有装语音引擎，都要真机核对。iOS 扩展里不行时，键盘不出这一项，只有 App 的背单词页有喇叭。
- Android 第三方引擎可能把 `isNetworkConnectionRequired()` 报错；我们只能按它报的挑。
- Windows 中文系统常常只装了中文的 SAPI 声音，这时用中文声音读英文单词，听起来不地道但能听懂；没有任何声音时不出声并提示。
- Linux 的 `spd-say` 是异步的，读没读出来我们不知道；speech-dispatcher 没起来时它会自己拉起服务，第一次会慢一点。
