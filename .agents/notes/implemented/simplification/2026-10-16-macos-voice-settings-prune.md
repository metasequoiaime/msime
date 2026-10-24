# Agent Note: 精简语音输入设置页：桌面照鸿蒙手机收进「服务配置」子视图，macOS 固定空格锁定、提示音细分和录音后端，Windows、Linux、HarmonyOS 跟进

Status: implemented

桌面主页面不分标签的决定由[输入法与录音行为两个顶部标签](../feature/2026-10-19-voice-input-recording-tabs.md)部分取代；服务配置子视图及其余精简保持。

## Problem

macOS 设置窗口的「语音输入」页有 5 个标签页、约 43 个控件。多数不是没用，而是平铺和拆分：

- 同一个决定拆成几行：三个「按住录音」开关同一时刻只认一个（`VoiceHoldShortcut.h`）；豆包「流式接口」下拉和「识别接口地址」输入框写的是同一个 `asr_endpoint`；「录音后端」「读取录音设备」「可用录音设备」「麦克风设备」四行选的是同一个麦克风。
- 高级项和常用项在同一层：资源 ID、数字格式化、语义顺滑、热词表、结果提交策略、润色模型和接口地址。
- 几处在 macOS 上是错的：
  - 选豆包时显示一个空的「识别模型」输入框和「识别语言」，`DoubaoVoiceRequest.mm` 两个都不读。
  - 豆包资源 ID 为空时，原生录音回退到 `volc.bigasr.sauc.duration`，client-providers、设置页、Windows 都用 `volc.seedasr.sauc.duration`。设置页「测试」通过、实际录音失败。
  - 「麦克风设备」在 macOS 上显示 Linux 的 PulseAudio/ALSA 说明，「录音后端」的空选项写着 Linux 的「沿用服务设置」。
  - 存着别的平台写下的后端（比如 `pulse`）时，`InputController.mm` 直接拒绝录音，页面上只有一行灰掉的「（此平台不可用）」。

## Decision

### 页面排版（共享设置页，所有桌面宿主）

- 桌面语音页不再分标签，照[鸿蒙手机的整理](../feature/2026-10-12-harmony-phone-voice-page-tidy.md)分成主页面和「服务配置」子视图：
  - 主页面：页首说明；「识别」组（总开关、识别服务、「服务配置 ›」、识别语言、自动添加标点、边说边显示）；「语音快捷键」；「录音行为」（语音提示音、录音时静音其他声音）。
  - 子视图：「识别服务」（接入说明、豆包鉴权方式、旧版 App Key、密钥，然后「更多选项」里的接口地址、识别模型、资源 ID、数字格式化、语义顺滑、热词表；本地模型；检查按钮），「识别结果」（结果提交策略），「文本润色」，「提示音」（开始、结束，只在 `voice_cue_switches` 的宿主上），「录音设备」。
- 子视图在页面里换上来，不全屏覆盖，侧栏和标题栏照旧可用；主页面和子视图都保持挂载（两层 div，外层 `hidden`，内层排版），凭据草稿、润色输入和设备列表不因来回切换丢掉。离开语音页时子视图关上。
- 「服务配置 ›」右侧写状态（`desktopVoiceServiceStatus`）：macOS 和 Windows 的豆包没填密钥就拒绝录音，照实写「未填写」或「未填写完整」；HarmonyOS 2in1 的豆包这时改用系统识别，沿用手机的 `doubaoServiceStatus`；网络服务看当前服务的密钥，本地识别看有没有选模型。系统识别和 Linux（密钥由语音服务保管）不写状态。
- 用户改过接口地址或资源 ID（与出厂值不同）时，「更多选项」一打开就展开，免得改过的值藏起来。
- 「流式预编辑」改名「边说边显示」，与手机一致。
- Android 和 iOS 用同一份共享页时照旧一列排下来，不进子视图；鸿蒙手机的布局不变。

### 合并（共享设置页，所有宿主）

