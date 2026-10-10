# HarmonyOS 功能迁移对照

iOS 与 macOS 的同类文档是 [ios-parity.md](ios-parity.md) 和 [macos-parity.md](macos-parity.md)，方法一致：先做存在性比对定位可疑区域，再按行为逐条下钻。本文记录用什么方法比过、发现了什么，便于以后复核而不是重新发明一套。

逐文件的落点清单在 [harmony-feature-inventory.md](harmony-feature-inventory.md)：来源的 145 个产品源文件，一个不漏地写明在这个仓库里落到了哪里。那份记「列全」，这份记「怎么比的、发现了什么」。

## 范围与基线

迁移的目标是 MSIME-Apple 的完整功能，而不是能编译的子集。公共业务放共享层、公共管理界面放 Tauri；输入算法与组合状态仍归 C++ Engine；平台特性按 HarmonyOS 自身的机制适配，不照搬来源的实现形态。

比对在固定对象上进行，便于复核：

- 来源：`metasequoiaime/MSIME-Apple`，提交 `b93f169839c442cfa7034f3130c3dfaac11b9467`。
- 目标：`metasequoiaime/msime` 的 `develop`。

来源入口是该检出的 `platforms/ios/` 与 `shared/`。HarmonyOS 的对照面是 `platforms/harmony/entry/src/main/ets`、`apps/harmony/src` 与它渲染的 `packages/ui/src`——鸿蒙的设置界面就是那份共享 React 页，所以对照面必须把它算进来，只比 ArkTS 会把一整层功能误判成缺失。

## 比对轴一：客户端能力面

最有信号的一条轴不是文件也不是符号，而是 `SettingsClient`：共享设置页按宿主声明的能力决定画什么，所以「别的宿主提供而 Harmony 不提供的成员」就是「同一份页面在鸿蒙上少的那几块」。差集要把三方都算进来——`packages/ui/src/index.tsx` 的 `SettingsClient` 成员、桌面宿主、以及 `apps/desktop/src/core/mobile-host-services.ts`（Android 与 iOS 两个移动宿主共用）——只拿移动宿主做基线会漏掉桌面独有的 20 个成员。

鸿蒙没有提供的 21 个成员逐条分类如下，**没有一个是 MSIME-Apple iOS 有而鸿蒙缺的功能**：

| 成员 | 判定 |
| --- | --- |
| `appIcon` | 本平台无公开 API，见下文「不迁移的」 |
| `windowControl`、`beginWindowDrag`、`resizeWindow`、`onWindowStateChanged` | 共享页自绘标题栏与缩放把手，由 `window_chrome` 能力门控，仅桌面 |
| `restartInputMethod`、`installInputSource`、`uninstallInputSource` | 重启输入法与安装输入源；`restart_input_method` 能力对鸿蒙为 false，后两个是 macOS 的 IMK bundle |
| `loadMacosShuangpinKeymap` | macOS 升级前存在原生 defaults 域里的双拼键位提示开关，只在共享偏好 `shuangpin_keymap_hint` 还没有值时读来显示；开关本身已并入共享偏好，由 `shuangpin_keymap_hint` 能力门控（macOS、Windows） |
| `openScreenKeyboard`、`openHandwriting`、`openVoice` | 桌面把这三个开成独立面板窗口；本宿主它们是键盘自己的面（`SURFACE_*`） |
| `pickVoiceModelPath` | 按路径加载本地语音模型；本宿主用系统识别器或 HTTP provider |
| `resetLearnedData` | 共享 C ABI 对移动端明确返回 `learned-data reset is unavailable on mobile` |
| `clipboard` | 设置页里的剪贴板历史列表。iOS 与 Android 也都不提供——Apple 的剪贴板管理在键盘的 `KeyboardClipboardView` 里，对应本宿主的 `SURFACE_CLIPBOARD` |
| `resolveFontFamilies` | 候选字体预览的字族解析。iOS 的 `candidate_font_controls` 为 false，根本不显示候选字体控件；本宿主与 Android 一致 |
| `loadDefaultPreferences`、`openThirdPartyLicenses` | 「恢复默认设置」与第三方许可。MSIME-Apple 的 iOS 上都不存在（iOS 的「恢复默认」只作用于皮肤设计和键盘布局），属桌面功能 |

