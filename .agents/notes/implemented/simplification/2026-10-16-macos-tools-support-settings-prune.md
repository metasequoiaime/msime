# Agent Note: 精简 macOS 的工具与支持类设置页（插件、AI 辅助、打字统计、剪贴板、账号、维护与诊断、帮助与反馈、关于）

Status: implemented

## Problem

macOS 设置里「工具」「账号」「支持」几组页面上的控件，很多不是用户真要做的决定：同一个入口出现两次（云词库、打开数据目录）、只有出错时才用得上的恢复按钮一直摆着（刷新登录方式、清除失效登录状态）、页面已经会自动做的事又给一个按钮（刷新统计、从系统剪贴板同步）、开发者用的块平铺在主页面上（MCP 连接、AI 润色测试）、一组里没有任何控件（macOS 的「云剪贴板」组），以及两项几乎看不出效果的微调（AI 候选数量、效果强度）。另有两处开了不起作用或说法不对：没选音乐包时打开「背景音乐」什么也不放（宿主要求开关打开并且选了包，`key_sound.rs` 的 `music: enabled && !pack.is_empty()`）；AI「自定义提示词」写着「额外的提示词」，实际会整个替换内置提示词，而内置提示词带着 JSON 输出约定（`ai.rs` 的 `DEFAULT_CANDIDATE_PROMPT`），写一句短指令就可能让联想不出结果。

调研见本轮的 macOS 设置项精简报告（第 2–6 节，插件、AI、账号、支持各页的核查结论）。共享 React 设置页同时服务 Windows、Linux 和三个触屏宿主，Windows 还有一套照它逐页移植的原生 WinUI 设置窗口。第一轮只决定 macOS；随后的跟进把同样的精简带到 Windows（共享页和原生窗口）、Linux 和 HarmonyOS，逐项按「控件在那个平台上存在、macOS 的理由在那里也成立」决定，见「各平台跟进」。

## Decision

**改变取值的砍除按宿主声明，宿主自己不再读存下的值。**

- client-core 的 `HostPlatform` 新增 `fixed_ai_candidate_limit()`（macOS 为 3）和 `fixed_effect_intensity()`（macOS 为 50），`HostCapabilities` 新增同名的 `Option<u8>` 字段，写法照 `fixed_candidate_page_size`：按偏好的宿主不序列化这两个键。固定的值等于新档案的默认值，单测钉住这一点。
- host-api 按编译到的平台（`compiled_platform()`）取值：`host_ai_candidate_limit` 用在会话创建时和偏好更新时的 AI 座位配置、在线查询，以及 `msime_client_ai_request_for_query` 建请求时的配置上，三处同一个值，`chat_completion_http_request` 对「请求与配置数量一致」的检查照常成立；`SoundSettings::new` 的效果强度同样取固定值。macOS 的 `InputController` 本来就从在线查询读 `candidate_limit`（含 AI 缓存键），不需要改；`TypingEffectPanel` 的 `applyPreferences:` 不再读 `plugins.effect_intensity`，固定为 `kDefaultIntensity`，特效包的强度仍由 `applySettings:` 换上。
- 设置页由 `settingsCapabilities` 的 `showAiCandidateLimit`、`showEffectIntensity` 决定是否画这两行；没有宿主时（网页预览）照样画。
- 原生回退窗口 `AISettingsWindow.mm` 去掉「候选数量」一行和它的校验，`MSIMEAISettingsMerge` 不再把 `candidate_limit` 当作自己管的键，保存别的字段时文档里的数量原样保留，不会被回退窗口写回。`AppearancePreferences.mm` 没有这两项，`sharedPreferencesByMerging:` 也不写它们。
- 偏好字段、校验、云同步映射都不动；没有数据迁移，存着别的值的文档在 macOS 上不再被读，在 Android、iOS 上照旧生效。

**其余是共享页的布局改动，对所有宿主一样。**