- 三个按住录音的开关合成一个「按住录音」（Windows、Linux、HarmonyOS 叫「长按录音」）选择，末项「不使用」。存储里同时开着几个时显示第一个打开的（右 Option、右 Control+右 Option、Control+Command 的顺序），渲染时不改写；用户改动时三个值一起写。空格锁定只在选了组合键时显示。Ctrl+F9 收进「更多选项」：Mac 键盘上 F9 是媒体键，还要「输入监控」授权。 组合键的顺序、平台名称与缺键默认值由[共享按住录音目录](2026-10-25-voice-hold-key-catalog.md)生成；默认值直接读取共享 Rust 偏好。
- 豆包的「流式接口」和「识别接口地址」合成一个下拉：两个预设和「自定义地址」，选自定义或存的地址不是预设时下面出现输入框。空地址按豆包的默认地址（双向流式）显示，不再显示成自定义。
- 选豆包时不画「识别模型」（各宿主的豆包识别都不读 `asr_model`）；HTTP 服务的识别模型收进「更多选项」。
- 选豆包时 macOS 和 Windows 不画「识别语言」。Linux 的豆包由单独运行的语音服务识别，HarmonyOS 的豆包没填凭据时改用系统识别，这两处照旧显示。`voice_input.language` 会上云（`settings_sync.rs`），只按条件隐藏，键和值都不动。
- 开始、结束提示音在总开关关着时变灰。

### 固定下来的值（macOS，`HostCapabilities` 三个新标志；其他平台见下面的「各平台跟进」）

- `voice_space_lock_switch`：macOS 为 false。`InputController.mm` 把 `MSIMEVoiceHoldShortcut::Options::spaceLock` 固定为 true，不再读 `MSIMEClientVoiceHotkeyHoldSpace`。
- `voice_cue_switches`：macOS 为 false。`MSIMEVoiceCueEnabled(defaults, start)` 换成只看总开关的 `MSIMEVoiceCuesEnabled(defaults)`。
- `voice_capture_backends`：macOS 为 false。设置页只画一个「麦克风」下拉：第一项「系统默认设备」，下面是读到的设备，子视图可见时自动读取列表。选中的设备拔掉后显示「已选的麦克风（未连接）」，选回系统默认设备即可恢复。宿主经 `MSIMEVoiceCaptureDeviceUID` 取设备：存着 macOS 不认识的后端时连同它的设备一起当作系统默认设备，照常录音，不再拒绝。
- 三个标志在 TS 里是可选字段，缺省按 true：只有明确报 false 的宿主收掉控件。
- 存储里缺某个按住组合键时，设置页按 client-core 的出厂值算（只有 `hotkey_ralt` 默认开），不再一律按开算，免得显示一个宿主上并没有生效的组合键。
- 设置页判断存着的后端是否有效时与宿主的 `MSIMEVoiceCaptureBackendSupported` 一样不分大小写。外来后端在麦克风下拉里单独占一项「系统默认设备（已忽略其他平台的录音设置）」，选「系统默认设备」才会把它和它的设备清掉；读取设备列表失败时，存着的设备显示「未连接」，不再停在「读取中」。
- 不迁移数据：共享文档里的 `start_sound`、`end_sound`、`hotkey_hold_space_lock`、`capture_backend` 原样保留，macOS 宿主不再读前三个，也不再把它们搬进 NSUserDefaults（`MSIMEApplySharedVoicePreferences`）或从 NSUserDefaults 写回共享文档（`MSIMEVoicePreferencesFromDefaults`，`sharedPreferencesByMerging:` 经它写回）。原生回退窗口的「按住说话时按空格锁定录音」开关、它的属性、探针和恢复默认用的键一起删掉。回退窗口本来就只有一个「播放提示音」和一个带「自动」的「录音设备」下拉，排版不用改；它的「录音设备」跟宿主对外来后端的处理对齐：存着外来后端时显示「自动」，在那里选了麦克风就把 `MSIMEClientVoiceCaptureBackend` 写成 `macos`（与共享页的麦克风下拉一样），仍是「自动」就后端和设备都不写（`MSIMEVoiceProviderCaptureBackendForSaving`）。否则回退窗口只写设备，选的麦克风会连同外来后端一起被忽略，录音悄悄用系统默认设备。

### Bug

- `DoubaoVoiceRequest.mm` 资源 ID 为空时回退到 `volc.seedasr.sauc.duration`，与 client-providers（`voice/provider.rs`）一致；先修这个，再把资源 ID 收进「更多选项」。
- 「沿用服务设置」只在 Linux 写；其他宿主的空后端写「系统默认」，设备说明不再落到 Linux 的文案。