这条轴的好处是它不会因为措辞不同而误报，坏处是它只看得见「页面级」的缺失：一整块没有会被抓到，一个区块里少一节不会。

## 比对轴二：中文文案

抽取来源全部非测试源文件里含汉字的字符串字面量，逐条在鸿蒙宿主与共享 UI 的语料中精确检索。

| | 数量 |
| --- | --- |
| 来源产品文案（已排除仅出现在测试文件中的 281 条） | 1373 |
| 精确未命中 | 729 |
| 其中含 Swift 插值 `\(…)`，按构造不可能命中 TypeScript 语料 | 108 |
| 其余纯字面量 | 621 |
| 纯字面量中只出现在键盘侧（`KeyboardExtension` / `SharedUI`） | 125 |

**这个比率本身没有意义，得说清楚为什么。** iOS 那条轴的未命中是 161/1666，因为 iOS 直接复用同一份 Tauri 页，文案是同一批字符串；鸿蒙的键盘界面是自己的 ArkTS，设置页虽然共用但说明文字由不同的人分别写过，所以措辞不同是常态而不是异常。抽样 30 条应用侧未命中逐条核对，全部是已实现功能的不同措辞（云词库、社区、皮肤编辑器、AI 皮肤、对话、个人词库）。

这条轴的价值不在比率，而在于**它会指出哪些「节」在目标里完全没有对应概念**。它指出的一处是词库页的「词库信息」：来源 `FeatureSettingsViews` 在词库页写明装的是哪套词库——规格、上游提交、以及它随应用更新而不单独下载。数据本来就在设备上（随应用安装的词库带着一份清单，鸿蒙已经把它作为引擎资源暂存），缺的只是一条读出来的路。那一节加在共享页而不是鸿蒙自己的界面里：每个能回答这个问题的宿主都该显示它。

## 比对轴三：键盘侧逐面板

按面板把来源的键盘扩展与鸿蒙的 `KeyboardView` 对起来。名称匹配在这条轴上没用——鸿蒙几乎每个都改了名（`CandidateTranslationStore` → `TranslationPolicy`、`EmojiCatalog` → `EmojiCatalogModel`、`HandwritingInputView` → `HandwritingStrokePolicy`），所以是按功能对。

| 来源面板 | 鸿蒙 | 结论 |
| --- | --- | --- |
| Symbol / Emoji / Clipboard / Scheme / Skin / More / Tool | `SURFACE_*` 十余个面与 `symbols` 切换 | 已覆盖；布局选择并入方案选择器，与共享 `touch_keyboard_schemes` 模型一致 |
| `JapaneseNineKeyView`、`HandwritingInputView` | 日语面与手写面 | 已覆盖 |
| `KeyboardAIView`（AI 润色） | 润色面（`AiPolishPolicy.ts` + `KeyboardView.polishFace`） | 已覆盖。来源润色的是**选区**，而 HarmonyOS 不给输入法读取选区的能力（`InputClient` 只有光标前后的文本，选区只以下标通知），照抄手势等于画一个不知道自己在操作什么的按钮，所以改为润色光标前的文字 |
| `KeyboardVoiceView` | 无，且不该有 | 来源那一套是 iOS 的变通：键盘扩展不能录音，所以 App 录完把文字交接给键盘（`VoiceTextHandoffStore`、「发送到键盘」「等待键盘插入」整组文案）。鸿蒙键盘自己能录音，这一整个交接面不存在是正确的 |
| `HandwritingInputView` 的模型下载流程 | 无，且不该有 | 同上：来源要联网下载 Google 手写模型，鸿蒙用系统 Core Vision Kit OCR，没有模型可下 |