- 插件 › 声音与效果：成就音效和升档音收进组里的「更多音效」折叠（`MoreOptions` 加了可选的 `title`，默认仍是「更多选项」）；打字统计关着时「成就音效」不能打开并说明要先在「打字统计」页打开记录，但已经打开的仍然可以关掉；升档音只在宿主播放音效包（`key_sound`）时出现，它放的就是音效包的上屏采样。打开「背景音乐」时还没选包，就替用户选一个随带的音乐包（`musicPackToSelect`：先按 client-services `BUILTIN_MUSIC_PACKS` 的顺序找随带的，再取目录里第一个；没有包存储的宿主选随带的第一个）；目录读到了却一个音乐包都没有时开关不能打开。脚本插件详情的「执行顺序」只在至少两个脚本在运行（或这个排在第 8 个之后不会运行）时出现。插件页进入时读一次打字统计开关，用来决定成就音效能不能打开。
- AI 辅助：「获取模型列表」和「自定义提示词」收进「服务」组已有的「更多选项」；三个槽位的「提示词方案」下拉撤掉，只剩一个绑定当前 `prompt_id` 槽位的文本框，`prompt_id` 和其他槽位原样保留，`customPromptSlotOptions` 仍给语音润色用；说明改为「填写后会整个替换内置提示词，不是追加」，并提醒要求只输出 JSON。「AI 润色测试」只在 iOS、Android 和 HarmonyOS（手机与 2-in-1，`KeyboardSession.ets` 用这份配置润色）上保留，macOS、Windows、Linux 三个桌面上撤掉。macOS 不画「联想」组（里面只有候选数量）。
- 打字统计：桌面版撤掉「刷新统计」按钮，页面每 5 秒和窗口回到前台时自动重读；手机溢出菜单里的「刷新统计」保留（Android 设备冒烟 `TypingStatisticsDeviceSmoke` 还在按它）。macOS 不再给统计页的「打开数据目录」（`main.tsx` 不注入 `openDirectory`），它和「维护与诊断 › 在 Finder 中显示」打开的是同一个偏好目录；Linux、Windows 照旧。
- 剪贴板：macOS 的「云剪贴板」组撤掉，那句「请从输入法菜单中的『云剪贴板…』打开」挪进一直显示的「剪贴板」组（`ClipboardHistorySection` 的 `note`）。「从系统剪贴板同步」按钮撤掉，改为保存的 `clipboard_history` 从关变成开的那一次自动调用一次 `sync`；进入页面或本来就开着时不同步。Windows 原生剪贴板页的同步按钮走同一个 `sync_clipboard_history`，不受影响。
- 账号：桌面的「云端」组去掉「云词库」一行（入口留在「词库」页，不登录也看得到），只剩云剪贴板，macOS 上因此整组不出现；触屏宿主「我的」里的云词库不动。「刷新登录方式」只在没有可用的登录方式（读失败或一个都没开）时出现；「清除失效登录状态」只在读不出本机登录状态（`status()` 失败）时出现，清掉以后消失。登录说明在没有设置同步（`settingsSync`）的桌面上不再许诺「同步设置」。
- 维护与诊断：macOS 的「重新注册当前输入源」和「安装或更新水杉输入源」合成一个「修复输入源」，先 `installInputSource` 再 `restartInputMethod`。安装一步改走和启动检查同一套版本比较的 `macos_input_source::repair`：没装时安装、随附的更新时替换，从不降级；与启动检查不同的是版本相同也重装一遍，因为 Info.plist 读得出版本不代表可执行文件、框架和签名完好（清理工具删掉了一部分、只拷了一半），原来的「安装 / 更新」会覆盖这样的 bundle，修复不能少了这一步；不是打包运行、没有随附输入源（`SourceUnavailable`）和 bundle 已留在原处只等登记（`RegistrationPending`）都不算失败。安装真的失败时照样重新登记已装的那份，登记完再报失败。原来「安装 / 更新」那条不比较版本的强制重装（`macos_input_source::install`）随之删掉，它没有别的调用方。启动时安装失败的提示改为指向「修复」。「连接 AI 助手」整块移到新的子页 `mcp`（`subPageParents.mcp = "developer"`，只在宿主报告 `mcpServerStatus` 时存在），维护页上只剩进入它的一行。
- 帮助与反馈并进关于（桌面）：注册表新增 `desktopMergedPages = { feedback: "about" }`。`settingsNavGroups` 不变，触屏宿主、Windows 原生窗口的 `SettingsNavigation.h` 和 `main.tsx` 的 `onNativeSettingsPage` 都照旧认 `feedback` 和 `help`；桌面侧栏不列出被并进的页，`index.tsx` 把桌面上请求的 `feedback` 显示成 `about`，`help` 的上级随之点亮「关于」、返回按钮写「返回关于」。「关于」在桌面（含 HarmonyOS 2in1）上画 `FeedbackSections`（帮助入口、问题报告、反馈渠道），桌面上不再挂一份藏起来的反馈页；手机保留单独的反馈页和应用内表单。
- 页名：`desktopPageTitle` 让 macOS 的账号页叫「账号」（设置同步只接在 Windows 的账号会话上），其他桌面仍叫「账号与同步」；`settingsPageProjections` 的 `desktopPageTitle` 选项缺省时用注册表里的名字，触屏宿主仍由 `mobilePageTitle` 改写。页面留在侧栏。