### 各平台跟进（Windows、Linux、HarmonyOS）

macOS 这一批合入前，用户要求鸿蒙、Windows、Linux 也跟进。逐项看控件在不在这个平台上、macOS 的理由在这里成不成立，成立才带过去；三个标志改成：空格锁定和提示音细分只有 Android、iOS 还读存下的值（`voice_space_lock_switch`、`voice_cue_switches` 只在这两个平台为 true），录音后端只有 Linux 有得选（`voice_capture_backends` 在 macOS、Windows、HarmonyOS 为 false）。iOS 还报着提示音细分，所以标志不删。

- **Windows**：
  - 三个标志都为 false。Server（`server_main.cpp`）不再读 `hotkey_hold_space_lock`、`start_sound`、`end_sound`，`VoiceInputConfig` 里这三项保持默认的 true：空格只在按住录音组合键期间被拦下（`VoiceHotkeyPolicy.h`），与 macOS 同理。
  - 外来录音后端：`voice_capture_selection` 把 `pulse`、`macos` 这类后端连同设备当作没选，用系统默认设备录音，不再在 `VoiceInputSession.cpp` 里拒绝；后端名不分大小写，与设置页一致。空、`auto`、`windows` 在 Windows 上效果相同，所以和 macOS 一样只给「麦克风」下拉；选中的设备拔掉后 WASAPI 录音失败，「拔掉后无法录音」这句在 Windows 上也对。
  - 豆包资源 ID 为空时 Server 原来直接拒绝（`voice_doubao_start_message`）。存储里没有这个键时原生页「测试」按 seedasr 通过，是 macOS 那个 bug 的另一种样子；用户清空输入框存下 `""` 时，「测试」原样发出空值，client-providers 的 `credential/doubao.rs` 也报配置无效，两边碰巧一致。现在读配置时经 `doubao_resource_id` 回退到 `volc.seedasr.sauc.duration`，原生页的「测试」经 `doubao_test_resource_id` 同样回退，缺键和空值都按 seedasr 检查。
  - 原生 WinUI 页当时没有页内子视图或下钻控件，不另造一个，所以仍是一整页，按共享页的先后排，不画「服务配置」那一行。后来设置窗口有了页内标签框架，这一页改成「识别与输入」「录音行为」两个标签加「服务配置」子视图，与共享页相同，见 [Windows 设置的语音页标签与服务配置子视图](../feature/2026-10-20-windows-settings-voice-ai-mac-parity.md)。
  - 合并照共享页：「长按录音」下拉（含「不使用」，多开的配置显示第一个、不改写，换一项三个键一起写），空格锁定不给开关；Ctrl+F9 收进「更多选项」（沿用本地模型那个 `Expander`）。macOS 收起 Ctrl+F9 的理由（F9 是媒体键、要「输入监控」授权）在 Windows 上不成立，收起是因为共享页在 Windows 上已经这样排，原生页的约定是照着共享页的桌面分支画。录音行为只剩语音提示音和静音。识别服务配置的接口地址、识别模型、资源 ID 和豆包选项收进「更多选项」，改过地址或资源 ID 时一打开就展开；豆包的「流式接口」和地址合成一个下拉，空地址显示双向流式（`doubao_stream_endpoint_id` 以前把空地址当成自定义）。选豆包时不画识别模型和识别语言（`DoubaoAsrClient` 两个都不发）。
