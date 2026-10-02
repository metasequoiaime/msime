# 网络请求与数据流向

这份文档说明**本仓库的代码**在什么条件下发出什么网络请求、内容是什么、去哪里、默认是否开启，以及在哪一行代码里。

它不是隐私政策。产品的隐私政策在 <https://msime.app/privacy/>，设置页「关于」里也有入口（`packages/ui/src/index.tsx`）；Linux 的「隐私政策」入口直接打开本文档，与 Windows 参考实现链接它自带的 PRIVACY.md 一致。两者冲突时以隐私政策为准，但 Linux 除外：msime.app/privacy/ 的 Linux 部分目前与这个宿主不符（它写的是不检查更新、凭据存进 Secret Service，而实际会检查更新、服务商凭据存在 0600 权限的文件里），在网站更正之前，Linux 的实际行为以本文档中可对照代码的描述为准。这里提供的是可以对着代码核对的技术细节，给审阅者和贡献者用。

输入法是能看到你输入的一切的软件，所以这个问题值得一个能逐条验证的答案，而不是一句「我们重视你的隐私」。

## 一句话结论

默认配置下，**会把你输入的内容发出设备的只有云联想，以及 macOS 与 Linux 新装时的候选翻译**。它默认开启，把当前正在组的拼音串发给 Google 输入工具；Windows、Linux 与 macOS 在第一次使用时先问（见[云联想](#云联想默认开启)），macOS 在回答之前不发请求。其余所有联网功能——语音识别、语音润色、AI 联想、账号同步——默认凭据为空，你不填自己的密钥它们就不会发出任何请求。候选翻译在 macOS 与 Linux 新装时默认用「水杉账号」，把当前页的中文候选词发到 `api.msime.app`（见[候选翻译](#候选翻译macos-与-linux-新装默认用水杉账号)）；其他平台默认不联网，需要你在设置里选择一个翻译服务（自己的凭据，或显式选择「水杉账号」）。

另有一条不携带输入内容的自有上报路径，端点是本项目自己的 `https://api.msime.app`：六个平台都会发送匿名使用统计（每天一条活跃记录、每个输入法进程一条会话记录、崩溃时的错误摘要与调用栈），**默认开启**，可以在设置里用「匿名使用统计」（偏好字段 `usage_reporting`）关闭，关闭后不再发送并清空本机队列，见[使用统计与崩溃上报](#使用统计与崩溃上报默认开启可关闭)。设置窗口或 App 首页打开时会拉取一次不带凭据的[服务公告](#服务公告)；社区里只有你主动提交时才会发出[举报](#社区举报与审核)。六个平台还会在安装后首次启动时向同一端点注册一个本机匿名水杉账号，只发送本机随机生成的标识与口令，见[账号与同步](#账号与同步需要登录)。仓库不接入任何第三方统计或崩溃上报 SDK。

## 逐项说明

### 云联想（默认开启）

| | |
| --- | --- |
| 偏好字段 | `cloud_candidates`，默认 `true` |
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

**首次使用会先问**：默认值是开启，但三个桌面平台在第一次使用时先说明这项功能再让你选。Windows 安装器在全新安装时显示「联网功能」页（`platforms/windows/installer/msime_setup.iss`），取消勾选就写入 `cloud_candidates = false`；Linux 的首次配置页（`packages/ui/src/account/linux-setup-page.tsx`）在同一处说明并提供选择；macOS 在全新配置下由输入法第一次激活时弹出「联网功能」对话框（`platforms/macos/src/settings/AppearancePreferences.mm` 的 `MSIMEClientCloudCandidatesConsent`），回答之前不发任何云候选请求。升级都不问，沿用已存的值。

**关掉它**：设置页「云联想」开关，或把配置里的 `cloud_candidates` 设为 `false`。关闭后宿主不再发起云候选请求，并拒绝任何返回的云来源候选。

保持默认开启是为了与已发布的平台输入法行为一致。这与常见中文输入法的云输入功能是同一类能力，但既然仓库公开，端点和发送内容就应当白纸黑字写在这里，而不是让人去读源码才能知道。

### 候选翻译（macOS 与 Linux 新装默认用水杉账号）

`candidate_translations` 默认 `true`，支持腾讯机器翻译（`https://tmt.tencentcloudapi.com`）、小牛翻译（`https://api.niutrans.com/v2/text/translate`）和自定义端点。三者都要求你在设置里填入自己的 API 凭据，默认全为空字符串——**没有凭据就不会发出请求**，开关为真也一样。发送内容是待翻译的候选词。代码在 `crates/client-core/src/credential/translation.rs`。

macOS、iOS、Android 和 Linux 另提供「水杉账号」（`translation_account`）：选择它后，会把当前页的中文候选词（包括本地已有释义的）连同目标语言代码 POST 到 `https://api.msime.app/v1/translate`。请求带账号令牌：macOS 与 Android 在你已登录时用登录的账号，否则（以及 iOS 上始终）用安装后首次启动时注册的本机匿名账号（见[账号与同步](#账号与同步需要登录)）。macOS 与 Linux 的出厂默认（新装，以及「恢复默认设置」）就选中它，所以新装后打字即会发送；已有配置文件里没有这个字段的，按未选择处理，升级不会替你打开。iOS 与 Android 默认不选，只有你显式选择才发送。不想发送，在设置的翻译服务里改选别的服务或「不使用在线翻译」，或关掉候选翻译。你自己的服务优先：候选翻译关闭、小牛或自定义服务已启用、或腾讯已启用且两项凭据都可用时，都不走水杉账号。共享层把这个判定算成翻译查询里的 `translation_account` 字段（`crates/host-api/src/ffi/providers.rs`），各宿主只在它为真时发请求。Windows、HarmonyOS 没有这条路径。没有选择任何服务时不发出候选翻译请求。

macOS 26 及以上在没有选择任何服务时（候选翻译开启，小牛、自定义、腾讯都未启用，也没有选择水杉账号——新装默认选了水杉账号，所以要先改选「不使用在线翻译」），会用 Apple 系统自带的离线翻译模型为随包词典答不上的中文候选补一行释义。翻译在本机完成，候选词不离开这台 Mac；只用你已经在「系统设置 → 通用 → 语言与地区 → 翻译语言」里下载好的语言对，输入法不会触发下载，没下载就不补。代码在 `platforms/macos/src/backend/translation/BackendOnDeviceGloss.swift`，判定在 `InputController.mm` 的 `currentOnDeviceGlossRequest`。

### 语音输入（默认凭据为空）

`voice_input.enabled` 默认 `true`，但这只表示功能可用，录音要你主动触发。默认识别服务是豆包（`wss://openspeech.bytedance.com/...`），**`asr_token` 默认为空字符串**，不填就无法使用。可选的识别服务还有 SiliconFlow、OpenAI、Groq、EveryAPI、Mistral Voxtral，以及两种不出设备的选项：

- `local`：设备上的识别模型，`asr_model_path` 是一个绝对路径，指向设置页下载的 sherpa-onnx 模型目录（包含 `msime-model.json`）。不需要 Token，也没有端点。
- `system`：调用操作系统自带的识别（macOS / iOS / HarmonyOS），数据流向由操作系统决定。macOS 与 iOS 26 起使用 SpeechAnalyzer（设备端）；更早的系统在识别器支持时设置 `requiresOnDeviceRecognition`，不支持时由系统决定是否上传。

选择云端服务时，发送的是录制的音频。代码在 `crates/client-core/src/credential/asr.rs`，服务选择逻辑在 `crates/client-core/src/preferences.rs`。

**本地模型**全程在设备上运行：录音、识别结果和热词都不离开设备，识别期间不发出任何网络请求。macOS 与 Linux 的输入法进程不自己加载模型，而是拉起本机的 `msime-voice-local` 辅助进程，经标准输入输出交换音频和文本（协议见 `shared/voice/README.md`），不经过网络套接字；Windows 在本机的 `msime-client-server` 进程内识别，Android、iOS 与 HarmonyOS 在应用进程内识别。热词取自你的个人词库（只取用户自己添加的拼音词条），在本机交给识别器，或在识别后于本机做近音替换（`crates/client-core/src/voice/hotwords.rs`）。

唯一的联网发生在**下载模型**时，且只在你在设置页点「下载」后发生：

| | |
| --- | --- |
| 目的地 | GitHub Releases（`https://github.com/k2-fsa/sherpa-onnx/releases/download/...`，下载时会被重定向到 GitHub 的文件存储域名）；配置了镜像时改为镜像地址 |
| 发送内容 | 对模型归档的 HTTPS GET 请求，User-Agent 为 `msime/<版本号>`，不携带任何输入内容、音频、账号或设备标识 |
| 需要凭据 | 否 |
| 偏好字段 | `voice_input.asr_model_mirror`，默认空字符串，表示直接访问 GitHub |
| 代码 | `crates/client-core/src/voice/local_models.rs`；地址、长度和 SHA-256 固定在 `resources/local-asr-models.json` |

镜像必须是 `https://` 地址，请求 URL 是镜像前缀加上原始 GitHub 地址，所以**镜像运营方能看到你的 IP 和你下载的是哪个模型**。下载内容按目录里固定的 SHA-256 校验，镜像无法替换文件；校验不过的下载会被丢弃。重定向离开 HTTPS 时请求被拒绝。模型装好之后，识别不再需要网络。

### 语音润色（不填 Token 不发生）

开启后把识别出的文本发给一个 Chat Completions 服务做整理。默认值按平台不同：Windows 与 macOS 跟随来源模板，默认服务是 DeepSeek（`https://api.deepseek.com/chat/completions`，模型 `deepseek-v4-flash`），`voice_input.polish_text` 默认开启；其余宿主默认服务是 SiliconFlow，`polish_text` 默认关闭。`voice_input.polish_enabled` 在所有宿主上默认 `false`。无论哪个平台，`polish_token` 默认都为空，宿主在 Token 为空时不发起润色请求，所以默认情况下不会发送任何文本。

### AI 联想（自带端点与密钥）

配置完整才会发起请求，发送内容是当前查询。默认值按平台不同：Windows 与 macOS 跟随来源模板，`ai_assistant.enabled` 默认开启，并预填 DeepSeek 的接口（`https://api.deepseek.com/chat/completions`）与模型（`deepseek-v4-flash`）；Token 默认为空，`chat_completion_http_request`（`crates/client-core/src/ai.rs`）在没有可用密钥时拒绝构造请求，因此不会发出任何请求。其余宿主默认关闭，接口为空，需要显式启用并自己填写。端点可以改成任意服务，代码对它做校验：必须是 http/https、不得内嵌用户名密码、长度不超过 2048、不含控制字符（`apps/desktop/src-tauri/src/ai.rs`）。常见的 DeepSeek、OpenAI、Groq、SiliconFlow、OpenRouter、EveryAPI 只是文档和测试里出现的示例，仓库不预置任何一家的密钥。

### 账号与同步（需要登录）

`https://api.msime.app`，定义在 `crates/client-core/src/account.rs` 的 `ACCOUNT_ORIGIN`。普通账号不登录不发生。

匿名账号在六个平台上都是安装后自动注册的：本机随机生成一个标识（`msime-` 加 16 位）和一个 48 位口令，先保存在本机，再向 `/v1/auth/challenges` 与 `/v1/auth/login` 换取令牌。只发送这两个随机值，不含输入内容、设备信息或系统账号。网络失败不影响输入，下次启动重试；已有登录或匿名会话时只读本机文件，不再发请求。各平台的时机：

- Linux：Debian `postinst` 为可联系的登录用户注册；手工安装或当时网络不可用时，`msime-linux-setup` 会在首次配置时重试。身份和令牌由 `msime-linux-online-provider` 保存在用户配置目录的 `anonymous-account.json` 与 `anonymous-session.json`，两个文件均为当前用户专用权限，不进入设置页或输入法进程。
- Windows：Server 首次以 `--production` 启动时（安装程序完成页会拉起它），按 Windows 用户分别保存在 `%LOCALAPPDATA%\MSIME\account`，不放进全机共用、所有用户都可写的数据目录。
- macOS：输入法首次激活时（`MSIMEEnsureAnonymousAccount`）；iOS：应用首次启动时，已存有登录或匿名会话（即使已过期）就不发请求。两者都存于 App Group 容器，键盘与应用共用。
- Android：应用首次打开时（`AccountIdentity.register`），已存有匿名会话（即使已过期）就不发请求，存于应用私有存储。
- HarmonyOS：应用首次启动或键盘首次加载时，存于应用的 `files/state` 目录。

Windows 与 HarmonyOS 除了你主动提交的[社区举报](#社区举报与审核)之外，不用这个账号发送任何内容。候选翻译在选择了「水杉账号」时才发送（macOS 与 Linux 新装默认选中），见[候选翻译](#候选翻译macos-与-linux-新装默认用水杉账号)；Android 浏览社区皮肤与词库目录时，也会带上匿名账号的令牌（`platforms/android/java/app/msime/android/community/CommunityCatalog.java`），取不到照常列出目录。凭据的存放：iOS 用 Keychain（`crates/tauri-mobile-platform/ios/Sources/MobilePlatformPlugin.swift`），Android 用 Keystore 加密后落盘；macOS、Windows 与 Linux 桌面端存在当前用户私有的 `account-session.json` 里（`crates/client-core/src/account/file_storage.rs`），只有本人可读写，不加密。

### 云剪贴板（需要登录）

用来在手机和电脑之间传文字：一台设备上把文字放进云剪贴板，另一台设备上从云剪贴板里点选。条目存在 `https://api.msime.app` 你的账号下，最多 50 条，单条不超过 4000 个 UTF-16 单元。

- **只上传你明确选择的文字**：在云剪贴板面板里输入或粘贴后点上传，或者在本地剪贴板历史的某一条上点「发到云剪贴板」。任何平台都不会自动读取系统剪贴板，复制时也不会自动上传。
- **只在打开面板时读取**：设置页、键盘里的「云端」栏和 macOS 输入法菜单里的「云剪贴板…」在打开和点刷新时各拉取一次列表，没有轮询或推送。
- **密码框里不出现**：Android、iOS、HarmonyOS 键盘和 Linux Fcitx5 在密码类输入框里不显示云端条目，也不发请求；macOS 在安全输入期间不打开云剪贴板面板。
- **关闭即删除**：把云剪贴板关掉会删除云端全部条目，共享设置页在关闭前会先请你确认。
- **键盘怎么拿到账号**：键盘和设置应用是不同的进程，账号令牌不会被复制成明文文件。Android 键盘向主进程里一个不导出的 ContentProvider 取令牌，只有主进程会刷新令牌；iOS 的会话存放在 App 与键盘扩展共有的 App Group 钥匙串访问组里，键盘需要「允许完全访问」才会联网；HarmonyOS 键盘读取设置应用在应用私有目录里保存的同一份会话；macOS 的输入法与设置应用读写同一个 `~/Library/Application Support/app.msime.macos/account-session.json`。iOS、HarmonyOS 与 macOS 刷新令牌时持有跨进程文件锁，防止两个进程用同一个刷新令牌而让会话被服务端吊销。
- **从电脑上屏**：Windows 与 Linux 的设置窗口里，云剪贴板面板只有在这次打开时记下了另一个应用的输入窗口才能直接上屏，否则只能复制；它不会沿用以前记下的窗口。

### 资源与更新下载

首次准备词库时从 GitHub Releases 拉取固定版本的资源，地址、长度和 SHA-256 全部写死在 `resources/desktop-dictionary.lock.json` 里，逐一校验，全部成功才发布到内容标识目录。下载的是公开发布物，不上传任何东西。检查更新只在点击「检查更新」时进行，向 `https://api.github.com/repos/metasequoiaime/msime/releases` 发起 GET 请求并在本地按平台标签前缀筛选（识别不出宿主平台时改为读取 `https://msime.app/update.json`）；请求除 IP 地址和防缓存时间戳外不携带标识，适用 GitHub 隐私条款。

Android 的手写识别使用 ML Kit，**首次使用需要联网下载识别模型**，之后在设备上离线识别。Linux 与桌面端使用 Engine 随附的离线 Zinnia 模型，从安装路径读取，全程不联网。

### 使用统计与崩溃上报（默认开启，可关闭）

六个平台都通过同一份共享实现向 `https://api.msime.app/v1/telemetry/events` POST 事件：`crates/client-core/src/telemetry.rs`（队列、安装 id、事件规则、发送与重试），原生宿主经 `crates/host-api` 的 `msime_client_telemetry_{begin,end,record_crash,flush,clear}`（`crates/host-api/src/ffi/reporting.rs`，声明在 `crates/host-api/include/msime_client.h`）调用它。**事件里没有任何输入内容、候选、剪贴板、账号标识或硬件信息**。

| | |
| --- | --- |
| 目的地 | `https://api.msime.app/v1/telemetry/events`，不带任何凭据（服务端和任何请求一样能看到来源 IP） |
| 偏好开关 | `usage_reporting`，默认 `true`。设置页「关于 → 隐私 → 匿名使用统计」在所有平台都显示（`packages/ui/src/settings/telemetry-section.tsx`）；Windows 原生设置在「关于」里另有同一开关（`platforms/windows/settings/main.cpp`），iOS 在「关于水杉 → 发送匿名使用统计」，Android 在「词库与输入 → 需要留意的 → 匿名使用统计」。旧字段 `telemetry_enabled` 不再读取 |
| 每条事件的字段 | 随机事件 id、`kind`、`platform`（`windows`/`macos`/`linux`/`android`/`ios`/`harmony`）、`version`（应用的真实版本号）、`install_id`；只有 `crash` 另带 `message` 与 `stack` |
| `install_id` | 每个安装首次使用时随机生成一次，存在本机的 `telemetry-state.json` 里，不由硬件、账号或任何用户数据派生 |
| 需要凭据 | 否 |

事件只有四种，客户端不再发送 `download`（GitHub Release 的下载数由服务端统计，装机数由 `active` 统计）：

- `active`：每个安装每个 UTC 日最多一条，id 固定为 `active-<install_id>-<yyyymmdd>`，重复发送也只记一次。
- `session`：输入法宿主进程正常结束时记一条（iOS 键盘是每次键盘弹出到收起），在下一次发送时送出。
- `session_crash`：上一个会话没有正常结束**并且**崩溃处理程序为它写下了崩溃记录时才记。只留下会话标记（注销、关机、系统杀进程、内存不足）不算崩溃，不产生任何事件。
- `crash`：崩溃处理程序在崩溃时**只写本地文件、不联网**，下次启动时才转成事件发送。`message` 是异常或信号的摘要（最多 1000 个字符），`stack` 是调用栈帧（最多 16000 个字符、12 KiB，在行边界截断）。共享层在入队前去掉每一行路径里的目录部分，只留文件名，所以用户名和文件夹名不会离开设备。每次启动最多转换 8 条崩溃记录。

发送与保留：事件先写进本机队列再发送，`202` 后移除；`400` 类拒绝直接丢弃不再重试；`429`（遵守 `Retry-After`）、`5xx` 和网络错误保留到下次用同一个 id 重发。队列最多 64 条，超出时丢弃最旧的。关闭开关后什么都不发送，并立即删除队列、会话标记和崩溃记录；`install_id` 文件保留，这样重新打开时不会把同一个安装算成两次。队列里格式无效的事件在读取时丢弃。

各平台的会话边界、崩溃来源和文件位置：

- **Windows**（`platforms/windows/src/entrypoints/server_main.cpp`，包装层 `platforms/common/Telemetry.cpp`）：会话是 Server 进程从启动到消息循环结束；开关读自偏好里的 `usage_reporting`（`platforms/windows/src/system/TelemetryConsent.h`：缺省算开启，显式 `false` 算关闭，读不出来的值或文件算关闭），设置页保存后立即生效。启动时发送一次，之后每 30 分钟一次。崩溃来源：`std::terminate` 记录异常类型和 `what()` 的第一行（JSON 解析异常只记类型和编号，因为其文本可能引用用户文件的内容）；未处理的结构化异常由 `SetUnhandledExceptionFilter` 记录异常名与代码、出错模块文件名+偏移和逐帧的模块+偏移（例如 `EXCEPTION_ACCESS_VIOLATION (0xc0000005) in module.dll+0x…`）。文件在 `%LOCALAPPDATA%\MSIME\` 下：`telemetry.json`、`telemetry-state.json`、`telemetry-session.json`、`telemetry-crashes\`。安装器的「联网功能」页说明了这项统计默认开启（`platforms/windows/installer/msime_setup.iss`）。
- **Linux**：IBus 宿主（`platforms/linux/src/entrypoints/ibus_main.cpp`）的会话是 `msime-linux-ibus` 从启动到主循环返回，崩溃守护以 `--recovered` 重启的进程同样如此；Fcitx5 插件（`platforms/linux/fcitx5/FcitxEngine.cpp`）的会话是插件实例的生存期，发送在后台任务里进行，不占用事件循环。崩溃来源：`std::terminate`（规则同 Windows），以及 SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT 的信号处理程序，写下 `SIGSEGV: segmentation fault (code N)` 这样的摘要和 `backtrace_symbols_fd` 的栈帧，然后交回原先的处理程序（Fcitx5 自己的崩溃日志照常工作）。启动时发送一次，之后每 30 分钟一次。开关是共享偏好 `preferences.json` 里的 `usage_reporting`。文件在 `$XDG_STATE_HOME/msime/`（未设时为 `~/.local/state/msime/`）下，文件名同 Windows；Fcitx5 用自己的子目录 `$XDG_STATE_HOME/msime/fcitx5/`，因此有自己的 `install_id`，同一台机器上两种框架都用会被算成两个安装。
- **macOS**（`platforms/macos/src/core/UsageReporting.mm`，由 `input_method_main.mm` 调用）：会话是输入法进程的生存期，单独打开的设置窗口不算。崩溃来源：`NSSetUncaughtExceptionHandler`（异常名、原因和 `callStackSymbols`）和 SIGSEGV、SIGBUS、SIGILL、SIGFPE、SIGABRT、SIGTRAP 的信号处理程序（`backtrace_symbols_fd`：二进制文件名、地址、符号+偏移）。启动时发送一次，之后每 3 小时一次。开关是共享设置页的「匿名使用统计」。文件在 `~/Library/Application Support/MSIME/telemetry/`。
- **iOS**（`platforms/ios/SharedUI/core/UsageReporting.swift`）：会话是键盘的一次弹出到收起（`KeyboardExtension/Sources/core/KeyboardUsageReporting.swift`），App 本身不产生会话。崩溃来源：键盘里的信号处理程序（`CrashSignalRecorder.c`，信号名和 `backtrace_symbols_fd` 的栈帧）和未捕获异常处理程序（异常名、原因和 `callStackSymbols`）；App 的崩溃来自系统的 MetricKit 诊断（`CrashDiagnostics.swift`），只取异常类型、代码、信号、ObjC 异常名、系统给出的终止原因和「二进制名+偏移」形式的栈帧，从不取 ObjC 的 `composedMessage`。键盘只有在「允许完全访问」打开时才发送，最多每 15 分钟一次；否则由 App 在回到前台时发送。文件在 App Group 容器的 `MSIME/telemetry/` 下，App 与键盘共用一个 `install_id`。`PrivacyInfo.xcprivacy` 声明了 CrashData、ProductInteraction 与 DeviceID（随机安装 id），均为不关联身份、不用于跟踪。
- **Android**（`platforms/android/java/app/msime/android/core/Telemetry.java`，经 `platforms/android/native/client_jni.cpp` 调用共享层）：会话只在 `:ime` 进程里，是 `MSIMEInputService` 从 `onCreate` 到 `onDestroy`。崩溃来源：两个进程的未捕获异常处理程序，记录 `Throwable.toString()` 的第一行（**可能包含异常自带的消息文本**）和 Java 栈帧（类、方法、源文件名与行号，最多 4 层 Caused by）；App 进程的崩溃在键盘下次启动时作为单独的 `crash` 发送。原生（信号）崩溃目前不捕获。App 打开时发送，键盘弹出时最多每 6 小时发送一次。文件在应用私有目录 `files/telemetry/`。
- **HarmonyOS**（`platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets`，经 `platforms/harmony/native/client_napi.cpp` 调用共享层）：会话是键盘进程从 `KeyboardExtensionAbility.onCreate` 到 `onDestroy`，设置应用只发送不产生会话。崩溃来源只有系统在下次启动时投递的 HiAppEvent `APP_CRASH`：JS 崩溃取错误名、消息第一行和引擎调用栈；原生崩溃取信号名与代码和「文件名+pc+符号」形式的栈帧，不含故障地址和目录。键盘启动时与之后每 6 小时（打字时检查）发送一次，设置应用打开时也发送。文件在 `files/state/telemetry/` 下，另有只存键盘进程号的 `harmony-keyboard-process.json`。

### 服务公告

设置窗口或 App 首页打开时，向 `GET https://api.msime.app/v1/notices?channel=app&platform=<平台>` 拉取一次公告列表，不带凭据，也不带安装 id；服务端缓存一分钟，客户端最快一分钟问一次，输入法进程从不在后台拉取。共享实现在 `crates/client-core/src/notices.rs`。拉取的地方：

- Windows、macOS、Linux 的共享设置窗口，以及 Tauri 版 Android/iOS 设置（`apps/desktop/src-tauri/src/notices.rs`，在设置页挂载时读取一次）；缓存与已关闭的 id（最多 200 个）存在共享状态目录的 `notices.json`。
- iOS App 的「设置」首页打开或回到前台时（`platforms/ios/SharedUI/core/AppNotices.swift`），存在 `Application Support/MSIME/notices`；键盘不拉取。
- Android App 的「设置」页出现时（`platforms/android/java/app/msime/android/home/NoticeBanner.java`），存在 `files/notices`；`:ime` 进程不拉取。
- HarmonyOS 设置窗口打开或回到前台时（`platforms/harmony/entry/src/main/ets/pages/Settings.ets`），存在 `files/state/notices`；键盘不拉取。

公告正文是简单 Markdown，在本机渲染，原始 HTML 不执行，不加载图片；正文里的链接只有在你点击时才用系统浏览器（或邮件应用）打开，且只接受 `http`、`https` 和 `mailto`。关闭某条公告只记在本机。

### 社区举报与审核

社区里看别人发布的皮肤、候选窗口皮肤、插件、词库和回复模板时，每一项都有「举报」入口。只有你选择理由并提交时，才会向 `POST https://api.msime.app/v1/community/reports` 发送 `{kind, item_id, reason, detail}`：`reason` 是固定的六个理由之一（侵权/抄袭、色情低俗、违法违规、垃圾广告、恶意插件、其他），`detail` 是你自己填写的可选说明（最多 1000 字）。请求带账号令牌，已登录用登录账号，否则用本机匿名账号。共享实现在 `crates/client-core/src/community/report.rs`。

社区采用先发布后审核：发布的内容立即公开，管理员可以下架。查看「我的作品」或自己作品详情时，请求带 `fields=moderation`，只用来在被下架的作品上显示「已下架」；不会显示下架理由，也没有待审核状态。

## 留在本地的东西

- **输入历史与学习数据**由 C++ Engine 管理，写在宿主提供的用户目录里，不上传。
- **剪贴板历史默认关闭**（`clipboard_history` 默认 `false`）。开启后写入状态目录下的 `clipboard_history.json`，仅本地；关闭时会清空该文件。
- **@ 名单**：用户在设置页「插件」里手动添加的名字和地点写在状态目录下的 `plugins/mentions.json`，仅本地，不读取通讯录或位置。它不在 `preferences.json` 里，所以宿主拷贝偏好和账号同步都不会带上它。「@ 名字与地点」模式默认关闭（`local_modes.mention` 默认 `false`）；从这个模式以及 V、/ 模式上屏的文字不进学习数据，也不计入打字统计。
- **设置**保存在应用数据目录的 `preferences.json`，可用绝对路径环境变量 `MSIME_CLIENT_STATE_DIR` 指向隔离目录。

## 没有的东西

没有第三方统计、用户行为分析或崩溃上报 SDK：Sentry、Mixpanel、Amplitude、Crashlytics、Google Analytics 及其等价物既不在依赖里，也不在代码里。全仓库唯一的上报路径是上面那条自己实现的[使用统计与崩溃上报](#使用统计与崩溃上报默认开启可关闭)。下面的命令命中的文件应当只有这几类：

- 共享实现与 C ABI：`crates/client-core/src/telemetry.rs`、`crates/client-core/src/telemetry/tests.rs`、`crates/client-core/src/lib.rs`、`crates/client-core/src/account/client.rs`（发送用的 HTTP 客户端）、`crates/client-core/src/notices.rs`（文档注释里提到它）、`crates/client-core/src/preferences.rs`（`usage_reporting` 开关）、`crates/host-api/include/msime_client.h`、`crates/host-api/src/ffi/reporting.rs`、`crates/host-api/src/tests.rs`。
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

发现本文档与代码不符，请按[贡献指南](CONTRIBUTING.md)提 issue；若涉及数据泄露或凭据问题，走[安全策略](SECURITY.md)的私下报告入口，不要开公开 issue。