`settingsCapabilities` 里算出来却没人用的 `showDesktopMaintenanceShortcuts` 不动：它的使用方（快捷键页的维护快捷键一组）是 `252562694a` 有意删掉的，不是丢失，接回去不在这次的决定里。

## 各平台跟进

macOS 的决定逐项搬到 Windows、Linux 和 HarmonyOS，判据是控件在那个平台上存在、macOS 的理由在那里也成立；触屏宿主的桌面理由（候选窗口外观、鼠标驱动的面板）通常不成立。砍除照样是能力字段加宿主不读存值，不做数据迁移，会同步到云端的值不硬砍。

**固定取值**

- AI 候选数量：`HostPlatform::fixed_ai_candidate_limit` 扩到 macOS、Windows、Linux、HarmonyOS，都是 3。共享的 engine/input-runtime 只让第一条 AI 回答坐第一页最后一格（[AI 候选坐第一页最后一格](../feature/2026-10-16-ai-candidate-last-slot.md)），四个平台上多出来的回答都排到列表末尾；鸿蒙的候选条横向滚动，末尾更难看到。宿主都从 host-api 取：Windows TSF 的 `AiCandidateWorker` 读在线查询的 `candidate_limit`，Linux 的 fcitx5 和 IBus 转发 `msime_client_online_query`、`msime-linux-online-provider` 照查询里的数量请求，鸿蒙的 `KeyboardSession` 用在线查询的数量并经 `msime_client_ai_request_for_query` 建请求，三处都在 `host_ai_candidate_limit` 之后。只有 Linux 的 IBus 宿主要改：`online_dispatch` 发请求前把偏好快照里的 `ai_assistant` 和在线查询里的逐键比较，有一键不等就把 AI 配置从查询里删掉（防推迟应用的偏好改动带着旧设置发请求），而 `candidate_limit` 原在比较之列；快照里是文档存的原值，查询里是固定的 3，存着 1 或 5 的用户从此一个 AI 请求都发不出去。比较里去掉了这一键，固定值本来也不会随推迟的偏好改动变化；`scripts/test-linux-ibus-ai-candidate-limit.py` 在 Linux 固定数量时拒绝它回到比较里。fcitx5 没有这道比较。共享页靠 `showAiCandidateLimit` 藏起「联想」组；Windows 原生 AI 页删掉「联想」组和只剩它一个调用方的 `ais::candidate_limit`。这一项不在 `settings_sync.rs` 里，不同步。
- 效果强度：`fixed_effect_intensity` 扩到 Windows 和 HarmonyOS，都是 50。Windows 画的三种样式和 macOS 一样，特效包也会换掉强度；鸿蒙把三种样式都画成亮度不同的卡片闪光，样式本身已经是强度的粗调，理由比 macOS 更充分。Windows 的 host-api 由 `SoundSettings::new` 取固定值；Server 原来还自己从偏好文档读 `plugins.effect_intensity` 交给候选窗口（会话发布特效设置之前用），这条路一起去掉，候选窗口在那之前用默认强度 50，特效包的强度照旧随按键发布。鸿蒙不经 host-api 读强度，`TypingEffectPolicy.settings` 改为固定 `DEFAULT_INTENSITY`，特效包的强度仍由 `resolve` 换上。Windows 原生插件页删掉「效果强度」一行，`PluginPageModel.h` 的 `effect_intensity` 和读写照旧，存着的值原样往返。Linux 不画样式，只在辅助行里显示连击数，没有强度可调，仍是 `None`，能力文档不变。
- Android 和 iOS 这一轮没有跟进，AI 候选数量和效果强度照旧按偏好；`scripts/test-reference-config-coverage.py` 仍能在共享页上找到 `ai_assistant.candidate_limit`，就是靠这两个宿主。