- **Linux**：
  - `voice_space_lock_switch`、`voice_cue_switches` 为 false：IBus（`ClientEngine.cpp`）和 Fcitx5（`FcitxEngine.cpp`）不再读 `hotkey_hold_space_lock`；两个输入法宿主（`VoiceProviderOptions.h`）和桌面语音面板（`voice.rs`）不再把 `start_sound`、`end_sound` 交给语音服务，服务没收到时按缺省都播放。服务照旧接受这两项，留给自行接入的客户端。
  - `voice_capture_backends` 保持 true，不带过去：Linux 的语音服务真在 PulseAudio、PipeWire、ALSA、自动之间选，空值是沿用服务自己的设置，设备名也要手填（`msime-linux-voice-provider` 的 `request_capture_command`），「只有一种录音方式」这个前提不成立。
  - 外来后端（从别的平台带来的 `macos`、`windows`、`harmony`）以前原样转发，服务报 `invalid capture backend`，录音失败。现在两处转发都连同设备一起不发，服务改用自己的录音设置；服务的协议对第三方照旧严格。设置页那一行「X（此平台不可用）」保留。后端名区分大小写，不照 macOS、Windows、HarmonyOS 那样不分：Linux 页面走的是「录音后端」下拉，存着的 `Pulse` 在那里本来就写「Pulse（此平台不可用）」，服务也只认小写，转发时同样丢掉，页面和录音结论一致；只在转发端改成不分大小写反而会让页面写不可用、录音却用 PulseAudio。
  - 识别语言照旧显示（见上面「合并」），资源 ID 不用修：`linux_provider_credentials.rs` 保存时丢掉空资源 ID，服务自己补 seedasr。
  - macOS 那批跳过的凭据块（`voice-credential-section.tsx`）也合并了：豆包的「流式接口」和接口地址合成一个控件，选「自定义地址」或存的地址不是预设时才露出输入框；其他服务照旧只有地址输入框。
- **HarmonyOS**：
  - 三个标志都为 false。2in1 接实体键盘时 `KeyboardSession` 把空格锁定固定为开；开始、结束提示音只跟总开关（`VoiceRecordingBehaviourPolicy`），手机和 2in1 一样，理由不依赖触屏。手机「服务配置」子视图里写死的「提示音」组按 `voice_cue_switches` 收起（门控，不删）。这和[手机语音页整理](../feature/2026-10-12-harmony-phone-voice-page-tidy.md)里「只改排版，不删功能」相反：那是早先的指示，这次用户要求各平台跟进。
  - 录音后端：空、`auto`、`harmony` 效果相同，选中的设备不在时 `HarmonyVoiceCaptureDevices` 退回系统默认设备，所以手机子视图和 2in1 都改成「麦克风」下拉，触屏上也少两行。说明另写：「选中的麦克风拔掉后改用系统默认设备；系统语音识别由服务自行取音，不受此项影响」，不照搬 macOS、Windows 的「拔掉后无法录音」。`VoiceCaptureDevicePolicy.selects` 改成不分大小写，以前存着 `Harmony` 时页面显示选中了设备、键盘却悄悄用系统默认设备。
  - 识别语言照旧显示。豆包的空接口地址以前被键盘拒绝（「请先配置有效的豆包接口、资源 ID 和凭据」），设置页「检查」却经 `providerSettingValue` 按默认地址检查，合并后的「流式接口」还把空地址显示成双向流式；空资源 ID 则两边都拒绝：键盘同上，「检查」经共享的 `asrServiceCredentialTestConfig` 原样发出 `""`，`Settings.ets` 的 `doubaoCredential` 报「请填写有效的 WSS 接口、资源 ID 及对应鉴权方式的凭据。」。现在键盘的 `DoubaoCredentialPolicy.resourceId`、`endpoint` 分别回退到 seedasr 和双向流式地址，`asrServiceCredentialTestConfig` 也把空资源 ID（缺键、`""` 或只有空白）回退到 seedasr，检查和录音的结论一致。这个共享函数也给 macOS、Windows 上的 Tauri 设置页用，那里的「测试」和宿主录音同样从此一致。
  - 手机页不画语音快捷键，按住录音的合并只在 2in1 上有效。
- **不带过去的**：macOS 原生回退窗口、`ShortcutTest.mm`、`LiveVoiceControllerTest.mm` 和 `DoubaoVoiceRequest.mm` 的改动只属于 macOS。

共享设置页中两种设备控件的展示与请求状态由[公共录音设备选择状态](2026-10-17-voice-device-picker-state.md)分层；原有平台能力、自动读取与显式刷新区别保持不变。

### 原样保留

- 豆包鉴权方式、旧版 App Key、接入说明按钮：默认服务商的必填流程，留在子视图里「更多选项」之外，和密钥在一起；Windows 原生页上也留在「更多选项」外面。

## Alternatives considered