这条轴找到的另一处是无障碍标签：来源 `KeyboardViewController` 有 55 处 `accessibilityLabel`，而 `LetterKeyFacePolicy.accessibilityLabel`、`EnglishLetterCaseState.accessibilityLabel/accessibilityValue`、`JapaneseVariantPolicy.accessibilityLabel`、`CandidateGlossPolicy.accessibilitySuffix` 四份标签策略在鸿蒙侧都已经移植过来、都有单测，调用方却只有它们自己的测试——标签算出来了，从没挂到任何控件上。抓到它靠的是从渲染那一端反过来数：不是问「标签在不在」，而是问「有没有人把标签挂上去」。这条已接线，并由下面那道符号可达门禁看着。

## 比对轴四：来源自己的测试断言

前三条轴比的都是「有没有这个东西」——页面、文案、面板、调用点。全都通过之后仍有一类东西看不见：**同一个东西在两边行为不同**。文件在、接线在、测试绿，但做的事不一样。

这一轴的做法是把 MSIME-Apple 自己的测试套件当规格读：`platforms/ios/{KeyboardTests,ServiceTests,tests}` 与 `shared/backend/Tests` 共 232 个 `func test*`、1276 条断言，函数名本身就是行为规格；挑出属于键盘宿主（而非共享设置 UI、而非 Engine）的那些，逐个去鸿蒙侧找对应实现，对不上的再读来源的测试体。成本高于前三条，但它是唯一能看见「行为不同」的。

绝大多数直接对得上：`CandidateChipsNeverWrapToASecondLine` 对 `CandidateWrapPolicy`、`SpaceCursorMovementAccumulatesDistanceAndReverses` 对 `SpaceCursorMovement`、`WubiCandidatesCarryAndShowTheCodeLeftToType` 对 `WubiCodeHintPolicy`、`SchemeGridHasFourColumnsAtNarrowAndWideSizes` 在装机运行的截图里就是四列。对不上的逐条查实并修掉，全部是「三道机械门禁都抓不到」的那种——文件在、接线在、单测绿，缺的是一个本该发生却没发生的行为：