**Windows 原生设置窗口**（共享页在 Windows 上已经是新布局，这里补齐原生 WinUI 窗口）

- AI 辅助：获取模型列表和当前槽位的「自定义提示词」收进「服务」组已有的「更多选项」（沿用页面上的 `Expander`，搜索落到里面的行时展开）；「提示词方案」下拉撤掉，`PromptSlot` 只留槽位名，说明换成共享页那句（`ais::prompt_description`）。「AI 润色测试」撤掉，`build_ai_test_tools`、`run_ai_test` 和它们的状态、`ais::polish_test_prompt`、`ais::test_failure` 随之删掉：Windows 的语音润色用的是 `voice_input.polish_*`，不是这份配置。
- 插件 › 声音与效果：成就音效和升档音收进按键音效组末尾的「更多音效」`Expander`，升档音说明补上要先打开连击计数；进入插件页时经 `msime_client_typing_statistics` 读一次统计开关，统计关着时成就音效不能打开（已经打开的仍可关掉，`pg::achievements_blocked`）。打开背景音乐时还没选包就按 `pg::music_pack_to_select`（照 `musicPackToSelect`）选一个随带的音乐包，目录里一个音乐包都没有时开关不能打开；原生窗口原来只写 `music_enabled`，打开了也不出声。脚本插件详情的「执行顺序」改用 `pg::script_order_shown`。
- 打字统计：撤掉「刷新统计」（页面本来就每 5 秒和窗口回到前台时重读）和「打开数据目录」。后者在原生窗口里与「维护与诊断 › 数据目录」的「打开」是同一个 `state_directory()`，macOS 的理由成立；第一轮说「Windows 上只有这一个入口」只对 Windows 上的共享页成立，那里没有维护页的打开按钮（`openDiagnosticLogDirectory` 只注入给 macOS），所以 `main.tsx` 照旧给 `openDirectory`。
- 剪贴板：撤掉「从系统剪贴板同步」，保存的 `clipboard_history` 从关变成开、并且存上时自动 `sync` 一次。Server 的 `ClipboardMonitor`（`AddClipboardFormatListener`）只记打开以后的复制，按钮补的正是打开前剪贴板上那一条，和 macOS 一样。原生窗口的「云剪贴板」组有真的「打开云剪贴板」按钮，不动。
- 账号：「云端」组去掉云词库（「词库」页有同一个入口）；「登录状态」组只在没有可用登录方式时给「刷新」、只在读不出本机登录状态时给「清除失效登录状态」，两样都不需要时整组不出现。登录说明许诺的设置同步在 Windows 上是真的，不改；侧栏本来就叫「账号」。
- 帮助与反馈并进关于：`SettingsNavigation.h` 去掉 `feedback`（手写页已并入屏幕键盘页后，侧栏共 16 页；与共享桌面侧栏即 `settingsNavGroups` 减去 `desktopMergedPages` 一致），`help`、`feedback` 路由改为打开「关于」，托盘和工具栏菜单仍发这两个路由，落到「关于」。「关于」在「版本与更新」之后画帮助、问题报告和反馈渠道（`build_feedback_sections`）。本窗口没有子页，帮助内容仍直接画出，所以原生「关于」比共享页长。

