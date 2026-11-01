# 网络请求与数据流向

这份文档说明**本仓库的代码**在什么条件下发出什么网络请求、内容是什么、去哪里、默认是否开启，以及在哪一行代码里。

它不是隐私政策。产品的隐私政策在 <https://msime.app/privacy/>，设置页「关于」里也有入口（`packages/ui/src/settings/pages/about-page.tsx`），各平台都打开它。两者冲突时以隐私政策为准。这里提供的是可以对着代码核对的技术细节，给审阅者和贡献者用。

输入法是能看到你输入的一切的软件，所以这个问题值得一个能逐条验证的答案，而不是一句「我们重视你的隐私」。

## 一句话结论

新装的默认配置下，**没有任何功能会把你输入的内容发出设备**。会这样做的两项都要你自己打开：云联想把当前正在组的拼音串发给 Google 输入工具，新装默认关闭，Windows、Linux 与 macOS 在第一次使用时说明并让你选择，默认选项是不启用（见[云联想](#云联想新装默认关闭)）；候选翻译要你在设置里选择一个翻译服务（自己的凭据，或显式选择「水杉账号」，后者把当前页的中文候选词发到 `api.msime.app`，见[候选翻译](#候选翻译默认不联网)）。其余所有联网功能——语音识别、语音润色、AI 联想、账号同步——默认凭据为空，你不填自己的密钥它们就不会发出任何请求。从旧版本升级时沿用配置文件里已存的值，不会替你改。

另有一条不携带输入内容的自有上报路径，端点是本项目自己的 `https://api.msime.app`：六个平台都会发送匿名使用统计（每天一条活跃记录、每个输入法进程一条会话记录、崩溃时的错误摘要与调用栈），**默认开启**，可以在设置里用「匿名使用统计」（偏好字段 `usage_reporting`）关闭，关闭后不再发送并清空本机队列，见[使用统计与崩溃上报](#使用统计与崩溃上报默认开启可关闭)。打字统计的每日计数只有在你登录账号并打开「上传到云端」后才会上传，默认关闭，见[打字统计上传](#打字统计上传默认关闭需要登录)。设置窗口或 App 首页打开时会拉取一次不带凭据的[服务公告](#服务公告)；社区里只有你主动提交时才会发出[举报和屏蔽作者](#社区举报与审核)的请求。六个平台还会在安装后首次启动时向同一端点注册一个本机匿名水杉账号，只发送本机随机生成的标识与口令，见[账号与同步](#账号与同步需要登录)。上述自有上报不依赖第三方统计或崩溃上报 SDK。本项目还运营下载镜像 `https://dl.msime.app/gh/`：默认开启的词库自动更新，以及资源包、语音和语法模型等按需下载，默认都先从这个镜像取文件、失败再换回 GitHub，所以镜像能看到你的 IP 和下载的是哪个公开发布的文件（不含任何输入内容）；不由 Google Play 更新的 Android 安装包检查更新时查询本项目网站 `https://msime.app/api/releases`，见[资源与更新下载](#资源与更新下载)。可选的 Android 拍照识字模块在本机离线识别，关闭了所用推理库的遥测，见[拍照识字](#android-拍照识字可选模块手动导入)；Android 与 iOS 手写所用 ML Kit 自带的诊断上报也关掉了（Android 在基础 APK 里删掉上传组件，iOS 键盘写入 ML Kit 的统计关闭开关），见[资源与更新下载](#资源与更新下载)。

HarmonyOS 在这些之前还多一道门：设置应用首次启动时弹窗征得隐私政策授权，选择「不同意」或还没选之前，设置应用和键盘都不发出本文列出的任何请求（使用统计、匿名账号注册、公告、账号、社区、云同步、检查更新、词库更新、模型下载，以及你自己配置的云联想、翻译、AI 和云端语音），本机打字照常；升级上来的老用户也会看到一次。规则在 `platforms/harmony/entry/src/main/ets/privacy/PrivacyConsentPolicy.ts`，说明见 `platforms/harmony/README.md` 的「首次启动的隐私政策授权」。

## 逐项说明

### 云联想（新装默认关闭）

| | |
| --- | --- |
| 偏好字段 | `cloud_candidates`，新装与「恢复默认设置」为 `false`；配置文件里缺这个字段时按 `true` 读，升级沿用原来的行为 |
| 目的地 | `https://inputtools.google.com/request` |
| 发送内容 | 当前正在组的拼音串，作为 `text` 查询参数 |
| 需要凭据 | 否 |
| 代码 | `crates/client-core/src/cloud/candidates.rs` 的 `build_google_url`，经 `crates/input-runtime/src/providers.rs` 调用 |

URL 的完整形态可以在同文件的测试里看到，是断言过的字面量：

```
https://inputtools.google.com/request?text=ni%20hao&itc=zh-t-i0-pinyin&num=1&ie=utf-8&oe=utf-8
```

日文输入方案走 `ja-t-i0-und`，其余相同。这是仓库里唯一的云候选来源，没有第二个提供方。

发出前有几道硬性限制，定义在共享层（`candidates.rs` 顶部的常量）：输入为空、超过 256 字节、或含控制字符时不发；响应超过 256 KiB 或单条候选超过 512 字节即丢弃。请求按代次绑定，失焦和换代次会使迟到的结果作废，不会写进新会话。共享层的云候选代码路径不含任何日志调用，查询和响应不经由它落盘。

防抖间隔**由各宿主自己决定**，不是共享常量：共享层的 `spawn_with_debounce` 接受调用方给的时长，`spawn` 默认为零。已在代码中固定取值的是 Android（`OnlineCandidatePolicy.QUIET_INTERVAL_MILLIS = 350`）和 HarmonyOS（`OnlineCandidatePolicy.QUIET_INTERVAL_MS = 350`），两者都是组字停顿 350 ms 后才发一次，并用请求身份保证同一组合只问一次。其余宿主的取值请以各自代码为准。

**默认关闭，桌面平台首次使用时会问**：新装时 `cloud_candidates` 是 `false`（`crates/client-core/src/preferences.rs` 的 `Default`），六个平台都一样。三个桌面平台在第一次使用时先说明这项功能再让你选，默认选项都是不启用：Windows 安装器在全新安装时显示「联网功能」页（`platforms/windows/installer/msime_setup.iss`），勾选框默认不勾，勾上时把选择记在数据目录的 `installer-choices.json`，Server 首次启动准备状态时写进共享偏好（`platforms/windows/src/system/FirstRun.h`），之后删掉这个文件；Linux 的首次配置页（`packages/ui/src/account/linux-setup-page.tsx`）在同一处说明并提供同样默认不勾的选择；macOS 在全新配置下由输入法第一次激活时弹出「联网功能」对话框（`platforms/macos/src/settings/AppearancePreferences.mm` 的 `MSIMEClientCloudCandidatesConsent`），回车对应「不启用」，回答之前不发任何云候选请求。Android、HarmonyOS 与 iOS 不弹询问，在设置里打开（iOS 另有自己的开关，见 `platforms/ios/SharedUI/candidate/CloudCandidatePreference.swift`）。升级都不问，沿用配置文件里已存的值。

**打开或关掉它**：设置页「云联想」开关，或把配置里的 `cloud_candidates` 设为 `true` / `false`。关闭后宿主不再发起云候选请求，并拒绝任何返回的云来源候选。

这与常见中文输入法的云输入功能是同一类能力。它会把你正在输入的内容交给第三方，所以由你决定是否打开；既然仓库公开，端点和发送内容也应当白纸黑字写在这里，而不是让人去读源码才能知道。

### 候选翻译（默认不联网）

`candidate_translations` 新装默认 `false`，已有文档缺少此字段时保留开启行为；支持腾讯机器翻译（`https://tmt.tencentcloudapi.com`）、小牛翻译（`https://api.niutrans.com/v2/text/translate`）和自定义端点。三者都要求你在设置里填入自己的 API 凭据，默认全为空字符串——**没有凭据就不会发出请求**，开关为真也一样。发送内容是待翻译的候选词。代码在 `crates/client-providers/src/credential/translation.rs`。自定义端点的主机名恰好是 `api.deepl.com` 或 `api-free.deepl.com`（https、默认端口）时，按 DeepL v2 格式带上你填的密钥发给 DeepL：密钥以 `:fx` 结尾的发往 `https://api-free.deepl.com/v2/translate`，其余发往 `https://api.deepl.com/v2/translate`，与你填的是两个 DeepL 主机中的哪一个无关；这种地址没有密钥时不发。其他自定义端点原样使用你填的地址。请求由 `crates/client-core/src/translation.rs` 的 `custom_translation_request` 构造，Linux provider（`platforms/linux/scripts/msime-linux-online-provider`）照写的一份由 `shared/contracts/custom-translation/cases.json` 的共享用例约束。

Android 也支持这三种服务（含按主机名识别的 DeepL），在「设置 → 输入 → 翻译 → 翻译服务」里选择和填写凭据。选了它们之后，键盘把当前一页的中文候选词直接发给你配置的第三方服务翻译，不经过水杉服务器，候选词按该服务自己的数据政策处理：每页最多 10 个词，每个不超过 40 字，跳过表情和颜文字候选；腾讯每批最多 9 个词，小牛和自定义每个词单独请求一次；设置了第二目标语言时，每种语言各请求一次。离线释义查得到的语言仍然先用离线释义，但请求不因此省掉。自定义端点在 Android 上只接受 https，或指向本机（`localhost`、`127.0.0.1`、`[::1]`）的 http，局域网 http 不接受。同一设置下已经回答过的词（包括回答「没有译文」的）不会重复请求；失败时同一个词 30 秒内不再请求，整个服务连不上时 30 秒内不再发任何请求。凭据只保存在本机的设置文件里，不随设置同步上传，也不写进日志。代码在 `platforms/android/java/app/msime/android/candidate/UserTranslationClient.java` 和 `TranslationHttpTransport.java`。

macOS、iOS、Android 和 Linux 另提供「水杉账号」（`translation_account`）：选择它后，会把当前页的中文候选词（包括本地已有释义的）连同目标语言代码 POST 到 `https://api.msime.app/v1/translate`。请求带账号令牌：macOS 与 Android 在你已登录时用登录的账号，否则（以及 iOS 上始终）用安装后首次启动时注册的本机匿名账号（见[账号与同步](#账号与同步需要登录)）。所有平台新装和「恢复默认设置」都不选它，只有你显式选择才发送；已有配置文件里存着选择的，升级沿用，缺这个字段的按未选择处理。不想发送，在设置的翻译服务里改选别的服务或「不使用在线翻译」，或关掉候选翻译。你自己的服务优先：候选翻译关闭、小牛或自定义服务已启用、或腾讯已启用且两项凭据都可用时，都不走水杉账号。共享层把这个判定算成翻译查询里的 `translation_account` 字段（`crates/host-api/src/ffi/providers.rs`），各宿主只在它为真时发请求。Windows、HarmonyOS 没有这条路径。没有选择任何服务时不发出候选翻译请求。

macOS 26 及以上在没有选择任何服务时（候选翻译开启，小牛、自定义、腾讯都未启用，也没有选择水杉账号，手动开启候选翻译后就是这个状态），会用 Apple 系统自带的离线翻译模型为随包词典答不上的中文候选补一行释义。翻译在本机完成，候选词不离开这台 Mac；只用你已经在「系统设置 → 通用 → 语言与地区 → 翻译语言」里下载好的语言对，输入法不会触发下载，没下载就不补。代码在 `platforms/macos/src/backend/translation/BackendOnDeviceGloss.swift`，判定在 `InputController.mm` 的 `currentOnDeviceGlossRequest`。

### 语音输入（默认凭据为空）

`voice_input.enabled` 默认 `true`，但这只表示功能可用，录音要你主动触发。默认识别服务是豆包（`wss://openspeech.bytedance.com/...`），**`asr_token` 默认为空字符串**，不填就无法使用。可选的识别服务还有 SiliconFlow、OpenAI、Groq、EveryAPI、Mistral Voxtral、阿里云百炼（`bailian`，默认 `https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions`，模型 `qwen3-asr-flash`，录音以 Base64 放进 Chat Completions 请求），以及两种不出设备的选项：

- `local`：设备上的识别模型，`asr_model_path` 是一个绝对路径，指向设置页下载的 sherpa-onnx 模型目录（包含 `msime-model.json`）。不需要 Token，也没有端点。
- `system`：调用操作系统自带的识别（macOS / iOS / HarmonyOS），数据流向由操作系统决定。macOS 与 iOS 26 起使用 SpeechAnalyzer（设备端）；更早的系统在识别器支持时设置 `requiresOnDeviceRecognition`，不支持时由系统决定是否上传。

选择云端服务时，发送的是录制的音频。代码在 `crates/client-providers/src/credential/asr.rs`，服务选择逻辑在 `crates/client-core/src/preferences.rs`。

**本地模型**全程在设备上运行：录音、识别结果和热词都不离开设备，识别期间不发出任何网络请求。macOS 与 Linux 的输入法进程不自己加载模型，而是拉起本机的 `msime-voice-local` 辅助进程，经标准输入输出交换音频和文本（协议见 `shared/voice/README.md`），不经过网络套接字；Windows 在本机的 `msime-client-server` 进程内识别，Android、iOS 与 HarmonyOS 在应用进程内识别。热词取自你的个人词库（只取用户自己添加的拼音词条），在本机交给识别器，或在识别后于本机做近音替换（`crates/client-providers/src/voice/hotwords.rs`）。

唯一的联网发生在**下载模型**时，且只在你在设置页点「下载」后发生。用「从文件导入」安装时完全不联网：选中的本地文件只在本机按长度和 SHA-256 校验后解压进模型目录；设置页里每个文件的下载链接用系统浏览器打开，输入法本身不发请求（Android 的 full 与拼音版在打开「离线识别」而本机还没有语音运行库时会下载它，见[资源与更新下载](#资源与更新下载)）：

| | |
| --- | --- |
| 目的地 | GitHub Releases（`https://github.com/k2-fsa/sherpa-onnx/releases/download/...`，下载时会被重定向到 GitHub 的文件存储域名）。和资源包一样依次尝试：填了下载镜像时先走你的镜像，然后是本项目的镜像 `https://dl.msime.app/gh/` 加原地址（见[资源与更新下载](#资源与更新下载)），都失败才直连上面的 GitHub 地址 |
| 发送内容 | 对模型归档的 HTTPS GET 请求，User-Agent 为 `msime/<版本号>`，不携带任何输入内容、音频、账号或设备标识 |
| 需要凭据 | 否 |
| 偏好字段 | `voice_input.asr_model_mirror`，默认空字符串，表示不用你自己的镜像，先走本项目的镜像 `https://dl.msime.app/gh/`、再直连 GitHub |
| 代码 | `crates/client-providers/src/voice/local_models.rs`；地址、长度和 SHA-256 固定在 `resources/local-asr-models.json` |

镜像必须是 `https://` 地址，请求 URL 是镜像前缀加上原始 GitHub 地址，所以**镜像运营方能看到你的 IP 和你下载的是哪个模型**；没填自己的镜像时，通常是本项目的镜像先收到这个请求。下载内容按目录里固定的 SHA-256 校验，镜像无法替换文件；校验不过的下载会被丢弃，换下一个来源。重定向离开 HTTPS 时请求被拒绝。模型装好之后，识别不再需要网络。

### 语音润色（不填 Token 不发生）

HarmonyOS 版目前不提供语音润色、AI 联想、AI 对话、高情商回复、AI 润色、AI 候选翻译和 AI 皮肤：这些功能在拿到 AI 生成类资质之前由 `HostCapabilities.ai_generation` 关闭，偏好里存着打开的值也不会发请求（见 `platforms/harmony/README.md`）。下面两节说的是其余五个平台。

开启后把识别出的文本发给一个 Chat Completions 服务做整理。默认值按平台不同：Windows 与 macOS 跟随来源模板，默认服务是 DeepSeek（`https://api.deepseek.com/chat/completions`，模型 `deepseek-v4-flash`），`voice_input.polish_text` 默认开启；其余宿主默认服务是 SiliconFlow，`polish_text` 默认关闭。`voice_input.polish_enabled` 在所有宿主上默认 `false`。无论哪个平台，`polish_token` 默认都为空，宿主在 Token 为空时不发起润色请求，所以默认情况下不会发送任何文本。

### AI 联想（自带端点与密钥）

配置完整才会发起请求，发送内容是当前查询。默认值按平台不同：Windows 与 macOS 跟随来源模板，`ai_assistant.enabled` 默认开启，并预填 DeepSeek 的接口（`https://api.deepseek.com/chat/completions`）与模型（`deepseek-v4-flash`）；Token 默认为空，`chat_completion_http_request`（`crates/client-providers/src/ai.rs`）在没有可用密钥时拒绝构造请求，因此不会发出任何请求。唯一的例外是桌面上的 Ollama：它跑在用户自己的机器或局域网里、不做鉴权，选它时不需要也不发送 Token，接口地址仍按本机和局域网才可用 http 的规则检查。其余宿主默认关闭，接口为空，需要显式启用并自己填写。端点可以改成任意服务，代码对它做校验：必须是 http/https、不得内嵌用户名密码、长度不超过 2048、不含控制字符（`apps/desktop/src-tauri/src/ai.rs`）。常见的 DeepSeek、OpenAI、Groq、SiliconFlow、OpenRouter、EveryAPI 只是文档和测试里出现的示例，仓库不预置任何一家的密钥。

### 账号与同步（需要登录）

`https://api.msime.app`，定义在 `crates/client-services/src/account.rs` 的 `ACCOUNT_ORIGIN`。普通账号不登录不发生。

匿名账号在六个平台上都是安装后自动注册的：本机随机生成一个标识（`msime-` 加 16 位）和一个 48 位口令，先保存在本机，再向 `/v1/auth/challenges` 与 `/v1/auth/login` 换取令牌。只发送这两个随机值，不含输入内容、设备信息或系统账号。网络失败不影响输入，下次启动重试；已有登录或匿名会话时只读本机文件，不再发请求。各平台的时机：

- Linux：Debian `postinst` 为可联系的登录用户注册；手工安装或当时网络不可用时，`msime-linux-setup` 会在首次配置时重试。身份和令牌由 `msime-linux-online-provider` 保存在用户配置目录的 `anonymous-account.json` 与 `anonymous-session.json`，两个文件均为当前用户专用权限，不进入设置页或输入法进程。
- Windows：Server 首次以 `--production` 启动时（安装程序完成页会拉起它），按 Windows 用户分别保存在 `%LOCALAPPDATA%\MSIME\account`，不放进全机共用、所有用户都可写的数据目录。
- macOS：输入法首次激活时（`MSIMEEnsureAnonymousAccount`）；iOS：应用首次启动时，已存有登录或匿名会话（即使已过期）就不发请求。两者都存于 App Group 容器，键盘与应用共用。
- Android：应用首次打开时（`AccountIdentity.register`），已存有匿名会话（即使已过期）就不发请求，存于应用私有存储。
- HarmonyOS：在设置应用的首次启动弹窗里同意隐私政策之后（键盘在读到这份同意之后），存于应用的 `files/state` 目录；没有同意就不注册。

Windows 与 HarmonyOS 除了你主动提交的[社区举报](#社区举报与审核)之外，不用这个账号发送任何内容。候选翻译在你选择了「水杉账号」时才发送，见[候选翻译](#候选翻译默认不联网)；Android 浏览社区皮肤与词库目录时，也会带上匿名账号的令牌（`platforms/android/java/app/msime/android/community/CommunityCatalog.java`），取不到照常列出目录。凭据的存放：iOS 用 Keychain（`crates/tauri-mobile-platform/ios/Sources/MobilePlatformPlugin.swift`），Android 用 Keystore 加密后落盘；macOS、Windows 与 Linux 桌面端存在当前用户私有的 `account-session.json` 里（`crates/client-services/src/account/file_storage.rs`），只有本人可读写，不加密。

**个人词库随云同步上传时，服务端读得到内容。** 在 Android 上登录水杉账号（本机匿名账号不行）并在「我的 › 同步」打开「云同步」（默认关闭）之后，除了设置、常用语和自定义键盘皮肤，个人词库也会同步：本机词库里记为用户词的全部词条（最多 10 万条）整份上传到 `PUT https://api.msime.app/v1/users/me/dictionary/snapshot`，内容只经 HTTPS 保护、没有另外加密，服务端逐条解析保存，看得到每一条的词库种类、编码、词和权重。用户词不只是你自己添加或导入的词和快捷短语，也包括打字时自动造的词：选中的整句候选、云联想和 AI 联想给出的候选，以及整句改字后上屏的整句（最多 7 个音节）；分段选出来拼成的词；连续三次一起选的两个词；回车原样上屏的英文字母串（英文模式、临时英文和临时日文模式里，以及拼音方案里打的不是完整拼音时，例如打 github 后回车；由偏好 `enter_learns_english_word`「回车学英文词」控制，默认开启）；以及给这些中文词自动生成的五笔编码。这些词的权重会随你打字时的选择调整（调频），上传的就是调整后的权重。内置词的调频、删除记录、固定位置、置顶、选词计数和整句联想学到的上下文不上传。上述自动上传到水杉账号的明文词库快照目前只有 Android 接入：iOS 的账号云同步只同步设置和常用语，macOS、Windows、Linux 和 HarmonyOS 不调用这条账号云同步路径；各平台的「云词库」页面只发送你在页面上做的修改，以及你选择导入或恢复的文件。

**「我的词条」的 WebDAV 网盘同步是另一条独立路径。** 用户另行配置 `dictionary_sync_webdav` 并完成设备批准后，会把「我的词条」（包括自动造的用户词）及其删除同步到自己的网盘，不经过水杉账号；条目在本机加密，网盘只保存密文，不上传用户词的调频权重、内置词的调权和其他学习记录。宿主会在打开相关设置、词条变化或定时到期时自动调用共享同步驱动，实际是否联网由共享层的防抖、退避和额度决定，并不都需要每次点「立即同步」。Windows 常驻 Server 的调度在 `platforms/windows/src/system/DictionarySyncSchedule.h` 和 `DictionarySyncWorker.cpp`，共享设置页的调度在 `packages/ui/src/settings/use-dictionary-sync.ts`；数据范围、加密和触发契约见 [`docs/dictionary-webdav-sync.md`](docs/dictionary-webdav-sync.md)。没有配置这组网盘凭据时不启用这条同步。

### 打字统计上传（默认关闭，需要登录）

打字统计默认只在本机。在设置应用的「打字统计」页打开「上传到云端」后（要先登录水杉账号；目前 macOS、Linux 和 Windows 的设置应用，以及 Android、iOS 和鸿蒙的 App/设置提供这个开关；Android 和 iOS 位于「统计」页的「概览」底部），向 `PUT https://api.msime.app/v1/users/me/typing-statistics` 上传最近 7 个有字数的日子的计数：每天的字数、活跃秒数、每小时的字数、各字符类别（汉字、拉丁字母、数字、标点等）和各输入方式（全拼、五笔、语音等）的字数，以及这台设备全部历史的候选位次计数，外加平台名和一个本机随机生成的设备标识（不是匿名使用统计的 `install_id`）。**不上传输入内容**，也不上传按键热力图、徽章、皮肤和语音时长。打开时立刻上传一次，之后在设置应用启动（Android、iOS 是 App 回到前台；鸿蒙还会在设置回到前台）和打开统计页时补传，两次之间至少隔 10 分钟；设置应用和 App 都不在后台定时上传，键盘本身不上传。构建请求体的代码在 `crates/client-services/src/typing_statistics/upload.rs`，开关和设备标识存在统计目录的 `typing-statistics-upload.json`。

服务端只保留 62 天，注销账号时一起删除；关掉「上传到云端」时客户端调用 `DELETE /v1/users/me/typing-statistics` 删掉你在云端的全部打字统计（删除失败时开关仍是关的，页面会提示）。账号数据导出里有这些数据（`typing_statistics.json`）。

这些数据只给 QQ 群助手的 `/stats` 用：在 QQ 群里 @机器人 发 `/bind` 拿到一个 8 位、10 分钟内有效的一次性绑定码，在「账号」页的「绑定 QQ 群助手」里输入（`POST /v1/users/me/qq-links`），之后在群里发 `/stats` 能看到你的打字数据。服务端只存群和成员标识的 SHA-256，不存 QQ 号或昵称；**`/stats` 的回复在群里所有人都看得到**。可以在「账号」页或在群里发 `/unbind` 解绑（`DELETE /v1/users/me/qq-links/{id}`）。绑定码由机器人发、在应用里输入，不反过来：别人看到你的码，最多把你的 QQ 绑到他自己的账号上，看到的是他自己的数据，读不到你的。实现在 `crates/client-services/src/account/{typing_upload,qq_links}.rs`。

### 云剪贴板（需要登录、自建中继或 WebDAV 网盘）

用来在手机和电脑之间传文字：一台设备上把文字放进云剪贴板，另一台设备上从云剪贴板里点选。条目存在 `https://api.msime.app` 你的账号下（改走自建中继时存在你自己的中继上、改用 WebDAV 时存在你自己的网盘上，见下文），最多 50 条，单条不超过 4000 个 UTF-16 单元。

- **端到端加密**：文字在你的设备上加密后才上传，只在你的设备上解密，服务端只保存密文，读不到内容，也不能替你搜索（搜索在设备上进行）。不需要设置口令：每台设备第一次使用时自己生成一对密钥，私钥只留在这台设备上；账号的剪贴板密钥由第一台开启云剪贴板的设备生成，服务端只保存分别用每台设备的公钥封装过的副本。协议公开在 [`docs/cloud-clipboard-e2e.md`](docs/cloud-clipboard-e2e.md)，实现在 `crates/client-core/src/cloud/clipboard_e2e.rs`。
- **新设备要由已有设备批准**：在新设备上登录后，要在一台已经能用云剪贴板的设备上点「允许」，它才拿得到密钥；两台设备上会显示同一组 16 位核对码，核对一致再允许。服务端因此没办法悄悄加一台设备来读你的剪贴板。移除设备或「重置云剪贴板」会换一把新密钥并清空云端条目，其余设备都要重新批准。所有设备都丢了时，在新设备上重置即可，旧条目无法找回。
- **服务端仍然看得到的**：条目数量、写入时间、是否置顶、写入设备的名称、两条内容是否相同（用于去重），以及每台设备的名称、公钥和登记时间。
- **本机密钥放在哪里**：与账号凭据放在一起——iOS 钥匙串、Android 用 Keystore 加密后落盘、HarmonyOS 应用私有目录、macOS/Windows/Linux 当前用户私有的 `cloud-clipboard-keystore` 文件；退出登录时与凭据一起删除。同一台设备上的输入法和设置应用共用这一份，所以在服务端只算一台设备。

- **改走自建中继**：Windows 原生设置窗口、macOS、Linux 与 HarmonyOS 设置页「剪贴板」和 Android、iOS「我的 → 云剪贴板」页的「云剪贴板同步方式」可以选「自建中继」，填中继地址和管理员发的令牌（偏好字段 `cloud_clipboard_relay`）。选了以后，云剪贴板的请求只发往你填的地址、带中继令牌，不再经过 `api.msime.app`，也不需要登录水杉账号；加密和设备批准都和上面一样，上面说的元数据留在你的中继上（中继不记录写入设备的名称）。地址只接受 `https://`（`http://` 只用于本机回环地址），证书必须是公网可信的。令牌和语音、翻译服务的凭据一样明文存在本机的设置文件里，不随设置文件导出、不随账号同步，诊断快照里换成 `<redacted>`。本机密钥另存为当前用户私有的 `cloud-clipboard-relay-keystore`（桌面端与账号会话同一目录；Android 在应用私有的 `no_backup/cloud-clipboard-relay/` 下，和账号那份一样用 AndroidKeyStore 里不能导出的密钥加密，不进云备份，也不随设备间迁移带走；iOS 存在钥匙串里，与账号那份同一个 App Group 访问组、不同的一项，只在本机可读、不随备份去别的设备；HarmonyOS 在应用沙箱的 `files/state` 下，和账号那份一样只靠应用沙箱保护，由设置应用生成，经设置应用与键盘之间的共享目录复制给键盘），退出水杉账号不删除它，换了中继地址或令牌后不再使用旧的那份。Android、iOS 和 HarmonyOS 上配置了中继时，设置页和键盘的「云端」都只连中继，账号页不再为显示条数去问水杉账号；还不能经中继收发的页面（Android、iOS 的 Tauri 设置页和云剪贴板面板）直接拒绝，不改走水杉账号。协议和参考实现见 [`docs/cloud-clipboard-relay.md`](docs/cloud-clipboard-relay.md)，客户端在 `crates/client-services/src/clipboard_relay.rs`。

- **改存到你的 WebDAV 网盘**：Windows 原生设置窗口、macOS、Linux 与 HarmonyOS 设置页「剪贴板」和 Android、iOS「我的 → 云剪贴板」页的「云剪贴板同步方式」可以选「WebDAV」，填网盘里的文件夹地址、网盘账号和应用密码（偏好字段 `cloud_clipboard_webdav`，与自建中继二选一）。保存前先「测试连接」，在那个文件夹里建一个临时子目录试几次写入再删掉。选了以后，云剪贴板的请求只发往你的网盘，不经过 `api.msime.app`，也不需要登录水杉账号；文字照样在本机加密，网盘上只有密文。网盘能看到的元数据与上面「服务端仍然看得到的」相同（不记录写入设备的名称），另外还能看到每次写入的时间和次数：每次写入是 `msime-clipboard/` 文件夹里的一个新文件。删除、清空和换密钥之后，旧的密文会从文件夹里删掉，但可能还留在网盘的回收站或历史版本里（没有密钥解不开）。知道这个应用密码的人能改写网盘上的记录，所以建议给水杉输入法单独开一个应用密码，移除设备之后换掉它。坚果云免费账号每 30 分钟 600 次请求，与你授权的其他第三方应用共用；打开面板或复制时才同步，没有实时推送和后台轮询。账号和应用密码明文存在本机的设置文件里（与中继令牌相同），不随设置文件导出、不随账号同步，诊断快照里账号和应用密码都换成 `<redacted>`。本机还缓存一份从网盘读到的记录（`cloud-clipboard-webdav-state`，只有密文和设备公钥、名称这些公开数据，桌面端在账号会话同一目录，Android 在 `no_backup/cloud-clipboard-relay/` 下，iOS 在 App Group 容器的 `cloud-clipboard-relay/` 下、不进设备备份，HarmonyOS 在 `files/state` 下、并复制给键盘），用来少发请求和发现记录被回退。设备密钥串与自建中继用同一个文件，换了网盘地址或账号后不再使用旧的身份，换应用密码不影响。协议和存储格式见 [`docs/cloud-clipboard-webdav.md`](docs/cloud-clipboard-webdav.md)，客户端在 `crates/client-services/src/clipboard_webdav.rs`。

- **只上传你明确选择的文字**：在云剪贴板面板里输入或粘贴后点上传，或者在本地剪贴板历史的某一条上点「发到云剪贴板」。复制时不会自动上传到云端。（Android 在你开启本机剪贴板历史后会自动把复制的文字记在本机，见下文「默认值」，这些记录同样不会上传；你在键盘里把某一条「添加到常用语」后，它就成了一条常用语，开着常用语同步时会随常用语一起同步。）
- **只在打开面板时读取**：设置页、键盘里的「云端」栏和 macOS 输入法菜单里的「云剪贴板…」在打开和点刷新时各拉取一次列表，没有轮询或推送。
- **密码框里不出现**：Android、iOS、HarmonyOS 键盘和 Linux Fcitx5 在密码类输入框里不显示云端条目，也不发请求；macOS 在安全输入期间不打开云剪贴板面板。
- **关闭即删除**：把云剪贴板关掉会删除云端全部条目、剪贴板密钥的封装和设备登记，共享设置页在关闭前会先请你确认。
- **键盘怎么拿到账号**：键盘和设置应用是不同的进程，账号令牌不会被复制成明文文件。Android 键盘向主进程里一个不导出的 ContentProvider 取令牌，只有主进程会刷新令牌；iOS 的会话存放在 App 与键盘扩展共有的 App Group 钥匙串访问组里，键盘需要「允许完全访问」才会联网；HarmonyOS 键盘读取设置应用在应用私有目录里保存的同一份会话；macOS 的输入法与设置应用读写同一个 `~/Library/Application Support/app.msime.macos/account-session.json`。iOS、HarmonyOS 与 macOS 刷新令牌时持有跨进程文件锁，防止两个进程用同一个刷新令牌而让会话被服务端吊销。
- **从电脑上屏**：Windows 与 Linux 的设置窗口里，云剪贴板面板只有在这次打开时记下了另一个应用的输入窗口才能直接上屏，否则只能复制；它不会沿用以前记下的窗口。

### 资源与更新下载

Linux 安装包没带词库时，首次配置（设置应用的首次配置页，或终端里的 `msime-linux-setup --download`，其他版本是 `msime-linux-<id>-setup`；设置应用里词库的「立即更新」对应 `--update --download`）从 GitHub Releases 取回固定版本的词库，地址、长度和 SHA-256 全部写死在随装的 `desktop-dictionary.lock.json`（源文件是 `resources/desktop-dictionary.lock.json`）里，逐一校验，全部成功才放进词库目录。每个文件先从本项目的镜像 `https://dl.msime.app/gh/` 加原地址下载，失败、超过锁定的长度或 SHA-256 不符时换回 GitHub 原地址，所以通常是本项目的镜像先收到这些请求，能看到你的 IP 和下载的是哪些词库文件；请求的 User-Agent 是 `msime-linux-setup`。环境变量 `MSIME_DOWNLOAD_MIRROR` 可以把这个前缀换成别的镜像，设为空串则只直连 GitHub。代码在 `platforms/linux/scripts/msime-linux-setup`。下载的是公开发布物，不上传任何东西。检查更新在点击「检查更新」时进行，由应用本身（不经网页）向 `https://api.github.com/repos/metasequoiaime/msime/releases` 发起 GET 请求，在本地按平台标签前缀、版本、架构和更新通道筛选；请求除 IP 地址和固定的 User-Agent `msime-client` 外不携带标识，适用 GitHub 隐私条款。Windows、Linux 与 macOS 的「关于」页可以选更新通道（偏好 `update_channel`：稳定版为默认，另有测试版、每日构建；只属于本机，不进设置文件和账号同步），选每日构建时同一次检查另外向 `https://api.github.com/repos/metasequoiaime/msime/releases/tags/nightly` 发一个同样不带标识的 GET；通道只决定在本地怎么筛选，不随请求发出。Linux 的设置应用另外在打开设置窗口时（启动或被叫到前台时；只为表情、剪贴板、屏幕键盘等面板启动时不请求）在后台按所选通道发同样的请求，成功后一天内不再请求，失败后六小时内不再请求，发现新版本时只发一条桌面通知，不下载也不安装；上次检查的时间、所用的通道、提醒过的版本和找到的版本记在状态目录的 `update-check.json`。Fcitx5 插件和 IBus 宿主本身不发这个请求。Windows 的常驻进程（安装后的 Server，预览实例不算）启动五分钟后开始、之后每小时看一次是否到期，按同样的节流在后台发同样的请求（Windows 7 专用版只查正式发布），发现新版本时只弹一条系统通知，不下载也不安装；记录在本用户的 `%LOCALAPPDATA%\MSIME\update\update-check.json`（其他版本在各自的用户目录下），设置窗口打开「关于」页时只读这个文件，不联网。macOS 带更新配置的发布包由 Sparkle 更新：输入法每天一次、设置应用在点「检查更新」时，向 `https://github.com/metasequoiaime/msime/releases/download/macos-appcast/` 下所选通道的 appcast（稳定版 `appcast-<版本 id>.xml`，测试版和每日构建是 `appcast-beta-…`、`appcast-nightly-…`，设置应用的文件名多一段 `settings-`）发 GET，没有打开 Sparkle 的系统信息上报，输入法在后台下载签名的更新包并在切换输入源或退出时安装。构建时或运行时设置环境变量 `MSIME_DISABLE_UPDATE_CHECK=1` 可关掉 Linux 与 Windows 的后台检查，「检查更新」按钮照常可用；开发构建不做后台检查。

Android 的检查更新不走上面的 GitHub API：不由 Google Play 更新的安装包（为 Play 打的包和从 Play 安装的都不做自更新，关于页也不显示这几项）在「关于」页点「检查更新」时，以及打开「自动更新」（默认关闭，只存在本机）后由系统任务在联网时每天一次，向 `https://msime.app/api/releases?platform=android`（本项目网站对 GitHub 发行版列表的缓存）发 GET，在本地按版本和更新通道挑选。有新版本时，你在关于页点下载、或自动更新在后台，把本版本的 APK 和同一发布里的 SHA-256 校验文件下载到应用缓存的 `updates/`；每个文件先从本项目的镜像 `https://dl.msime.app/gh/` 加 GitHub 发布页地址下载，镜像的任何失败（网络、HTTP 状态、摘要不符）都换回 GitHub 原地址，所以本项目的镜像能看到你的 IP 和下载的是哪个版本。安装前核对 SHA-256、包名、更高的版本号和与当前安装一致的签名证书，再交给系统安装器由你确认，从不静默安装。这些请求的 User-Agent 是 `MSIME/Android`，不带账号、设备标识或输入内容；代码在 `platforms/android/java/app/msime/android/account/UpdateApi.java` 与 `platforms/android/java/app/msime/android/home/UpdateJobService.java`。

六个平台的「自动更新词库」默认开启。设置应用启动、回到前台或运行期间按每天一次的节流，通过 `https://api.github.com/repos/metasequoiaime/msime-dictionary/releases?per_page=100` 查询公开版本，并从该仓库的 HTTPS Release 附件地址下载兼容的词库、清单与许可文本；GitHub 可能重定向到自己的文件存储域名。查询版本和下载清单时 User-Agent 是固定的 `msime-dictionary-updater`，下载词库文件时是 `msime/<版本号>`，都不携带账号、设备标识、输入内容或个人词条。查询版本的 API 请求始终直连 GitHub；Release 附件（清单、词库与许可文本）和资源包一样依次尝试：填了下载镜像（偏好 `voice_input.asr_model_mirror`）时先走你的镜像，然后是本项目的镜像 `https://dl.msime.app/gh/` 加原地址，都失败才直连 GitHub，所以通常是本项目的镜像先收到下载请求，能看到你的 IP 和下载的是哪个词库版本。摘要先取自 GitHub API，再与清单交叉校验，不论文件从哪里取到都按它校验，镜像无法替换文件。没下完的词库文件留在状态目录的 `dictionary-updates/.partial-downloads/`，下次检查时带 `Range` 头接着下。可在「词库更新」关闭自动更新，关闭后仍可手动检查。代码在 `crates/client-providers/src/dictionary_updates.rs`。

桌面和 Android 发布包不内置的几个资源包由设置应用在首次用到时下载，之后从本机读取。哪些平台下载哪些：

- 日文词典、粤拼注音与笔画词库：macOS 和 Android full 版下载，它们的发布包只内置打中文所需的核心词库；Android 拼音版只下载日文词典（给临时日语用），它不提供粤拼、注音和笔画方案，所以不下载语言词库；Windows 和 Linux 的安装包带着它们，Android 日文版的 APK 带着日文词典。
- 非英文离线释义（法、日、西、俄、德、韩）与本地语音运行库（sherpa-onnx 与 ONNX Runtime 两个原生库）：只有 Android 的 full 与拼音版下载，其他平台和 Android 的其他版本照旧随包。
- 手写模型：macOS 下载；Windows 只在系统没有中文 Windows Ink 手写识别器、安装目录里也没有模型时下载；Linux 只在安装前缀里没有随包模型时下载（我们自己发布的 deb/rpm 不带它，各发行版仓库的包继续内置）。
- 桌面神经联想模型（约 25 MB）：三个桌面平台都只在打开「桌面神经联想」时下载，安装布局里已经带着它时不下载。
- 离线英文音标词典（ECDICT 生成的 `en-phonetic.db` 与许可声明，约 5.7 MB）：只有 Android、iOS 和 Linux 下载，而且只在你打开「显示英文音标」（偏好字段 `candidate_pronunciation`，默认关闭）后、在设置里那一行点「下载」时发生，打开开关本身不联网；macOS 和 Windows 随安装包带着，鸿蒙随 HAP 带着。原地址是 `https://github.com/metasequoiaime/chinese-ime-lm/releases/download/pronunciations-ecdict-.../`，和其他资源包一样依次尝试：填了下载镜像时先走你的镜像，再走本项目的镜像 `https://dl.msime.app/gh/` 加原地址，都失败才直连 GitHub，所以通常是本项目的镜像先收到这个请求，能看到你的 IP 和下载的是这个资源包；长度和 SHA-256 固定在 `resources/pronunciations.lock.json`，不论从哪里取到的都按它校验。装好后打字时只在本机查这张表，不发送候选、释义或任何输入内容；iOS 上它放在 App Group 共享目录的 `MSIME/resource-packs/pronunciations/`，不进 iCloud 备份，删除 App 时一并删除。

三个桌面平台的 octagram 语法模型只在你点「检查更新」或「下载／更新」时读取官方目录 `https://raw.githubusercontent.com/metasequoiaime/msime/develop/resources/grammar-models.catalog.json`；稳定版、测试版和每日构建在本机按已保存的更新通道筛选，不把通道作为请求参数发送。目录返回模型下载地址、大小和 SHA-256；下载由你再次点击后进行，和其他资源包一样依次尝试：填了下载镜像时先走你的镜像，然后是本项目的镜像 `https://dl.msime.app/gh/` 加目录中的上游 HTTPS 地址，都失败才直连上游地址；镜像不提供的文件会被拒绝，请求随即换到下一个来源，但被尝试过的镜像都能看到你的 IP 和请求的是哪个模型。读取目录本身始终直连上面的官方地址。请求只包含固定路径和 User-Agent `msime/<版本号>`，不发送输入内容、账号、设备标识、本地路径或已安装模型列表。已安装模型日常离线使用，不因目录更新自动下载或替换；目录或下载失败时保留旧模型。

| | |
| --- | --- |
| 触发 | 桌面上只在这几种情况下发生：在设置里选日文、五笔、粤拼、注音或笔画方案（macOS；水杉五笔随包带着五笔码表）；在设置里选用本地语音识别而输入法没有自带语音运行库（macOS）；第一次打开手写面板（宿主需要下载手写模型时）；打开「桌面神经联想」；设置应用启动时发现已保存的方案（或上一次的中文方案）需要的词库、已打开的桌面神经联想需要的模型、或已选用的本地语音识别需要的运行库（macOS）还没装；在资源包那一行点「下载」或「重试」。Android 上只在你在「添加语言」里添加日语、五笔、粤语、注音或笔画，打开离线释义，打开「离线识别」（本机还没有语音运行库时），或在资源包那一行点「下载」「重试」时发生；应用启动时不自动下载，唯一的例外是五笔码表：全功能版的当前方案或上一次的中文方案是五笔、本机却没有五笔码表时（例如在首次设置里选了五笔），回到设置应用时弹窗说明原因和大小，问你要不要下载，你点了「下载」才开始（水杉五笔随包带着五笔码表，拼音版没有五笔）。词库自动更新已经下载过同一份五笔码表或日文词典时，安装资源包直接从本机复制，不联网。使用按流量计费的网络时先告诉你下载大小、等你确认。从内置这些文件的旧版 Android 升级时，已经解压在本机的文件按固定的长度和 SHA-256 校验后直接转成资源包，不联网；新版同时换了词库版本、旧文件与新的长度和 SHA-256 对不上时，旧文件删除，改为按上面的规则下载 |
| 目的地 | GitHub Releases（`https://github.com/metasequoiaime/msime-dictionary/releases/download/dict-v.../` 的词库，`https://github.com/metasequoiaime/chinese-ime-lm/releases/download/model-v1/` 的桌面神经联想模型，`https://github.com/metasequoiaime/chinese-ime-lm/releases/download/offline-glosses-.../` 的离线释义，`https://github.com/k2-fsa/sherpa-onnx/releases/download/v.../` 的 Android 语音运行库归档，`https://github.com/metasequoiaime/msime/releases/download/voice-runtime-macos-v.../` 的 macOS 语音运行库归档（上游库用本项目 Developer ID 重签后发布），下载时会被重定向到 GitHub 的文件存储域名）与 `https://raw.githubusercontent.com/metasequoiaime/msime-engine/<固定提交>/...`（手写模型）。每个文件依次尝试：配置了镜像时先从你的镜像下载，然后是本项目的镜像 `https://dl.msime.app/gh/` 加原地址（Cloudflare R2 前面的缓存，只放行本项目和上面这几个上游的发布文件），都失败再回到上面的原地址 |
| 发送内容 | 对固定文件的 HTTPS GET 请求，User-Agent 为 `msime/<版本号>`；中断后接着下载时带 `Range` 头，只说明从第几个字节开始。不携带任何输入内容、账号或设备标识 |
| 需要凭据 | 否 |
| 偏好字段 | 沿用 `voice_input.asr_model_mirror`，默认空字符串，表示不用你自己的镜像，先走本项目的镜像 `https://dl.msime.app/gh/`、再直连 GitHub；下载失败时资源包那一行和手写面板都提供「设置下载镜像」 |
| 存放位置 | 偏好目录下的 `resource-packs/<资源包>/`：macOS 是 `~/Library/Application Support/app.msime.macos/resource-packs/`，Windows 是输入法服务的数据目录，Linux 是运行时选项记录的状态目录，Android 是应用私有目录 `files/bootstrap/state/resource-packs/`（没下完的部分留在同一目录下的 `.partial-<资源包>/`，下次接着下载）；许可证文本放在数据旁边 |
| 代码 | `crates/client-providers/src/resource_packs.rs`、`apps/desktop/src-tauri/src/platform/desktop/desktop_resource_packs.rs`，Android 经 `crates/host-api/src/ffi/pack_downloads.rs` 调用同一份实现；地址、长度和 SHA-256 固定在 `resources/desktop-dictionary.lock.json`、`resources/language-dictionaries.lock.json`、`resources/offline-glosses.lock.json`、`resources/pronunciations.lock.json`、`resources/voice-runtime.lock.json` 与 `resources/voice-runtime-android.lock.json`（语音运行库归档和从中取出的两个库）、`resources/voice-runtime-macos.lock.json`（重签的 macOS 语音运行库归档和其中的库）、`resources/handwriting-model.lock.json` 和 `resources/settled-model.lock.json` |

镜像规则和上面[本地语音模型](#语音输入默认凭据为空)的相同：必须是 `https://` 地址，镜像运营方能看到你的 IP 和你下载的是哪个资源包，但下载内容按固定的 SHA-256 校验，镜像无法替换文件，校验不过就换下一个来源。不需要这些功能就不会发生这些请求；缺少资源包时对应的方案或手写面板显示为不可用，桌面神经联想保持现有候选；Android 上缺少的语言不出现在键盘里，离线释义只有英文，本地语音识别不可用，中文输入不受影响。

Android 的手写识别使用 ML Kit，**首次使用需要联网下载识别模型**（从 Google 的 `dl.google.com` 下载），之后在设备上离线识别。ML Kit 自带经 Google datatransport 上报的使用诊断：每次检查模型、下载模型或识别时记一条 `FIREBASE_ML_SDK` 事件（带 ML Kit 在本机生成并长期保留的 `ml_sdk_instance_id`、应用包名与版本、手写模型语言、机型与硬件、系统版本与指纹、系统语言、国家、时区、运营商代码 MCC/MNC 和网络类型，不含笔迹和识别结果），攒在应用数据库里，由系统任务调度在设备空闲时上传到 `firebaselogging.googleapis.com`。2026-10-25 在 Pixel 3 XL（Android 12）上用 PCAPdroid 抓包确认过这条上传（HTTPS，发出约 1.9 KB）。基础 APK 的清单删掉了 datatransport 的后端发现服务、上传任务和闹钟入口：ML Kit 照常识别，诊断事件不再写入、不再排上传任务；从旧版本升级时数据库里已有的事件也不会再上传。删掉之后在模拟器上从下载模型到识别上屏，整机抓包只有 `dl.google.com`（模型）和本项目的 `api.msime.app`。升级 ML Kit 时要重新做这项网络检查。iOS 键盘扩展链接的 ML Kit（`MLKitCommon` 14.0.0，带 `GoogleDataTransport` 10.1.0）同样会记使用统计：它从扩展自己的 `UserDefaults` 读 `com.google.firebase.ml.<模块>.__MLKIT_DEFAULT.isStatsCollectionEnabled`，读不到就当作同意。2026-10-25 在 iPhone XR（iOS 18.7.10，商店版 0.52.0，打开「允许完全访问」）上用 rvictl 镜像整机流量、tcpdump 按进程抓包，同时看系统日志：每次识别时扩展都去读 `digitalInk` 这个键、读不到，紧接着 ML Kit 报 `No binding was found for required, single-bound service: CCTPolicyVending_API`，也就是上传所需的 Clearcut 传输没有链接进来；约 6 分钟里（写字、收起键盘让扩展进入后台、再写字）扩展没有发起任何网络连接，也没有查询 Google 或 Firebase 的域名。这只是碰巧没发，升级 ML Kit 可能把这条传输带进来，所以键盘在第一次调用 ML Kit 之前把 `digitalInk`、`mddDownloader`、`commonDownloader` 和 `vision` 四个模块的这个键都写成 `false`（`platforms/ios/KeyboardExtension/Sources/handwriting/MLKitStatsOptOut.swift`），ML Kit 读到后不再记录这些事件。设置 App 不链接 ML Kit。iOS 升级 ML Kit 时同样要重新做这项检查。Windows 先用系统的 Windows Ink 识别器，没有中文识别器时用 Engine 的离线 Zinnia 模型；Linux 与其余桌面端都用这个 Zinnia 模型。发行版仓库的 Linux 包随附该模型，从安装路径读取，全程不联网；macOS、Windows 和我们自己发布的 Linux deb/rpm 按上一段在需要时下载，之后同样离线识别。

### Android 拍照识字（可选模块，手动导入）

配套模块 `msime-ocr-v1.apk` 发布在本仓库的 `ocr-module-android-v1` Release，只由用户手动下载后在拍照识字设置里导入。宿主只接受 `resources/ocr-module-android.lock.json` 固定的完整 SHA-256，无法导入任意 APK 启用本功能；基础 APK 不包含 OCR 推理库和模型，也不自动下载模块；Google Play 渠道不提供外部可执行模块。

相机权限由用户在拍照识字设置页主动授予，相机只在键盘拍照面板打开时工作。照片仅在内存中识别，不自动保存图片、复制文字或上屏；用户确认后才把选中文字交给当前编辑器。关闭面板、隐藏键盘或切换输入框时释放相机并清空待选结果，迟到结果作废。

模块用 ONNX Runtime 1.31.0 运行 PaddlePaddle 的 PP-OCRv6 tiny 检测与识别模型，图片和识别结果只在本机内存中处理。ONNX Runtime 的 Android 包自带向 Microsoft 上报的遥测：模块清单删掉了它的初始化 Provider 和网络权限，创建运行环境时也关闭了遥测。真机（Pixel 3 XL，Android 12）上连续识别 41 张图和一次键盘内拍照识别期间及其后 60 秒，水杉应用 UID 没有任何套接字，系统流量统计里该 UID 的收发字节不变。模块的依赖、许可证和来源见 [docs/third-party.md](docs/third-party.md#android-可选离线-ocr-模块resourcesocr-models-androidlockjson)；升级 ONNX Runtime 时要重新做这项网络检查。

### 使用统计与崩溃上报（默认开启，可关闭）

六个平台都通过同一份共享实现向 `https://api.msime.app/v1/telemetry/events` POST 事件：`crates/client-services/src/telemetry.rs`（队列、安装 id、事件规则、发送与重试），原生宿主经 `crates/host-api` 的 `msime_client_telemetry_{begin,end,record_crash,flush,clear}`（`crates/host-api/src/ffi/reporting.rs`，声明在 `crates/host-api/include/msime_client.h`）调用它。**事件里没有任何输入内容、候选、剪贴板、账号标识或硬件信息**。

|                |                                                                                                                                                                                                                                                                                                                                                               |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 目的地         | `https://api.msime.app/v1/telemetry/events`，不带任何凭据（服务端和任何请求一样能看到来源 IP）                                                                                                                                                                                                                                                                |
| 偏好开关       | `usage_reporting`，默认 `true`。设置页「关于 → 隐私 → 匿名使用统计」在所有平台都显示（`packages/ui/src/settings/telemetry-section.tsx`）；Windows 原生设置在「关于」里另有同一开关（`platforms/windows/settings/main.cpp`），iOS 在「关于水杉 → 发送匿名使用统计」，Android 在「词库与输入 → 需要留意的 → 匿名使用统计」。旧字段 `telemetry_enabled` 不再读取 |
| 每条事件的字段 | 随机事件 id、`kind`、`platform`（`windows`/`macos`/`linux`/`android`/`ios`/`harmony`）、`version`（应用的真实版本号）、`install_id`；只有 `crash` 另带 `message` 与 `stack`                                                                                                                                                                                   |
| `install_id`   | 每个安装首次使用时随机生成一次，存在本机的 `telemetry-state.json` 里，不由硬件、账号或任何用户数据派生                                                                                                                                                                                                                                                        |
| 需要凭据       | 否                                                                                                                                                                                                                                                                                                                                                            |

事件只有四种，客户端不再发送 `download`（GitHub Release 的下载数由服务端统计，装机数由 `active` 统计）：

- `active`：每个安装每个 UTC 日最多一条，id 固定为 `active-<install_id>-<yyyymmdd>`，重复发送也只记一次。
- `session`：输入法宿主进程正常结束时记一条（iOS 键盘是每次键盘弹出到收起），在下一次发送时送出。
- `session_crash`：一个会话没有正常结束**并且**崩溃处理程序为它写下了崩溃记录时才记，下次启动时补报，每个会话最多一条。只留下会话标记（注销、关机、系统杀进程、内存不足）不算崩溃，不产生任何事件。同一目录里同时有几个宿主进程时（例如同一用户的两个图形会话），每个进程按自己的会话 id 结束会话、写崩溃记录，不会把别的进程的会话记成崩溃或结束；`telemetry-state.json` 为此记着最多 16 个尚未结束的会话 id、版本和开始它的进程号，以及最多 16 个已经补报过 `session_crash` 的进程号；进程号只留在本机，用来把系统事后报告的崩溃（iOS 的 MetricKit）对回会话，不随任何事件发送。
- `crash`：崩溃处理程序在崩溃时**只写本地文件、不联网**，下次启动时才转成事件发送。`message` 是异常或信号的摘要（最多 1000 个字符），`stack` 是调用栈帧（最多 16000 个字符、12 KiB，在行边界截断）。共享层在入队前去掉每一行路径里的目录部分，只留文件名，所以用户名和文件夹名不会离开设备。每批最多转换 8 条崩溃记录，其余保留到后续补报；原记录只在队列持久化成功后删除。新记录保存发生时的平台、版本、组件、架构、源码提交、构建运行与重跑编号；升级后补报仍用原身份，构建身份位于消息第二行，不含设备路径或输入内容。

发送与保留：事件先写进本机队列再发送，`202` 后移除；`400` 类拒绝直接丢弃不再重试；`429`（遵守 `Retry-After`）、`5xx` 和网络错误保留到下次用同一个 id 重发。队列最多 64 条，优先保留崩溃：容量不足时淘汰最旧的普通统计，全是崩溃时把新增诊断继续留在记录文件中等待投递释放容量。关闭开关会停用统计，并删除队列、会话标记和崩溃记录；`install_id` 文件保留，这样重新打开时不会把同一个安装算成两次。已经交给网络的请求无法撤回。队列里格式无效的事件在读取时丢弃。

iOS、共用原生辅助进程、MCP、桌面设置/面板，以及使用共用包装器的 macOS 输入法与原生设置窗口、Windows Server、Windows 原生设置、IBus、Fcitx5 与 Win7 设置/面板的采集准备和队列另绑定本机持续授权：共享偏好 `usage_reporting_consent` 保存随机授权代次和起点，队列状态与原生崩溃记录头保存所属代次。它们不随事件、设置同步或诊断包上传。关闭后重开会换代，旧进程晚写的记录不能进入新队列；首次接入这套协议时，无法证明属于当前授权的旧待发报告也会丢弃，匿名安装 id 保持。iOS 的 MetricKit 诊断批次必须完全位于本轮持续授权之后，每条发送前重新核对授权，网络发送期间不占用偏好锁。共用辅助采集器用于 Windows 看门狗和准备程序、Linux 原生及 Python 辅助进程、macOS 本地语音进程；它们只落盘、不联网。MCP 同样只落盘、不联网；桌面设置/面板发送端逐条核对当前授权，Linux 的 Fcitx5 补收也按同一规则。共享桌面入口收到真实偏好目录时自动绑定当前代次，显式旧代次仍拒绝；没有真实偏好来源或读取失败时停用，已经接入授权的队列拒绝无绑定普通操作。共用输入会话包装器另用独立观察线程核对持久授权，发送阻塞期间也响应关闭，旧请求收尾不覆盖新授权。Android 的 App、IME、Java 同意镜像、发送和系统退出补录也使用共享代次；系统退出只认发生时的同代次摘要，并再次核对发生时间。Java 镜像不自行授予权限，故障现场只使用预备身份。鸿蒙的 App 在共享目录发布合并统计开关与隐私政策决定的授权，键盘只读现有文件和锁；两侧每秒独立核对，不等待焦点镜像或网络完成。关闭重开换代，临时读失败暂停且不删除队列。共享目录不可用时键盘不上报，App 使用私有权威授权；发布失败停止本进程使用旧许可，但无法保证另一沙箱立即得知撤回。共享授权只留本机，普通偏好镜像不复制它。这些分支代码不等于六个平台的设备与后台边界均已验收。

桌面共享设置与面板进程，以及 Windows 原生设置、Win7 原生面板和 macOS 原生设置窗口（`--preferences`，组件 `settings-native`），在读取同意后准备独立崩溃捕获，不额外建立输入会话。原生捕获器在崩溃现场只保存本地记录，不发送网络请求。

各平台的会话边界、崩溃来源和文件位置：

- **Windows**（`platforms/windows/src/entrypoints/server_main.cpp`，包装层 `platforms/common/Telemetry.cpp`）：会话是 Server 进程从启动到消息循环结束；开关读自偏好里的 `usage_reporting`（`platforms/windows/src/system/TelemetryConsent.h`：缺省算开启，显式 `false` 算关闭，读不出来的值或文件算关闭），设置页保存后立即生效。启动时发送一次，之后每 30 分钟一次。崩溃来源：`std::terminate` 只记录异常类型（JSON 解析异常另记编号），不取 `what()` 文本；未处理的结构化异常由 `SetUnhandledExceptionFilter` 记录异常名与代码、出错模块文件名+偏移和逐帧的模块+偏移（例如 `EXCEPTION_ACCESS_VIOLATION (0xc0000005) in module.dll+0x…`）。文件在 `%LOCALAPPDATA%\MSIME\` 下：`telemetry.json`、`telemetry-state.json`、`telemetry-session.json`、`telemetry-crashes\`。安装器的「联网功能」页说明了这项统计默认开启（`platforms/windows/installer/msime_setup.iss`）。
- **Linux**：IBus 宿主（`platforms/linux/src/entrypoints/ibus_main.cpp`）的会话是 `msime-linux-ibus` 从启动到主循环返回，崩溃守护以 `--recovered` 重启的进程同样如此；ibus-daemon 停止组件、注销和关机时发来的 SIGTERM（以及 SIGHUP、SIGINT）也先退出主循环、正常结束会话，再按信号的默认动作退出；Fcitx5 插件（`platforms/linux/fcitx5/FcitxEngine.cpp`）的会话是插件实例的生存期，发送在后台任务里进行，不占用事件循环。崩溃来源：`std::terminate`（规则同 Windows），以及 SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT、SIGTRAP 的信号处理程序，写下 `SIGSEGV: segmentation fault (code N)` 这样的摘要和预先准备的模块文件名、模块相对偏移、ELF build-id；信号不是执行出错、而是用 `kill`、`raise` 一类调用发来的时，摘要改为写明来源：本进程自己发出的写 `(raised by this process)`，别的进程发来的写 `(sent by <进程名>)`，进程名取发送者的 `/proc/<pid>/comm`（最多 15 个字符，是系统里另一个程序的名字，不写进程号），读不到时写 `another process`。然后交回原先的处理程序（Fcitx5 自己的崩溃日志照常工作）。启动时发送一次，之后每 30 分钟一次。开关是共享偏好 `preferences.json` 里的 `usage_reporting`。文件在 `$XDG_STATE_HOME/msime/`（未设时为 `~/.local/state/msime/`）下，文件名同 Windows；Fcitx5 用自己的子目录 `$XDG_STATE_HOME/msime/fcitx5/`，因此有自己的 `install_id`，同一台机器上两种框架都用会被算成两个安装。
- **macOS**（`platforms/macos/src/core/UsageReporting.mm`，由 `input_method_main.mm` 调用）：会话是输入法进程的生存期，单独打开的设置窗口不算。进程被 SIGTERM 结束（设置应用安装或更新输入法时结束旧进程，或系统注销、关机时发来 SIGTERM）也算正常结束，先排进 `session` 再退出；被 SIGKILL 强制结束才只留下会话标记。崩溃来源：`NSSetUncaughtExceptionHandler`（异常名和「二进制名+偏移」形式的栈帧，不取异常原因）和 SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT、SIGTRAP 的信号处理程序（预先准备的二进制文件名、相对偏移和 Mach-O UUID）。启动时发送一次，之后每 3 小时一次。单独打开的原生设置窗口（`--preferences`）在同一持续授权下准备独立崩溃捕获（组件 `settings-native`），记一条 `active` 并在打开时与之后每 3 小时补发队列，不建立输入会话，窗口关闭时停止。开关是共享设置页的「匿名使用统计」。文件在 `~/Library/Application Support/MSIME/telemetry/`；水杉五笔、水杉拼音等其他版本各用 `~/Library/Application Support/MSIME/<版本 id>/telemetry/`，同时安装的版本互不共享 `install_id`。
- **iOS**（`platforms/ios/SharedUI/core/UsageReporting.swift`）：会话是键盘的一次弹出到收起（`KeyboardExtension/Sources/core/KeyboardUsageReporting.swift`），App 本身不产生会话。崩溃来源：App 与键盘共用的信号处理程序（`CrashSignalRecorder.c`，信号名、模块文件名、相对偏移和 Mach-O UUID）和未捕获异常处理程序（异常名和「二进制名+偏移」形式的栈帧，不取异常原因）；App 和键盘扩展的崩溃还来自系统在 App 下次打开时送来的 MetricKit 诊断（`CrashDiagnostics.swift`），只取异常类型、代码、信号、ObjC 异常名、系统给出的终止原因和崩溃线程「二进制名+偏移」形式的栈帧，从不取 ObjC 的 `composedMessage`。诊断按进程号对回键盘当时尚未结束的会话：键盘自己的处理程序已经记下的同一次崩溃不再记第二份，没记下的补记为那个会话的崩溃（`crash` 加 `session_crash`）；被系统强制结束（SIGKILL，例如看门狗、内存上限、挂起时仍持有 App Group 里的文件锁即 `0xdead10cc`）的摘要以 `Killed` 开头，按上面的规则不算会话崩溃，与 App 自身的崩溃一样只报 `crash`。MetricKit 卡死诊断单独以 `Hang` 摘要补录，不增加 `session_crash`。键盘只有在「允许完全访问」打开时才发送，最多每 15 分钟一次；否则由 App 在回到前台时发送。文件在 App Group 容器的 `MSIME/telemetry/` 下，App 与键盘共用一个 `install_id`。`PrivacyInfo.xcprivacy` 声明了 CrashData、ProductInteraction 与 DeviceID（随机安装 id），均为不关联身份、不用于跟踪。
- **Android**（`platforms/android/java/app/msime/android/core/Telemetry.java`，经 `platforms/android/native/client_jni.cpp` 调用共享层）：会话只在 `:ime` 进程里，是 `MSIMEInputService` 从 `onCreate` 到 `onDestroy`。崩溃来源：两个进程的未捕获异常处理程序只记录异常类型（不取消息文本）和 Java 栈帧（类、方法、源文件名与行号，最多 4 层 Caused by）；App 进程的崩溃在键盘下次启动时作为单独的 `crash` 发送。JNI 接入共用原生信号捕获器，保存故障指令与模块相对偏移、ELF build-id。Android 11 起另读取曾启用采集的进程留下的短身份与系统退出代码，补录崩溃、ANR 和低内存退出；不读取原始描述、系统日志或 tombstone。App 打开时发送，键盘弹出时最多每 6 小时发送一次。文件在应用私有目录 `files/telemetry/`。
- **HarmonyOS**（`platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets`，经 `platforms/harmony/native/client_napi.cpp` 调用共享层）：会话是键盘进程从 `KeyboardExtensionAbility.onCreate` 到 `onDestroy`，设置应用只发送不产生会话。崩溃来源包括 NAPI 接入的共用原生信号记录器、API 21 默认错误处理器，以及系统在下次启动时投递的 HiAppEvent `APP_CRASH`。本进程入口在正常路径固定原记录路径与构建头，暂停、关闭和停止时同步停写；默认错误处理器同步保存后把原异常交给前任，无前任时同步退出，不在故障回调刷新偏好或联网。JS 崩溃只取标准错误类型和调用栈帧，不取消息文本；原生崩溃取信号名与代码和「文件名+pc+符号」形式的栈帧，不含故障地址和目录。正常执行时把共享层预备的原 UUID、构建头、PID 和准备时间作为本机系统自定义参数保存；补报时验证原身份和当前持续授权，不用补报版本重建。构建头里的本机授权代次在入队时剥离。原生帧额外保留合法 ELF build-id，用于选择符号。系统 `APP_FREEZE` 卡死只保留标准原因，以 `Hang` 摘要独立补录，不增加 `session_crash`，不读取 hilog、页面历史和外部日志。键盘启动时与之后每 6 小时发送一次，设置应用打开时也发送；授权观察独立于发送。文件在 `files/state/telemetry/` 下，不再读写单独的键盘 PID 文件。当前分支的原身份实际系统投递仍待真机验证；官方系统事件订阅从 API 22 支持输入法应用，最低 API 21 已接入本进程原生与主线程 ArkTS 兼容采集，真实致命故障、原处理器链与同源包仍待设备验收。

### 服务公告

设置窗口或 App 首页打开时，向 `GET https://api.msime.app/v1/notices?channel=app&platform=<平台>` 拉取一次公告列表，不带凭据，也不带安装 id；服务端缓存一分钟，客户端最快一分钟问一次，输入法进程从不在后台拉取。共享实现在 `crates/client-services/src/notices.rs`。拉取的地方：

- Windows、macOS、Linux 的共享设置窗口，以及 Tauri 版 Android/iOS 设置（`apps/desktop/src-tauri/src/notices.rs`，在设置页挂载时读取一次）；缓存与已关闭的 id（最多 200 个）存在共享状态目录的 `notices.json`。
- iOS App 的「设置」首页打开或回到前台时（`platforms/ios/SharedUI/core/AppNotices.swift`），存在 `Application Support/MSIME/notices`；键盘不拉取。
- Android App 的「设置」页出现时（`platforms/android/java/app/msime/android/home/NoticeBanner.java`），存在 `files/notices`；`:ime` 进程不拉取。
- HarmonyOS 设置窗口打开或回到前台时（`platforms/harmony/entry/src/main/ets/pages/Settings.ets`），存在 `files/state/notices`；键盘不拉取。

公告正文是简单 Markdown，在本机渲染，原始 HTML 不执行，不加载图片；正文里的链接只有在你点击时才用系统浏览器（或邮件应用）打开，且只接受 `http`、`https` 和 `mailto`。关闭某条公告只记在本机。

### 社区举报与审核

社区里看别人发布的皮肤、候选窗口皮肤、插件、词库和回复模板时，每一项都有「举报」入口。只有你选择理由并提交时，才会向 `POST https://api.msime.app/v1/community/reports` 发送 `{kind, item_id, reason, detail}`：`reason` 是固定的六个理由之一（侵权/抄袭、色情低俗、违法违规、垃圾广告、恶意插件、其他），`detail` 是你自己填写的可选说明（最多 1000 字）。请求带账号令牌，已登录用登录账号，否则用本机匿名账号。共享实现在 `crates/client-services/src/community/report.rs`。

iOS、Android、HarmonyOS 以及 macOS 与 Linux 设置应用的作品详情里另有「屏蔽作者」，要先登录水杉账号（不用本机匿名账号）。确认屏蔽时才向 `POST https://api.msime.app/v1/community/blocks` 发送 `{kind, item_id}`，服务端据此找到作品的作者并记在你的账号下，之后你的账号浏览社区目录时不再列出这位作者的作品；服务端不把作者的账号编号返回给客户端，只返回这条屏蔽的编号、作者的显示名和屏蔽时间。「我的 › 隐私 › 已屏蔽的作者」（macOS、Linux 与 HarmonyOS 2in1 在「关于 › 许可与隐私」里）打开时读取 `GET /v1/community/blocks`，取消屏蔽发 `DELETE /v1/community/blocks/{id}`。注销账号时屏蔽记录一并删除。共享实现在 `crates/client-services/src/community/block.rs`。

社区采用先发布后审核：发布的内容立即公开，管理员可以下架。查看「我的作品」或自己作品详情时，请求带 `fields=moderation`，只用来在被下架的作品上显示「已下架」；不会显示下架理由，也没有待审核状态。

## 留在本地的东西

- **输入历史与学习数据**由输入引擎（`crates/engine`）管理，写在宿主提供的用户目录里，默认不上传。例外是个人词库：在 Android 上打开云同步后，记为用户词的词条会以服务端读得到的形式上传，其中包括打字时自动造的词和它们学到的权重，见[账号与同步](#账号与同步需要登录)。内置词的调频、删除记录、固定位置、置顶、选词计数和整句联想学到的上下文不上传。
- **剪贴板历史默认关闭**（`clipboard_history` 默认 `false`）。开启后写入状态目录下的 `clipboard_history.json`，仅本地；关闭时会清空该文件。Android 上开启后，水杉作为当前输入法时会在你每次复制文字后自动记下一条（隐私模式、密码类输入框和系统标记为敏感的内容除外），不再需要手动点「保存当前」。
- **@ 名单**：用户在设置页「插件」里手动添加的名字和地点写在状态目录下的 `plugins/mentions.json`，仅本地，不读取通讯录或位置。它不在 `preferences.json` 里，所以宿主拷贝偏好和账号同步都不会带上它。「@ 名字与地点」模式默认关闭（`local_modes.mention` 默认 `false`）；从这个模式以及 V、/ 模式上屏的文字不进学习数据，也不计入打字统计。
- **生词本**：从候选释义加进来、或者从文本导入的生词，连同背单词的复习进度，都写在应用数据目录的 `vocabulary-wordbooks/personal.json` 和 `vocabulary-progress.json` 里，只在本机。生词本不进账号同步、云词库或输入习惯同步，也不上传；词头是候选旁显示的外文释义，释义是你选中的那个中文候选。最多 5000 个词。iOS 键盘要开启「允许完全访问」才写得进与 App 共用的目录。鸿蒙的键盘和设置应用各有沙箱，键盘先把词记在自己沙箱的 `keyboard-vocabulary/` 里，完整体验模式下经本机的共享沙箱交给设置应用、并进背单词的生词本，不经过网络。交接过程中本机还有三份副本：键盘沙箱里还没交出去的那本、共享沙箱里键盘发布的那本，以及设置应用私有目录里存下的上一次并过的那一版（`keyboard-vocabulary/vocabulary-wordbooks/personal.json`，用来避免把你移出的词再并回来）。两边还各有一份只记次数和指纹、不含词的清走记录 `keyboard-vocabulary/drained.json`。设置应用并过以后，键盘下次弹出时清走已经交过的词，键盘沙箱和共享沙箱里那本随之只剩还没交出去的词，设置应用下次回到前台时删掉它存的那一版；这一轮走完之前，在背单词里删掉「生词本」或移出某个词，不会立即删掉这几份副本。基础访问模式下，或者没有共享沙箱（模拟器、签名不带数据共享授权）时，键盘加的词一直留在键盘沙箱里，要等开启完整体验模式交出去，或者清除应用数据、卸载才会删除。除此之外，在背单词里删掉「生词本」这本书就清空了它；导出是你自己选的位置，导出以后那份文件归你处理。代码在 `crates/client-services/src/vocabulary/library.rs` 和 `personal.rs`，鸿蒙的交接在 `platforms/harmony/entry/src/main/ets/keyboard/PersonalWordbookPolicy.ts` 与 `SharedStateSync.ets`。
- **导入的辅助码表**：Android 在「输入 › 辅助码方案」里导入的 `.txt` 码表只在本机校验后复制到状态目录下的 `plugins/helpcode/`，不联网、不上传；应用只读你在系统文件选择器里选中的那一个文件，不申请存储权限。
- **设置**保存在应用数据目录的 `preferences.json`，可用绝对路径环境变量 `MSIME_CLIENT_STATE_DIR` 指向隔离目录。

## 没有的东西

本项目没有集成 Sentry、Mixpanel、Amplitude、Crashlytics 或 Google Analytics 作为统计或崩溃上报工具；自行实现的上报路径见[使用统计与崩溃上报](#使用统计与崩溃上报默认开启可关闭)。这不包含 ML Kit 等第三方功能 SDK 自身的诊断收集；Android 手写、拍照识字和 iOS 手写里已知的 SDK 上报已经关掉，边界见上文。下面的命令用于核对本项目直接实现的上报代码，不能据此排除 SDK 内部的联网行为；命中的文件主要有：

- 共享实现与 C ABI：`crates/client-services/src/telemetry.rs`、`crates/client-services/src/telemetry/tests.rs`、`crates/client-services/src/lib.rs`、`crates/client-services/Cargo.toml`（crate 描述）、`crates/client-services/src/account/client.rs`（发送用的 HTTP 客户端）、`crates/client-services/src/notices.rs`（文档注释里提到它）、`crates/client-core/src/preferences.rs`（`usage_reporting` 开关）、`crates/host-api/include/msime_client.h`、`crates/host-api/src/ffi/reporting.rs`、`crates/host-api/src/tests.rs`。
- 设置页开关：`packages/ui/src/index.tsx`、`packages/ui/src/settings/telemetry-section.tsx`、`packages/ui/src/settings/about-settings-actions.ts`、`packages/ui/src/settings/pages/about-page.tsx`、`apps/desktop/tests/settings/settings.test.tsx`。
- Windows 与 Linux：`platforms/common/Telemetry.{h,cpp}`、`platforms/common/tests/telemetry.cpp`、`platforms/windows/CMakeLists.txt`、`platforms/windows/settings/main.cpp`、`platforms/windows/src/entrypoints/server_main.cpp`、`platforms/linux/CMakeLists.txt`、`platforms/linux/README.md`、`platforms/linux/fcitx5/CMakeLists.txt`、`platforms/linux/fcitx5/FcitxEngine.cpp`、`platforms/linux/src/entrypoints/ibus_main.cpp`、`platforms/linux/tests/core/ibus_startup_telemetry.py`。
- Apple：`platforms/macos/src/core/UsageReporting.{h,mm}`、`platforms/ios/SharedUI/core/UsageReporting.swift`、`platforms/ios/tests/core/UsageReportingTests.swift`。
- Android：`platforms/android/README.md`、`platforms/android/java/app/msime/android/core/{Telemetry,MSIMEInputService}.java`、`platforms/android/java/app/msime/android/home/{HomeActivity,KeyboardSheets}.java`、`platforms/android/tests/core/TelemetryHandlerSmoke.java`。
- HarmonyOS：`platforms/harmony/README.md`、`platforms/harmony/native/client_napi.cpp`、`platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets`、`platforms/harmony/entry/src/main/ets/entryability/EntryAbility.ets`、`platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets`、`platforms/harmony/entry/src/main/ets/pages/Settings.ets`、`platforms/harmony/tests/keyboard-logic.test.ts`、`platforms/harmony/tests/tsconfig.json`。

```sh
git grep -ilwE '(telemetry|analytics|sentry|mixpanel|crashlytics)' -- crates apps packages platforms shared
```

`-w` 是必须的：不加的话 `PROCESSENTRY32W` 会匹配上 `sentry`，触感振幅和波形振幅的 `amplitude` 会匹配上 `amplitude`，全是误报。但别改写成 `\b…\b` 的形式——`git grep -E` 走的是 POSIX 扩展正则，`\b` 在这里不表示词边界，那样写的结果是命令永远返回零命中，看着像通过，其实什么都没查到。

依赖侧再核一遍锁文件：

```sh
grep -niE 'opentelemetry|sentry|mixpanel|crashlytics|google-analytics' pnpm-lock.yaml Cargo.lock
```

唯一的命中是 `@opentelemetry/api`，它是测试框架 vitest 的**可选 peer 依赖**，没有被安装，也不进入任何产物。

## 自己核对

```sh
# 所有出现在代码里的外部地址
git grep -nIoE 'https?://[a-zA-Z0-9.-]+\.[a-z]{2,}' -- crates apps packages platforms shared | sort -u

# 云候选的完整 URL 构造与其边界检查
sed -n '/fn build_google_url/,/^}/p' crates/client-core/src/cloud/candidates.rs

# 所有默认开启的偏好
grep -n 'enabled_by_default' crates/client-core/src/preferences.rs
```

发现本文档与代码不符，请按[贡献指南](.github/CONTRIBUTING.md)提 issue；若涉及数据泄露或凭据问题，走[安全策略](.github/SECURITY.md)的私下报告入口，不要开公开 issue。