| 来源用例 | 暴露的分叉 | 现在的行为 |
| --- | --- | --- |
| `testLatinFieldsUseFullKeyboardAndRestoreNineKeyHeight` | `EditorPolicy.prefersLatin` 有、也接了线，但 `onEditorChanged` 只改语言不改键面：九宫格用户点进邮箱或网址框，得到的是在 3×3 的 T9 网格上打英文 | 按编辑器类型同时切键面与语言，离开时恢复九宫格高度 |
| `NineKeyInputAndLayoutSwitches`、`ShiftIsDiscoverableAndSwitchesToEnglishCapitalization`、`SymbolKeyOpensAPanelInsteadOfAMenu` | 动作键那一行写在 26 键分支内部，三块自绘键面都没有它——九宫格和日语假名格没有空格、没有回车、退不出英文 | 动作行由三块键面共用的 `actionRow()` 生成 |
| `NineKeyDigitLayerKeepsTheGridInsteadOfTheTwentySixKeyRows`、日语的 `DigitLayerKeepsTheSameThreeColumnGrid` | 按 `123` 会掉进十列符号排 | 数字层保持三列网格，第十二格是括号键 |
| `JapaneseNineKeyTests` | 展开候选按内容无限增宽 | 单行且不越界 |
| `HandwritingTests` | 每笔后立即 OCR，且识别期间丢弃后续触摸；来源是 550 ms 停笔去抖 | 停笔去抖 + 最新快照串行识别 |
| `KeyboardAITests` | 关闭面板只丢本地 generation，会话仍缓存晚到结果，销毁后回调未解除 | 关闭、取消、清空与模式切换都按 generation 作废晚到结果 |
| `KeyboardSkinTests` | 渐变、照片、图案、键帽形状/材质/阴影全被解析，真实 ArkUI 键盘只用了颜色和字体 | 自定义皮肤完整进入原生键盘渲染 |
| `CustomServiceTests`（模型目录） | 设置页把 `provider` 交给 Harmony，原生模型目录桥却全按 Bearer、单页和 `/v1/models` 请求，丢掉 Anthropic 的 `x-api-key`/分页、Gemini 的 `/v1beta/openai/models`、EveryAPI 的能力过滤 | 协议差异由 `AiModelCatalogPolicy` 明确建模 |
| `CustomServiceTests`（语音 provider） | 语音设置公开 EveryAPI 与 Mistral，键盘运行时只允许 OpenAI、SiliconFlow、Groq | 五个 OpenAI-compatible 批量转写 provider 共用一条经过 HTTPS、模型与分槽凭据校验的路径 |
| `BackendAccountClientTests` | 词库变更已要求 64 位十六进制资源 ID，云剪贴板删除却接受任意非空字符串——虽经 URL 编码不构成路径穿越，但会把账户 token 带到一个本地即可判错的请求 | 两者共用同一资源 ID 校验，非法 ID 不进入 transport |
| `BackendAccountSessionTests` | 只保存 `refresh_token` 却从不使用：access token 到期就把仍可续期的账号当成退出 | 所有受保护请求共用同一状态机——到期前 30 秒刷新、并发请求共享一次刷新、保存旋转后的 refresh token、401/403 后只刷新重试一次，并以 session generation 阻止晚到登录或刷新在退出后复活账号；公开社区浏览在可选 session 失效时退回匿名 |
| 词典后端四组测试 | 取消固定位置的 DELETE 仍携带 `position: null`；云词库导出把整段文本封进 JSON 送过 WebView；冲突页会把畸形空页当成「没有变化」 | 省略该字段并保留服务端 context；导出按来源的 384 MiB 上限用 `requestInStream` 写沙箱临时文件、校验 NUL/媒体类型/`Content-Length` 与实际字节数后交给系统保存选择器；冲突页拒绝不前进或互相矛盾的 `next`/`has_more` 游标 |
| 快照恢复用例 | 共享 UI 会发 `snapshot_restore_preview` 与 `snapshot_restore_native`，Harmony 桥一个都不处理，而 `snapshotNative: true` 又让 WebView 刻意不读文件，恢复必然返回 `snapshot_unavailable` | 系统选择器选一份 `.ndjson`，检查 1–512 MiB 后复制到应用私有目录，由原生检查器验证 framing、字段、跨记录一致性、正文 checksum 与整文件 SHA-256；确认时 N-API 在 worker 线程调用 C ABI，C ABI 紧邻上传重新检查私有文件与整文件摘要，再流式 PUT，512 MiB 内容不进入 ArkUI 线程、WebView JSON 或整块内存 |
| `KeyboardSurfaceUITests.testReplyKeyboardPastesChoosesStyleAndInserts` | 回复键面只能粘贴、选风格并生成，缺「帮你回／帮润色」两种模式、源文字删除与清空、取消生成、换一句、重新选择风格 | `ReplyKeyboardPolicy` 生成两种提示语，回复上下文 generation 使取消、清空与模式切换丢弃迟到结果；模板请求保留自定义提示语路径 |
| `WelcomeUITests.testMainTabsKeepIndependentNavigation` | 四个底部 tab 只复刻了外观和入口，没有各自独立的导航栈：从「键盘」进「输入」，切去社区再回来会被重置到首页 | 共享 UI 仍是扁平路由以适配 WebView，但按 tab 记住各自最后一个叶页面，浏览器返回恢复页面时同步更新该记录 |
| `WelcomeUITests.testChatLoginIsFocusedAndCancelReturnsToTryout` | `AccountPage` 只把 Android/iOS 判作移动平台，Harmony 显示桌面账号操作；未登录的共享 AI 对话把输入框禁用了，而来源允许先唤起键盘、保留草稿，登录只是发送的前置条件 | 三种触屏宿主共用移动资料页；试用页聚焦可编辑输入框，从对话进入登录时带着返回来源，取消或登录成功都回到原对话页 |
| `WelcomeUITests.testReplayedWelcomeStartRespondsOutsideText` | 设置宿主没把共享账号页的「重新查看新手引导」接回引导状态，而且把自己的平台身份硬编码成 Android，首次设置逐字显示 Android 系统说明；重播后「稍后设置」也不返回发起位置 | 共享引导显式识别 HarmonyOS，保留该平台确实提供的「打开系统设置」与「选择输入法」两个动作；账号页重播入口重新显示同一引导；三个移动宿主重挂载时读取并白名单校验已有移动页 |
| `TypingStatisticsTests` 的自动清理断言 | 共享 `TypingStatisticsStore` 与 C ABI 已支持 `set_retention`，Harmony React 客户端与 ArkTS 请求白名单都没有这项，大屏统计页缺 30/90/180/365 天清理策略 | 该动作从共享 UI 直达同一份原生聚合存储；手机统计页保持触屏版，不显示这个桌面维护控件 |