**Linux**

- IBus 宿主的 AI 设置比较去掉 `candidate_limit`，否则固定数量以后存着别的值的用户发不出 AI 请求，见上面「固定取值」。
- 共享页的布局改动在 Linux 上已经生效，共享页这一轮只改两处判断。「升档音」原来跟着 `effectStyles` 藏起，可 Linux 的 host-api 在 `msime_client_key` 里算连击、升档时经 kira/cpal 播放音效包的上屏音，开关有效果却改不到；现在只看宿主有没有打字效果。账号页改叫「账号」：Linux 有账号页却没有设置同步（`main.tsx` 只给 Windows 接 `settingsSync`），「账号与同步」名不副实，这正是 macOS 改名的理由。
- 「打开数据目录」保留：共享页上维护页的「打开」只注入给 macOS，Linux 上统计页这一个按钮是唯一能打开数据目录的地方。
- `platforms/linux/README.md` 不再说设置页给出「AI 润色测试」；provider 的 `ai_test` 请求和 Tauri 的 `ai_test` 命令留着。

**HarmonyOS**

- 「AI 润色测试」保留：鸿蒙回复键盘的「帮润色」（`KeyboardSession`、`ReplyKeyboardPolicy`）用的就是这份 `ai_assistant` 配置，macOS 的理由在这里不成立，共享页的桌面判断本来就把它留给鸿蒙的手机和 2in1。
- 账号页保持「账号与同步」：2in1 注入了 `settingsSync`；手机由 `mobilePageTitle` 叫「我的」。手机溢出菜单里的「刷新统计」同 Android 一样保留。
- 插件页只在 2in1 上有，成就音效的统计判断、背景音乐选包、更多音效和帮助与反馈并进关于都随共享页生效；脚本插件、`mcp` 子页、修复输入源在鸿蒙上不存在。

**有意没有带过去的**

- Windows 原生窗口的「连接 AI 助手」（MCP）留在维护页上，不做成子页：原生窗口没有子页导航，维护页也很短（两个日志开关、数据目录、导出导入），子页要新加一套导航状态，而标题为「更多选项」的折叠会把这一页的主要内容说成次要选项。Windows 上的共享页已经有 `mcp` 子页。
- 「修复输入源」只有 macOS 有：Windows 和 Linux 没有安装输入源的控件，鸿蒙没有维护页的这一组。
- 剪贴板页「云剪贴板」组的改动只针对 macOS 那个只有一句说明的组；Windows、Linux、鸿蒙那里有真的按钮。

## Alternatives considered