- **只把高级项收进「更多选项」，保留五个标签页**（即[桌面标签页那篇](../feature/2026-10-13-desktop-input-expression-voice-tabs.md)里的语音分法）：改动最小，Windows 原生页也不用跟。不用它，是因为标签页本身就是拥挤的来源：用户要在五个标签之间找总开关、快捷键和麦克风，而鸿蒙手机已经用子视图验证过这套分法，仓库负责人批准过。
- **桌面也用全屏覆盖的子视图（同手机）**：代码现成。不用它，是因为桌面窗口的侧栏和自绘标题栏会被盖住，用户关不了窗口也换不了页。
- **空格锁定只藏开关，存下的 false 继续生效**：对关掉它的少数人最友好。不用它，是因为[九宫格数字设置的先例](2026-10-11-remove-number-keypad-settings.md)明确否决了「藏起来但仍生效」：改过的人从此改不回来。保留开关最强的理由是习惯按住组合键再打空格的人会被锁定，但空格只在按住组合键期间才被拦下，平时打字不受影响。
- **开始、结束提示音收进高级而不是固定**：保留了只想听一种提示音的配置。不用它，是因为两个开关在总开关之下只是微调，提示音细分在 macOS 上只剩总开关就够；其他宿主照旧读，留在子视图里。
- **录音后端整个删掉、不规整外来值**：最省事。不用它，是因为存着 `pulse` 这类值时 macOS 会拒绝录音，删掉后端控件后就没有界面能修好它，所以宿主先把外来后端当作系统默认设备。
- **麦克风下拉不带「系统默认设备」，只列设备**：列表更短。不用它，是因为选过的 AirPods 或 USB 麦克风拔掉后录音失败，没有默认项就回不去。
- **识别语言在所有宿主上随豆包隐藏**：更整齐。不用它，是因为 HarmonyOS 的豆包没填凭据时改用系统识别，Linux 的语音服务由用户管理，那里识别语言仍可能有用。
- **Windows 原生页也做一个页内子视图**：与共享页完全一致。不用它，是因为 `main.cpp` 没有这样的控件，要从头造一个下钻视图和返回逻辑；按共享页的先后排一整页，常用的几组仍在前面。
- **Linux 也只给「麦克风」下拉**：各平台一致。不用它，是因为 Linux 的语音服务在几种录音后端之间真有得选，空值还表示沿用服务自己的设置，收成一个下拉会丢掉这些选择。
- **主页面也放「麦克风」**：换耳机的人常用。不用它，是因为照鸿蒙手机的分法主页面只留最常用的几项，录音设备在子视图里和其他很少改的设置放在一起；快捷键例外地留在主页面，因为 macOS 的页首说明就是「用下方语音快捷键开始」。

## Consequences

- **收益**：macOS 语音页主页面从五个标签、约 43 个控件减到一屏左右；服务没配好时「服务配置」一行直接写着「未填写」。存着别的平台录音后端的配置不再录不了音；豆包默认资源 ID 与「测试」一致。
- **代价**：
  - 单独关掉开始或结束提示音、或关掉空格锁定的 macOS、Windows、Linux、HarmonyOS 用户，升级后会重新听到提示音、重新可以用空格锁定，且无处改回。
  - 同时开着几个按住组合键的配置，用户在页面上改一次就只剩一个，共享页和 Windows 原生页都一样。
  - 密钥、润色、录音设备都要多点一下「服务配置」。
  - Windows 原生语音页当时没有子视图，是一整页，密钥、润色、录音设备要往下翻。后来改成「识别与输入」「录音行为」两个标签加「服务配置」子视图，与共享页相同，密钥、润色、录音设备同样要多点一下「服务配置」，见 [Windows 设置的语音页标签与服务配置子视图](../feature/2026-10-20-windows-settings-voice-ai-mac-parity.md)。
  - `platforms/macos/tests/input/ShortcutTest.mm` 以前在 Control+Option+Space 和主快捷键测试前后把 `MSIMEClientVoiceHotkeyHoldSpace` 设为 NO 来隔离语音快捷键。这个值不再读了，那样设置不再隔离任何东西，所以改成在同一段里关掉三个按住录音的组合键（`MSIMEDisableVoiceHoldKeys` / `MSIMERestoreVoiceHoldKeys`）。
  - [桌面输入、标点与翻译、语音标签页](../feature/2026-10-13-desktop-input-expression-voice-tabs.md)里「语音分为识别服务、录音与结果、润色、快捷键、录音设备」那一句被本篇取代。那篇原文保留，加了一行指向本篇；输入页和标点与翻译页的标签不受影响。