形态判断也是这条轴带出来的：键盘宿主早已按 `deviceInfo.deviceType` 区分手机软键盘与 2-in-1 候选窗，设置 WebView 却只看 `platform === harmony`。共享宿主能力现在显式声明 `mobile_settings`，Harmony 在 ArkTS 桥中按真实设备形态覆盖它——手机继续用底部主导航、触屏预览与移动资料页，2-in-1 改用大屏侧栏、物理键盘预览与桌面资料编辑，平台名只留给平台文案，不再代替形态判断。

## 不迁移的：应用图标切换

来源 `AppIconSettingsView` / 共享页的 `SettingsClient.appIcon`。**本平台没有公开 API**，核查记录在 [platforms/harmony/README.md](../platforms/harmony/README.md)：`@ohos.bundle.bundleManager` 对自身只读，`@ohos.bundle.shortcutManager` 只管快捷方式可见性，`api/` 与 `kits/` 下没有 `setAbilityEnabled`，也没有任何形式的 alternate/dynamic icon。

按能力模型处理：宿主不声明，页面不画那个控件。这是裁剪，不是欠账。

## 存在性比对看不见什么，以及接手它的门禁

三条存在性轴各有盲区：能力面只看得见页面级的缺失，看不见区块里少一节；文案比对会被措辞差异淹没，产出是线索而不是清单；逐面板比对能找到整块缺失，但对「挂上了没有」无能为力。

「移植一份策略、给它写好单测、然后忘记接线」这一类，三道检查都会是绿的，因为套件自己 import 那个模块。它现在由两道棘轮式门禁机械化地看着，都在 `scripts/verify-local.sh` 里：

- `scripts/test-harmony-unwired-policies.py` 从导入方向问一个测试套件结构上问不出的问题——*除了测试之外，有没有东西到得了这个文件*。当前输出：123 个宿主模块全部可达。
- `scripts/test-harmony-unwired-symbols.py` 把同一判据下沉到方法这一层：除声明处和测试之外，没有东西按 `类名.方法(` 的形式引用它。它有两张名单，区别就是全部意义所在——`ALLOWED` 是本平台永远不会调用的，每条写明是什么差异；`PENDING` 是应该接而尚未接的，必须写明背后的缺口**具体是什么**，诊断清楚才能进。两张都是棘轮：名单上的名字一旦变得可达就报错，没上名单的新符号也直接报错。当前输出：297 个宿主静态方法，11 条平台差异，**0 条待接**。

`ALLOWED` 里的条目长这样，说明这张名单不是「以后再说」的堆放处：`CandidateGlossPolicy.token`/`isCurrent` 的移植版是 session/generation/epoch 三字段，而本宿主传 `(handle, epoch)`、epoch 由 `TranslationPolicy.signature` 铸出、那个签名本身就带 `generation`，第三个字段在构造上已经在第二个里面了，接上去等于把 generation 比两遍；`FloatingToolbarLayout.allComponents` 是「全部打开」而不是本宿主的默认——准备好的工具栏记录里 `screen_keyboard` 是 false，因为鸿蒙自带屏幕键盘，接上去会多出一个没人要的按钮。