- **只藏起 AI 候选数量和效果强度这两行，宿主照旧读存下的值** — 改动最小，核查组也指出两个值继续生效「无害」（效果样式仍能关掉效果，数量已经限制在 1–10）。不用它，是因为 [撤掉数字键面设置](2026-10-11-remove-number-keypad-settings.md) 已经否决过这种做法：改过的人从此改不回来，同一个值在 macOS 上看不见却在起作用。
- **在页面里按 `macosPlatform` 判断要不要画这两行** — 一行代码就够。不用它，是因为取值固定是宿主的事实，host-api 必须按同一个事实取值，否则设置页和宿主各说各的；能力字段让 host-api、设置页和测试读同一个 `HostPlatform` 方法。
- **成就音效、升档音直接砍成 `false`** — 插件组核查认为它们是三层嵌套的小玩法。不用它，因为两者默认就是关，「固定成关」等于删掉功能：音效包里的成就采样和 `host.rs` 的里程碑计算都会变成死代码；真正的问题是依赖没有显示出来，所以收进「更多音效」并在统计关着时禁用。
- **背景音乐：没选包时禁用开关，而不是替用户选一个** — 不替用户做选择、也不会突然出声。不用它，是因为安装包随带两个音乐包（`BUILTIN_MUSIC_PACKS`，macOS 的 bundle 在 `platforms/macos/CMakeLists.txt` 里把 `resources/sound-packs` 一起打包），用户打开开关就是想听，禁用开关只会让人去插件列表里找包再回来；只有一个音乐包都没有时才禁用。
- **剪贴板历史打开时（还没保存）就同步** — 反馈更快。不用它，是因为宿主读的是保存后的 `clipboard_history`，没保存时同步会把用户可能马上取消的开关先当真，原来的按钮在保存前也是禁用的。
- **把「帮助与反馈」从 `settingsNavGroups` 删掉，所有宿主一起合并** — 一处改动。不用它，是因为触屏宿主把反馈放在「我的」里有自己的设计，Windows 原生窗口的 `SettingsNavigation.h` 和路由别名照它对齐，删掉会同时改三套导航；桌面侧栏的投影就够了。
- **账号页按有没有 `settingsSync` 改名，Linux 也叫「账号」** — 更贴近事实（Linux 同样没有设置同步）。第一轮没有这样做，因为只决定 macOS；跟进时 Linux 加进了 `desktopPageTitle` 的条件。仍按平台而不是按有没有 `settingsSync` 判断，是因为页名在注册表投影时就要定下来，那时还拿不到客户端注入了什么。
- **「修复输入源」沿用原来「安装 / 更新」的强制重装，安装失败就不再登记** — 合并前后安装一步的含义不变。不用它有两个原因：已装的输入法可能比设置应用随附的新（Sparkle 更新了输入法、`scripts/install.sh` 装了更新的构建、新版之后又打开了旧 DMG 里的应用），启动检查在这些情况下都不降级，强制重装却会把用户悄悄退回旧版本；而开发运行、ad-hoc 签名构建、登记待定这些情况下安装一步本来就会失败，原来单独的「重新注册」能修好的问题，合并后反而修不了。
- **MCP 块放进维护页的「更多选项」折叠** — 不用新增页面。不用它，是因为展开后页面一样长，这块有标签、代码块和写入按钮，单独一页更容易读，返回和侧栏点亮也有现成的子页机制（同「使用统计」的 `usage-reporting`）。
- **手机溢出菜单里的「刷新统计」一起撤掉** — 理由相同，手机上也在自动轮询。没有撤，是因为 Android 设备冒烟按这个按钮驱动统计页，这一轮不碰触屏布局。

## Consequences

- **收益**：macOS 上这几页少了十几个常驻控件（MCP 连接块整块移走不计在内）；两个打开了不起作用或误导的地方修好（背景音乐、自定义提示词说明）；固定取值由一个 `HostPlatform` 方法决定，host-api、设置页、测试读同一个事实。
- **代价**：
  - 在 macOS、Windows、Linux、HarmonyOS 上把 AI 候选数量设成 1（省 token）或 10，在 macOS、Windows、HarmonyOS 上把效果强度调小的用户，升级后回到 3 和 50，且无处可改；最多约 4 倍的效果幅度变化。想要更弱只能换「闪光」样式或关掉效果。同一份偏好在 Android、iOS 上仍按存的值生效。
  - Windows 原生「关于」页把帮助内容直接画进来（没有子页），比共享页长。
  - 在 Windows 原生窗口里，统计页不再给「打开数据目录」，要从「维护与诊断」打开同一个目录；同一台机器上的共享页仍然给。
  - 自动同步会把用户打开开关前很久复制的内容记进历史，隐私标记的过滤与原来的手动同步相同。