## Verification

- 共享设置页：`apps/desktop` 下 `pnpm typecheck` 通过；rebase 到 develop 后全量 `vitest run` 657 个文件中 653 个通过，另四个（`settings-persistence-load-reuse.test.ts`、`settings-persistence-save-generation-reuse.test.ts` 读临时缓存时 ENOENT，`community-candidate-skins.test.tsx`、`translation-settings.test.tsx` 等待超时，都与语音页无关）是在负载均值 200 以上时失败，单独重跑 106 个用例全部通过。新增或改写的用例覆盖：主页面和子视图各有哪些组、返回后回到主页面（`voice-settings-copy.test.tsx`）；macOS 不画空格锁定、提示音细分、录音后端，豆包不画识别语言和识别模型，麦克风只在子视图可见时读取；HTTP 服务的识别模型和接口地址在「更多选项」里；`desktopVoiceServiceStatus` 各分支；按住录音的选择、多开配置显示第一个且不写、改动时三个一起写（`voice-hotkeys-section.test.tsx`）；流式接口和自定义地址合成一个控件、空地址显示默认（`doubao-stream-endpoint*.test.tsx`）；缺的组合键按出厂值算（`{hotkey_ralt:false}` 显示「不使用」）；麦克风下拉的系统默认设备、未连接设备、外来后端单独一项且选系统默认能清掉、后端不分大小写、读取失败不再显示「读取中」（`voice-capture-devices-section.test.tsx`）；先改识别语言再换成豆包，保存时语言照样写进 `voice_input`（`settings.test.tsx`）；2in1 照旧显示识别语言和手机的状态文案。
- client-core：`host_surface` 新增 `macos_alone_fixes_space_lock_cue_split_and_capture_backend`。
- macOS 原生：`ShortcutTest.mm` 改为关掉三个按住组合键来隔离语音快捷键；`SharedVoicePreferencesTest.mm`（提示音只看总开关、三个键不再双向搬运、外来后端的设备当作系统默认）、`HTTPVoiceControllerTest.mm`、`LiveVoiceControllerTest.mm`（外来后端照常录音、交给采集的设备为 nil）、`doubao_voice_fixture.py`（未写资源 ID 时请求头是 seedasr）随改动更新；`VoiceProviderSettingsKeysTest.mm` 新增回退窗口在外来后端下显示「自动」、选了麦克风把后端写成 `macos`、仍是「自动」时两个都不写。这台机器没有编译原生宿主，这些只由 CI 编译和运行。
- 各平台跟进：`host_surface` 的测试改成 `only_mobile_reads_space_lock_and_cue_split_and_only_linux_picks_capture_backend`，逐个平台核对三个标志（`rbuild cargo test -p msime-client-core host_surface` 和 clippy 通过）。共享页的用例改为 Windows、Linux 不给空格锁定开关，Windows 和 HarmonyOS 的录音设备只有「麦克风」，HarmonyOS 的说明不写「无法录音」，手机子视图没有「提示音」组，2in1 不给空格锁定开关，Linux 凭据块的流式接口只在自定义时露出地址输入框；`voice-provider-defaults.test.tsx` 核对 `asrServiceCredentialTestConfig` 把缺键、`""` 和只有空白的资源 ID 都按 seedasr 发出。HarmonyOS 的 `keyboard-logic.test.ts`（提示音只看总开关、后端不分大小写、豆包空资源 ID 和空地址的回退）在本机用 node 跑过。Windows 的 `tests/ui/voice_settings.cpp`（长按录音的选择和写法、空地址显示双向流式、「更多选项」何时展开、后端不分大小写、「测试」的空资源 ID 回退）、`tests/voice/voice_capture_selection.cpp`（外来后端当作系统默认设备）、`tests/voice/voice_session_policy.cpp`（空资源 ID 回退）和 Linux 的 `tests/voice/voice_provider_options.cpp` 在本机用 clang 编译运行通过；`main.cpp`、`server_main.cpp`、IBus、Fcitx5、ArkTS 宿主和 Tauri 的 `voice.rs` 测试只由 CI 编译。
- 没有启动设置应用或输入法核对实际排版和录音。