写这个符号扫描踩过两个会让数字偏低的坑，判据因此是现在这样：按裸方法名 `.method(` 匹配会撞名（`CandidateGlossPolicy.isCurrent` 一度显示为已接线，只因为 `HarmonyDoubaoRecognizer` 有个同名私有方法），所以按类名限定；按「文件里第一个导出类」归属方法也是错的（`ClipboardHistoryStore.ts` 导出六个类），所以按类体范围归属。

同一类的另外两道：`scripts/test-harmony-bridge-parity.py` 比的是「页面会调用的桥方法」与「真正注册出去的名字」，而不是两边都有没有这个符号；`scripts/test-harmony-settings-bundle.py` 确认 `apps/harmony` 的单文件设置页没有被重新提交进仓库、仍能构建成单文件——它由 `platforms/harmony/stage-settings.sh` 在每次打 HAP 前构建，不会比同一份检出里的 `packages/ui/src` 旧。

ArkTS 本身也有一道：`tsc`、逻辑套件、设置包构建全绿并不代表 HAP 打得出来，因为 `tests/run.sh` 只编译 `.ts` 不编译 `.ets`，而 ArkTS 的 `@Builder`/`build` 体内不得声明局部变量、对象字面量必须对应已声明接口等限制只有真打包才抓得到。`scripts/test-harmony-arkts-subset.py` 用文本扫描抓其中能抓的两类；装好 DevEco 命令行工具、`oh_modules` 与三 ABI 原生库之后，`verify-local.sh` 会直接跑一次完整的 `hvigorw assembleHap`。

逻辑套件本身用 `bash platforms/harmony/tests/run.sh` 跑，当前 1608 条断言。

## 来源的断言会给出假阳性，只有设备能判

`ReturnKeyAction.shouldPerformEditorAction` 的理由看着很硬：键面对不可执行的动作已经显示「换行」，而 `submit()` 无条件 `sendKeyFunction(enterKey)`——那不就是键上写着换行、按下去不换行吗。

接上去在真实输入框里一测，两个字符挤在同一行：`insertText("\n")` 被 WebView 忽略了。退回原样再测同一个框（`enter=8`，即 `ENTER_KEY_TYPE_NEW_LINE`），换行**正常出现**。也就是说 `sendKeyFunction` 拿到 enter key type 之后框架自己就把换行做了，Android 需要宿主二选一的那个分叉在这里不存在，接上去才是回归。

这条因此记在 `ALLOWED` 里，理由写的是实测而不是推断。教训是这条轴的性质：来源的断言说明**存在这样一条规则**，但不说明**这条规则该由谁实现**。平台已经实现了的，照搬过来就是回归。

## 系统集成

鸿蒙宿主作为系统输入法完整可用，装机运行的验收记录与复现步骤在 [platforms/harmony/README.md](../platforms/harmony/README.md)：系统接受本输入法并拉起扩展进程；`ArkTS → NAPI → Rust → C++ Engine` 整条链打通；共享 React 界面逐页渲染（欢迎流程、首页、词库页、账号页、社区页）；键盘在本应用和第三方应用里各验证过一次输入——`nihao` → 候选 `你好` → 上屏；回车键分别读作「前往」和「搜索」，都取自各自编辑器声明的动作，组合进行中变为「选定」；词库页显示的规格与提交号与锁文件一致。2-in-1 形态上安装、启用、切换为当前输入法同样验证通过，那是硬件键盘相关功能（模式和弦、语音快捷键、维护和弦）的目标形态。

也记下一个只有设备会告诉你的操作事实：`ime -e <bundle>` 默认进 `BASIC_MODE`，而那个模式下框架不会创建面板、扩展的 ArkTS 完全不运行，且没有任何错误提示；必须 `-f`。
