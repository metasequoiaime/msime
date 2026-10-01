# 网络请求与数据流向

这份文档说明**本仓库的代码**在什么条件下发出什么网络请求、内容是什么、去哪里、默认是否开启，以及在哪一行代码里。

它不是隐私政策。产品的隐私政策在 <https://msime.app/privacy/>，设置页「关于」里也有入口（`packages/ui/src/index.tsx`）；Linux 的「隐私政策」入口直接打开本文档，与 Windows 参考实现链接它自带的 PRIVACY.md 一致。两者冲突时以隐私政策为准，但 Linux 除外：msime.app/privacy/ 的 Linux 部分目前与这个宿主不符（它写的是不检查更新、凭据存进 Secret Service，而实际会检查更新、服务商凭据存在 0600 权限的文件里），在网站更正之前，Linux 的实际行为以本文档中可对照代码的描述为准。这里提供的是可以对着代码核对的技术细节，给审阅者和贡献者用。

输入法是能看到你输入的一切的软件，所以这个问题值得一个能逐条验证的答案，而不是一句「我们重视你的隐私」。

## 一句话结论

默认配置下，**只有一个功能会把你输入的内容发出设备：云联想**。它默认开启，把当前正在组的拼音串发给 Google 输入工具；Windows、Linux 与 macOS 在第一次使用时先问（见[云联想](#云联想默认开启)），macOS 在回答之前不发请求。其余所有联网功能——语音识别、语音润色、AI 联想、账号同步——默认凭据为空，你不填自己的密钥它们就不会发出任何请求。候选翻译默认不联网，需要你在设置里选择一个翻译服务（自己的凭据，或显式选择「水杉账号」）。

另有一条不携带输入内容的自有上报路径，端点是本项目自己的 `https://api.msime.app`：macOS、iOS、Android、Linux、HarmonyOS 会在启动时发一次安装计数，其中除 HarmonyOS 外还会在进程崩溃时发送异常信息，这五个平台默认开启且没有开关；Windows 的同一条路径**默认关闭**，只有在设置页「关于」里打开「匿名使用统计」（`telemetry_enabled`）之后才发送启动与崩溃事件。去重、调用栈和离线重试这三件事逐平台不同，不要按「六个平台一样」理解，差异逐条列在[安装与崩溃上报](#安装与崩溃上报)。仓库不接入任何第三方统计或崩溃上报 SDK。

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

### 候选翻译（默认不联网，需要你选择服务）

`candidate_translations` 默认 `true`，支持腾讯机器翻译（`https://tmt.tencentcloudapi.com`）、小牛翻译（`https://api.niutrans.com/v2/text/translate`）和自定义端点。三者都要求你在设置里填入自己的 API 凭据，默认全为空字符串——**没有凭据就不会发出请求**，开关为真也一样。发送内容是待翻译的候选词。代码在 `crates/client-core/src/credential/translation.rs`。

macOS、iOS、Android 和 Linux 另提供「水杉账号」（`translation_account`，默认 `false`）：只有你在翻译服务里显式选择它，才会把当前页的中文候选词（包括本地已有释义的）连同目标语言代码 POST 到 `https://api.msime.app/v1/translate`。请求带账号令牌：macOS 与 Android 在你已登录时用登录的账号，否则（以及 iOS 上始终）用一个匿名账号；Linux 在安装后的用户初始化流程中自动创建并保存本机匿名身份，候选翻译仍只有在显式选择该服务后才发送。你自己的服务优先：候选翻译关闭、小牛或自定义服务已启用、或腾讯已启用且两项凭据都可用时，都不走水杉账号。共享层把这个判定算成翻译查询里的 `translation_account` 字段（`crates/host-api/src/ffi/providers.rs`），各宿主只在它为真时发请求。Windows、HarmonyOS 没有这条路径。没有选择任何服务时不发出候选翻译请求。

macOS 26 及以上在没有选择任何服务时（候选翻译开启，小牛、自定义、腾讯都未启用，也没有选择水杉账号），会用 Apple 系统自带的离线翻译模型为随包词典答不上的中文候选补一行释义。翻译在本机完成，候选词不离开这台 Mac；只用你已经在「系统设置 → 通用 → 语言与地区 → 翻译语言」里下载好的语言对，输入法不会触发下载，没下载就不补。代码在 `platforms/macos/src/backend/translation/BackendOnDeviceGloss.swift`，判定在 `InputController.mm` 的 `currentOnDeviceGlossRequest`。

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

`https://api.msime.app`，定义在 `crates/client-core/src/account.rs` 的 `ACCOUNT_ORIGIN`。普通账号不登录不发生。Linux 安装包会在 Debian `postinst` 为可联系的登录用户注册匿名账号；手工安装或当时网络不可用时，`msime-linux-setup` 会在首次配置时重试。候选翻译只有在选择了「水杉账号」后才发送，见[候选翻译](#候选翻译默认不联网需要你选择服务)；Android 浏览社区皮肤与词库目录时，也会先取匿名账号的令牌（`platforms/android/java/app/msime/android/community/CommunityCatalog.java`），取不到照常列出目录。macOS 输入法激活时不创建账号。Linux 的匿名身份和令牌由 `msime-linux-online-provider` 保存在用户配置目录的 `anonymous-account.json` 与 `anonymous-session.json`，两个文件均为当前用户专用权限，不进入设置页或输入法进程。凭据存放在系统密钥库：macOS/iOS 用 Keychain（`crates/host-macos/native/account.mm`、`crates/tauri-mobile-platform/ios/Sources/MobilePlatformPlugin.swift`），Android 用 Keystore 加密后落盘。

### 云剪贴板（需要登录）

用来在手机和电脑之间传文字：一台设备上把文字放进云剪贴板，另一台设备上从云剪贴板里点选。条目存在 `https://api.msime.app` 你的账号下，最多 50 条，单条不超过 4000 个 UTF-16 单元。

- **只上传你明确选择的文字**：在云剪贴板面板里输入或粘贴后点上传，或者在本地剪贴板历史的某一条上点「发到云剪贴板」。任何平台都不会自动读取系统剪贴板，复制时也不会自动上传。
- **只在打开面板时读取**：设置页、键盘里的「云端」栏和 macOS 输入法菜单里的「云剪贴板…」在打开和点刷新时各拉取一次列表，没有轮询或推送。
- **密码框里不出现**：Android、iOS、HarmonyOS 键盘和 Linux Fcitx5 在密码类输入框里不显示云端条目，也不发请求；macOS 在安全输入期间不打开云剪贴板面板。
- **关闭即删除**：把云剪贴板关掉会删除云端全部条目，共享设置页在关闭前会先请你确认。
- **键盘怎么拿到账号**：键盘和设置应用是不同的进程，账号令牌不会被复制成明文文件。Android 键盘向主进程里一个不导出的 ContentProvider 取令牌，只有主进程会刷新令牌；iOS 的会话存放在 App 与键盘扩展共有的 App Group 钥匙串访问组里，键盘需要「允许完全访问」才会联网；HarmonyOS 键盘读取设置应用在应用私有目录里保存的同一份会话。iOS 与 HarmonyOS 刷新令牌时持有跨进程文件锁，防止两个进程用同一个刷新令牌而让会话被服务端吊销。
- **从电脑上屏**：Windows 与 Linux 的设置窗口里，云剪贴板面板只有在这次打开时记下了另一个应用的输入窗口才能直接上屏，否则只能复制；它不会沿用以前记下的窗口。

### 资源与更新下载

首次准备词库时从 GitHub Releases 拉取固定版本的资源，地址、长度和 SHA-256 全部写死在 `resources/desktop-dictionary.lock.json` 里，逐一校验，全部成功才发布到内容标识目录。下载的是公开发布物，不上传任何东西。检查更新只在点击「检查更新」时进行，向 `https://api.github.com/repos/metasequoiaime/msime/releases` 发起 GET 请求并在本地按平台标签前缀筛选（识别不出宿主平台时改为读取 `https://msime.app/update.json`）；请求除 IP 地址和防缓存时间戳外不携带标识，适用 GitHub 隐私条款。

Android 的手写识别使用 ML Kit，**首次使用需要联网下载识别模型**，之后在设备上离线识别。Linux 与桌面端使用 Engine 随附的离线 Zinnia 模型，从安装路径读取，全程不联网。

### 安装与崩溃上报

六个平台都实现了向 `https://api.msime.app/v1/telemetry/events` POST 事件的路径：Windows 默认关闭、要用户在设置里打开，其余五个平台默认开启。**事件里没有任何输入内容、候选、剪贴板或账号标识**，字段只有事件 id、类型、平台名、版本号，崩溃事件另带异常信息，其中两个平台还带调用栈。

| | |
| --- | --- |
| 目的地 | `https://api.msime.app/v1/telemetry/events` |
| `kind: "download"` | 启动计数。字段：随机 id、平台名、版本号。**只有 Apple 与 Android 做了本地去重**，各自只在首次启动发一次；Linux（IBus 宿主）、Windows（开关打开时）、HarmonyOS 的宿主进程每次拉起都发一次（Linux 上 launcher 在崩溃后重启的宿主除外），且 id 每次随机，服务端无法合并，所以这三个平台上它实际是启动计数而不是装机计数 |
| `kind: "crash"` | 进程崩溃。异常信息截断到 2048 字节。**调用栈只有 Apple 与 Android 有**（截断到 12000 字节）；Linux 与 Windows 的崩溃事件没有 stack 字段，message 是固定字面量 `"std::terminate"`，不携带真实异常信息；HarmonyOS 没有崩溃上报 |
| 需要凭据 | 否 |
| 偏好开关 | Windows：`telemetry_enabled`，默认 `false`，即设置页「关于」里的「匿名使用统计」，设置页只在 Windows 上显示它。其余五个平台**没有**，这条路径不读任何偏好字段 |

实现分三份，能力逐份不同：

- Apple（macOS 输入法进程、iOS 主 App）：`shared/backend/account/BackendTelemetryClient.swift`。macOS 由 `platforms/macos/src/input/input_method_main.mm` 在启动时 `dlsym` 调起，iOS 由 `platforms/ios/App/Sources/MetasequoiaImeApp.swift` 的 `init` 调起；两者同时注册 `NSSetUncaughtExceptionHandler`。首次启动以 `UserDefaults` 的 `msime.telemetry.firstLaunchRecorded` 去重。
- Linux 与 Windows（C++ 宿主进程）：`platforms/common/Telemetry.cpp`，没有首次启动去重。download 事件的 JSON 只有 id（随机十六进制串）、kind、platform（`"linux"` 或 `"windows"`）、version 四个字段，version 是构建时写入的发布版本：Linux 为 CMake 的 `MSIME_LINUX_VERSION`，与 `.deb`/`.tar.gz` 的包版本相同（发布工作流取 `platforms/linux/version.txt`），Windows 为 `MSIME_WINDOWS_VERSION`。Windows 由 `platforms/windows/src/entrypoints/server_main.cpp` 在读到已保存的偏好之后判断 `telemetry_enabled`（`platforms/windows/src/system/TelemetryConsent.h`，只有显式的 `true` 算开启，缺省、类型不对都算关闭）：开启时另起一个不等待的后台线程发送启动事件，端点慢或不可达都不会拖住 Server 启动；关闭时既不发送，也不写 `telemetry.json`。崩溃回调在 `wmain` 开头注册，但只在开关当时为开时才上报，设置页保存后立即按新值生效；启动事件则在 Server 下一次启动时按当时的值决定。Linux 只有 IBus 宿主 `msime-linux-ibus` 上报：`platforms/linux/src/entrypoints/ibus_main.cpp` 在向 ibus-daemon 注册 component 成功之后，另起一个不等待的后台线程发送，所以端点慢或不可达都不会拖住输入法注册；每次由 ibus-daemon 或用户拉起的宿主发一次，`msime-linux-ibus-launcher` 的崩溃守护重启的宿主（带 `--recovered`）不发。Fcitx5 插件（`msime-fcitx5`）不链接 `Telemetry.cpp`，既不发启动事件也不发崩溃事件。崩溃走 `std::set_terminate`，回调传的 message 是写死的 `"std::terminate"`，既没有异常内容也没有调用栈——`crash()` 组装的 JSON 只有 id、kind、platform、version、message 五个字段；Linux 上崩溃守护重启的宿主同样注册这个回调。
- Android 与 HarmonyOS：`platforms/android/java/app/msime/android/core/Telemetry.java`（由 `HomeActivity` 调起，`SharedPreferences` 的 `first-launch` 去重，`Thread.setDefaultUncaughtExceptionHandler` 捕获崩溃）和 `platforms/harmony/entry/src/main/ets/telemetry/Telemetry.ets`（由 `EntryAbility.onCreate` 调起，只有装机事件，没有崩溃捕获，也没有本地去重）。

事件都是先落盘再发送、发送成功才从队列移除，但**补发只有 Apple 与 Android 做了**：这两份实现在每次启动时遍历整个队列重试，所以离线期间攒下的事件联网后会补发。Linux 与 Windows 只落盘、只尝试发当前这一条，从不回头读队列里的历史事件——离线时写进 `telemetry.json` 的事件会一直留在那里，直到被 64 条上限挤掉，永远不会补发。HarmonyOS 连落盘都没有，`Telemetry.ets` 直接发一次 HTTP 请求，失败即丢弃。队列上限 64 条，文件位置：Apple 为应用支持目录下的 `MSIME/telemetry-events.json`（iOS 放在 App Group 容器里，文件权限 0600）、Windows 为 `%LOCALAPPDATA%\MSIME\telemetry.json`、Linux 为 `$XDG_STATE_HOME/msime/telemetry.json`（未设时为 `~/.local/state/msime/telemetry.json`）、Android 在 `msime-telemetry` 这个 SharedPreferences 里。在 Windows 上保持「匿名使用统计」关闭（出厂即关闭）就不会发送；从此前版本升级的 Windows 用户，旧版本已写下的 `telemetry.json` 会留在原处，关闭开关不会删除它，也不会补发。其余平台想让它彻底不发，目前只能在构建时去掉调用点，或者在网络层阻断该端点。

## 留在本地的东西

- **输入历史与学习数据**由 C++ Engine 管理，写在宿主提供的用户目录里，不上传。
- **剪贴板历史默认关闭**（`clipboard_history` 默认 `false`）。开启后写入状态目录下的 `clipboard_history.json`，仅本地；关闭时会清空该文件。
- **@ 名单**：用户在设置页「插件」里手动添加的名字和地点写在状态目录下的 `plugins/mentions.json`，仅本地，不读取通讯录或位置。它不在 `preferences.json` 里，所以宿主拷贝偏好和账号同步都不会带上它。「@ 名字与地点」模式默认关闭（`local_modes.mention` 默认 `false`）；从这个模式以及 V、/ 模式上屏的文字不进学习数据，也不计入打字统计。
- **设置**保存在应用数据目录的 `preferences.json`，可用绝对路径环境变量 `MSIME_CLIENT_STATE_DIR` 指向隔离目录。

## 没有的东西

没有第三方统计、用户行为分析或崩溃上报 SDK：Sentry、Mixpanel、Amplitude、Crashlytics、Google Analytics 及其等价物既不在依赖里，也不在代码里。全仓库唯一的上报路径是上面那条自己实现的[安装与崩溃上报](#安装与崩溃上报)，命中的文件应当只有它的那几份实现、调用点与构建配置（`platforms/linux/CMakeLists.txt`、`platforms/windows/CMakeLists.txt`），外加描述它的 `platforms/linux/README.md`、Windows 开关所在的偏好定义与设置页（`crates/client-core/src/preferences.rs`、`packages/ui/src/index.tsx`）和钉住它的测试（`shared/backend/Tests/BackendTelemetryClientTests.swift`、`platforms/linux/tests/core/ibus_startup_telemetry.py`、`platforms/windows/tests/runtime/telemetry_consent.cpp`、`crates/client-core/src/preferences/tests.rs`、`apps/desktop/tests/settings/settings.test.tsx`）：

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