- **重访信号**：如果 AI 座位改成能显示多行，候选数量要在对应平台上重新开放；Android、iOS 跟进时，按同样的能力字段决定是否固定这两个值（Android 的选中文字润色和 iOS 的 AI 回复也读这份配置，要先确认数量对它们没有意义）。Linux 将来画出样式时，效果强度要和样式一起决定。

## Verification

- Rust：`cargo test -p msime-client-core host_surface`（`macos_fixes_ai_candidate_limit_and_effect_intensity_at_their_defaults`），`cargo test -p msime-host-api`（`the_ai_request_sends_the_platforms_candidate_limit`、按平台改写的 `ai_queries_and_delivery_follow_pending_preferences` 和 `the_selected_effect_pack_replaces_the_preferences_style_and_follows_a_reimport`）。
- 共享页：`apps/desktop/tests/settings/plugins-section.test.tsx`、`script-plugins.test.tsx`、`settings.test.tsx`（AI 分组、修复输入源、剪贴板自动同步、帮助与反馈并进关于、账号页名）、`settings-page-projections.test.ts`、`mcp-connect.test.tsx`、`account/account-page.test.tsx`、`voice/polish-prompt-slots.test.tsx`、`typing-statistics.test.tsx`。
- macOS 原生：`TypingEffectPanelTest.mm`、`AISettingsWindowTest.mm`、`AISettingsSnapshotTest.mm` 已改为新行为，只能由 CI 的 macOS 构建编译运行。
- 跟进：`cargo test -p msime-client-core`（`fixed_ai_candidate_limit_and_effect_intensity_sit_at_their_defaults` 逐平台钉住固定值）、`cargo test -p msime-host-api`（按 `compiled_platform()` 取值的用例在各平台 CI 上各自成立）；共享页 `settings.test.tsx`（四个固定数量的平台没有「联想」组，Android 仍有）、`plugins-section.test.tsx`（Windows 和鸿蒙 2in1 没有效果强度，Linux 有升档音）、`settings-page-projections.test.ts`（Linux 的账号页名）；鸿蒙 `platforms/harmony/tests/run.sh`（`TypingEffectPolicy` 不读存下的强度）；`scripts/test-linux-ibus-ai-candidate-limit.py`（IBus 的 AI 设置比较不含固定的候选数量，去掉修复时失败）；Windows 的 `windows-ai-settings`、`windows-plugin-page-model`、`windows-settings-navigation` 在本机用 clang 编译运行过，原生窗口和 Server 的改动只能由 CI 的 Windows 构建编译。

相关：[AI 座位只坐一行](../feature/2026-10-11-ai-seat-placeholder.md)、[Windows 原生 AI 页](../feature/2026-10-11-windows-settings-ai-page.md)、[Windows 原生插件页](../feature/2026-10-11-windows-settings-plugins.md)、[Windows 原生统计页](../feature/2026-10-11-windows-settings-typing-statistics.md)、[Windows 原生剪贴板页](../feature/2026-10-11-windows-settings-clipboard.md)、[Windows 原生反馈页](../feature/2026-10-11-windows-settings-feedback.md)、[关于页系统信息](../feature/2026-10-10-shared-about-system-info.md)、[插件选择与播放条件](../bug-fix/2026-10-10-plugin-selection-playback-gates.md)。

### 审查后的验收约束

Windows 的「帮助与反馈」并入「关于」、手写并入屏幕键盘后，原生侧栏共 16 页。真实宿主验收同步访问这 16 页，并检查「关于」仍有反馈类型、描述、复制报告与交流入口；`scripts/test-windows-settings-ui-pages.py` 在本地与 CI 上核对 PowerShell 验收清单和 `SettingsNavigation.h` 的页面顺序，防止只改导航却漏改设备验收。保留独立的清单核对，是为了在不能启动 WinUI 的开发机器上也能立即发现这类漂移；真实 Windows 验收仍负责确认控件实际呈现。
