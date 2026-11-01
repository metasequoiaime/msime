# HarmonyOS 输入宿主

ArkTS 宿主与 NAPI 原生边界是完整实现：键盘扩展、设置应用、账号、社区、AI、语音、手写、候选与词库都在本目录内。能力对照见 [docs/harmony-parity.md](../../docs/harmony-parity.md)：用什么方法比过 MSIME-Apple、哪些是按平台特性裁剪而不是欠账。

## AI 生成类功能目前不提供

华为应用市场（中国大陆）要求 AI 生成类功能先做实名认证、提交《安全评估报告》和生成内容标识材料，这些资质到位之前，鸿蒙版不提供任何把用户文字发给大模型、再展示或上屏生成内容的功能：设置页的「AI 辅助」页、AI 对话（「试用键盘」也不带发送键）、AI 皮肤、语音页的「文本润色」、翻译服务里的「AI 辅助」、社区的回复模板、打字统计里「AI 润色」「高情商回复」两类分布（历史计数还在，只是不画），以及键盘功能面板的「高情商回复」「AI 润色」、打字时的 AI 联想、AI 候选翻译（含 `/fy`）和语音识别后的 AI 润色。

总闸只有一个：Rust 的 `HostCapabilities.ai_generation`（`crates/client-core/src/host_surface.rs`），鸿蒙为 false。共享设置页按它派生 `showAiGeneration`；`apps/harmony` 按它不给页面 `chat`、`aiSkins`、`aiAssistant` 客户端；键盘进程和设置进程启动时把它交给 `keyboard/AiGenerationGate.ts`，功能面板据此去掉两个 AI 格子，`HarmonyVoicePolisher.polish`、`KeyboardSession.thoughtfulReplies`、AI 联想分支、`TranslationPolicy.provider` 和设置桥的 `startRequest` 发请求前都问它，所以从别的设备同步来、打开了 AI 的偏好也不会触发请求。系统语音识别和各识别服务、云候选、腾讯云 / 小牛 / DeepL 机器翻译不是大模型生成，照常提供。资质到位后把那一行改回 true 即可恢复，下文描述的各 AI 功能就是那时的样子。决定与备选见[决策笔记](../../.agents/notes/implemented/feature/2026-10-27-harmony-hide-ai-generation.md)。

## 社区投稿目前不提供

华为应用市场（中国大陆）要求「信息发布」类服务先做实名认证，鸿蒙版的邮箱和华为账号登录都不核验手机号，所以实名认证到位之前，社区里只能浏览、搜索、看详情、获取和试用皮肤、安装插件、收藏、导入词包和存取回复模板，以及下架或删除自己的作品；不提供发布皮肤、词包、短语、回复模板和插件，修改已发布作品的分类，发布新版本，评分和举报。屏蔽作者不是投稿（不公开任何内容，只让本账号看不到那位作者的作品），照常提供，见下文「社区页仍是共享界面」一段。

总闸是 Rust 的 `HostCapabilities.community_contribution`（`crates/client-core/src/host_surface.rs`），鸿蒙为 false。共享设置页按它派生 `showCommunityContribution`，社区页据此不画这些入口；`apps/harmony` 的社区客户端对这些操作直接拒绝、不给举报；设置进程启动时把它交给 `keyboard/CommunityContributionGate.ts`，设置桥的 `startRequest` 在分发社区请求之前问它。

AI 生成类功能的总闸关着时，社区也不列 AI 生成的皮肤：设置桥的社区请求带 `exclude_ai_generated`，Rust 向服务端发 `exclude=ai`，服务端只留下来源为 `user` 的皮肤；服务端没有回 `X-MSIME-Skin-Origin: v1`、或者是还不认 `fields=origin` 的旧版本答了 400 时，皮肤图库按「社区暂不可用」处理。服务端契约和恢复方法见 `.agents/notes/implemented/feature/2026-10-28-harmony-withhold-community-contribution.md`。

## 输入法扩展的独立沙箱与共享沙箱镜像

**从 API 12（OpenHarmony 5.0.0.38）起，输入法扩展进程使用独立沙箱，与设置应用（UIAbility）互相看不到对方的 `files/`。** 华为「输入法安全模式介绍」原文：「输入法扩展使用独立沙箱，与应用主入口不可互相访问对方的独立沙箱。」ExtensionAbility 类型表里，「是否有独立 Extension 沙箱」一栏对 `inputMethod` 是「是」。两个进程拿到的路径字符串相同（`/data/storage/el2/base/haps/entry/files`），背后是两个不同的目录。

2026-10-09 在 HarmonyOS 6.0.2(22) 模拟器上实测：设置应用的 `files/state` 里有 `typing-statistics.json`、`account-session.json`、`key-feedback.json`、`CustomSkins`；键盘进程的 `files/state` 里一样都没有，另有它自己的 `preferences.json`、`CommonPhrases.json`、`PersonalDictionary`、`emoji_recents.json`。

所以本文中凡是写「设置应用和键盘两个进程共用 `files/state` 里同一个文件」的地方，在 API 12 及以上都不成立，两个进程各有一份。上面的实测是手机模拟器；2in1 没有单独实测，但华为的说明针对的是 `inputMethod` 扩展类型本身，不分设备形态，所以按同样不成立处理。没有共享沙箱时（签名不带 data-group-id、在模拟器上）可见的后果：

- 设置页的改动（输入方案、按键反馈、自定义皮肤）不会传到键盘，键盘只读它自己那份 `preferences.json`。
- 键盘看不到设置应用里的登录会话：键盘里的云剪贴板等账号功能按未登录处理。
- 打字统计：设置页打开统计只改了设置应用那份文件，键盘那份仍是默认的关闭状态，每次上屏计 0 个字。
- 插件：插件页（2in1 和手机、平板都有）把包和 `mentions.json` 装进设置应用自己的 `files/state/plugins`，键盘读的是它自己沙箱里的 `plugins/`，所以从设置页导入的符号集、音效、音乐、指令表、短语表、辅助码和特效包在键盘里都看不到；选用哪个包写在共享偏好里，同样传不到键盘。背单词在设置进程里运行，读的就是设置应用那份目录，单词本插件不受影响。
- 防止截屏：开关只写在设置应用那份 `files/state` 里，键盘读不到，三块面板不设隐私模式；设置应用自己的窗口照样受保护。

官方提供的共享方式是「共享沙箱」：签名 Profile 和键盘扩展 `module.json5` 的 `dataGroupIds` 登记同一个 data-group-id，两个进程用 `context.getGroupDir(id)` 拿到同一个目录；基础访问模式下键盘对它只读，完整体验模式下可读写。本宿主的 ID 是 AppGallery Connect 签发的 `group.1516753898738609107`（代码里在 `keyboard/SharedStateMirror.ets` 的 `DATA_GROUP_ID`）。签名 Profile 必须在 AGC 网页端「证书、APP ID和Profile › Profile › 新增」手动新建，DevEco 自动签名不会写入这项授权；新建后用 `strings <profile>.p7b | grep data-group-ids` 确认带着这个 ID。模拟器不做这项授权校验，只能在真机上验证。

两个进程并不直接共用共享目录，而是各自保留私有的 `files/state`，经共享目录镜像对方要看到的那部分：设置应用改完设置后把偏好、按键反馈、应用主题、统计开关、皮肤、插件、社区回复库、隐私政策授权、「防止截屏」开关（完整体验模式下还有账号会话）发布到 `<共享目录>/msime/settings/`，键盘在启动和每次获得焦点时导入；完整体验模式下键盘把它改的偏好、刷新过的会话和打字统计发布到 `<共享目录>/msime/keyboard/`，设置窗口回到前台时导入。正在用的皮肤包被同名替换（重装图库作品、皮肤编辑器覆盖保存）时偏好修订号不变，键盘在获得焦点、导入完之后比较选中包的目录、`skin.toml` 和画出的装饰图、背景图的 inode、修改时间和大小，变了就重新应用外观，不必重启键盘（`tests/candidate-skin-package-identity.test.ts`）。不直接共用的原因、条目表和冲突规则见 `.agents/notes/implemented/architecture/2026-10-09-harmony-ime-shared-sandbox.md`。基础访问模式下是单向的：键盘工具栏里改的设置会被设置应用下次的改动覆盖，统计计数和登录会话不交换。

### 「我的词条」只有一份

两个进程各有一份 Engine 日志（各自的 `files/state/user/msime_user.db`），这一点没有变：学习记录、候选顺序和固定位置只在键盘里，设置应用不打字。变的是「我的词条」——日志里 `user_inserted` 而且没删的词条，也就是设置页「我的词条」列出的、WebDAV 网盘同步的那一组，身份是（词库类别、编码、词）：两份经共享沙箱交换，收敛成同一组。以前键盘打字学到的词只在键盘那份里，设置页改的、导入的词只在设置应用那份里。规则在 Rust 的 `msime_client_user_words_mirror`（`crates/host-api/src/user_words_mirror.rs`，C 头文件里有完整说明），宿主一侧是 `keyboard/UserWordsMirror.ets` 和 `keyboard/UserWordsMirrorPolicy.ts`（`tests/user-words-mirror-policy.test.ts`，调用点由 `scripts/test-harmony-user-words-mirror.py` 守着）。决定与备选见 `.agents/notes/implemented/feature/2026-10-27-harmony-unified-user-words.md`。

- 每一边在自己的 `files/state/user-words/` 里给每个词条记一个「最后写入者胜」的寄存器（时间戳、在不在），发布到 `<共享目录>/msime/<settings|keyboard>/user-words/words.json`。这个文件不走上面的清单和指纹，由两边的 Rust 直接原子替换和读取。
- 设置应用：窗口回到前台和每次改完设置之后（与上面的镜像发布同一时机），在工作线程上先排空它自己的个人词库队列，再合并键盘的发布、发布自己的；本方日志、队列和键盘的发布都没变时不做。导入的个人词库文件、云词库下载和命名词库的词都走这个队列，API 12 起键盘排空的是它自己沙箱里的那个，所以这些词以前从来没进过任何一份词库；现在由设置应用写进它的词库，再交换到键盘。
- 键盘：写词库要 Engine 的独占维护权，只有建会话之前拿得到，所以合并放在那里，和个人词库队列的排空在同一处，一次最多写 128 项，剩下的在空闲时的会话重建里接着写；获得焦点时发现设置应用的发布变了，就排一次空闲时的会话重建（不打断正在组字的输入）。失去焦点时在工作线程上只记录、只发布，不写词库，会话照常开着；日志没变或者距上次不到 30 秒时不做。
- 基础访问模式下键盘对共享目录只读：设置应用的改动照样写进键盘，键盘学到的词、删掉的词留在键盘里，到不了设置应用，也就进不了网盘；切到完整体验模式以后的下一次发布一起带过去。模拟器上没有共享沙箱，两份照旧各管各的。

合并规则：

- 第一次运行（升级上来的老用户）两边都还没有寄存器，各自的词都记成新加的，合并结果是并集：迁移从不删词。代价是以前只在一边删过、另一边还在的词会回到两边。
- 之后同一个词条两边都改过时，时间戳晚的胜，相同时「在」胜过「不在」；时间戳是察觉到改动的时刻（键盘建会话之前或失去焦点时、设置应用在上面那几个时机），不是改动本身的时刻。一边删一个词、另一边加另一个词，两件事都保留。删除是一次写入（墓碑），对方确认合并过包含它的发布以后两边各自回收；键盘一直在基础访问模式下时确认不了，墓碑超过两万个时去掉最早的。
- 键盘一次少了 50 个以上、并且至少是原有一半的词时不当成删除：输入法里没有这种操作，多半是它的日志坏了被换成空的。设置应用那份照样留着，键盘这边也不自动写回。设置应用一侧不挡：停用命名词库、清除学习数据这类大批删除是用户在设置页的操作，照常传到键盘。
- Engine 不收的词（例如音节表里没有的拼音）照实记下、不再重试，不会被当成删除传回去；写不进去（维护权被占着）的留到下一次。整个交换不联网，词条只在两个沙箱和共享目录之间。

与「我的词条」的 WebDAV 网盘同步（`msime_client_dictionary_sync`）：同步只在设置应用里运行、只读写设置应用那份，键盘仍不调它，一台手机只有一个设备身份。键盘的词经上面的交换写进设置应用的词库以后，同步下一次运行开头的核对就把它们记成本机改动发出去；设置应用写进键盘的改动、或者排空队列写进了词时，还会向设置页派发 `msime-user-words-changed`，页面的词库同步（`packages/ui/src/settings/use-dictionary-sync.ts` 的 `HOST_USER_WORDS_CHANGED_EVENT`）据此马上以 `local_change` 记一次，和经设置改了词走同一条通知、连续几次只排一次；没有隐私政策的授权时页面不提供词库同步，也就不听这个事件、不联网。网盘来的词写进设置应用的词库后，下一次交换就带给键盘。

OpenHarmony 适配保留 ArkTS/ArkUI 应用入口与 NAPI 原生边界。共享输入算法、组合状态、配置校验和资源准备继续由 Rust Host API 与 Rust Engine（`crates/engine`）提供；`platforms/harmony/native/client_napi.cpp` 只负责 NAPI 注册和 C ABI 转发，不复制候选分页或输入状态机。

繁体输出只在显示与上屏边界转换：候选条与展开候选面板的显示文字、Engine 提交、`insert` 与 `insertWithSource` 经过 `KeyboardSession.asTraditional`，Engine 的候选原文、候选身份（按序号选择）和组合文本保持简体；由 `ChineseOutputPolicy` 决定是否适用（dedicated English、日语方案、临时日语保留原文），转换本身走 NAPI `simplifiedToTraditional` 调共享导出 `msime_client_simplified_to_traditional`，即 OpenCC s2t 词级转换，与 Windows、macOS、Linux、Android、iOS 逐字一致——「头发」出「頭髮」、「发展」出「發展」，ICU `i18n.Transliterator` 的逐字转换分不开这两个「发」。C ABI 拒绝的输入（内嵌 NUL）返回 `null`，保留原文。`scripts/test-harmony-traditional-output.py` 钉住从 C 头文件到调用点的这条接线，并拒绝宿主源码里重新出现 ICU 转写。

Harmony 设置页暴露共享的模糊拼音规则、触摸输入方案启用列表、自定义触摸键盘皮肤设计和候选英文释义开关。这四项此前都只有键盘一侧在消费：`PreferencesStore` 里有值，键盘准备 Engine 会话时会读，但设置页从未开启对应的客户端开关，用户没有任何途径改动它们。它们各自只写共享偏好，不需要平台能力。

手写方案直接使用 `crates/engine` 已有的离线中文单字笔画识别器：书写区外层接收触摸，ArkUI Canvas 绘制有界笔画，停笔后经 `msime_client_handwriting` 与 NAPI 工作线程返回候选，选字仍由共享 Engine 会话提交到编辑器。`stage-resources.sh` 自动获取 `resources/handwriting-model.lock.json` 固定的 Tegaki zh_CN 模型并校验大小与 SHA-256，随包附带模型和 Zinnia 许可证，约增加 26.8 MB 模型；不调用系统 OCR、不联网、不保存笔画。

语音输入的麦克风由键盘自己采集（`HarmonyPcmCapture`），再交给选中的识别方式：系统识别把音频写进 Core Speech Kit `speechRecognizer` 的写音频模式（见下文），豆包、HTTP 服务商和本地模型各自接收同一份 PCM。工具面板可以开始、停止或取消识别，最终文字经过长度和控制字符边界检查后通过当前 `KeyboardSession` 提交；原始音频不写入文件、不进入日志，也不复制到 Engine，只有选了云端服务时才发给那个服务。选了豆包却一项凭据都没填（这就是出厂配置）时用系统识别，语音面板和设置页都写明这一点；填了一部分时，语音面板报出缺的那一项，不再悄悄换成系统识别。系统识别需要 `SystemCapability.AI.SpeechRecognizer`，所有识别方式都需要用户授予 `ohos.permission.MICROPHONE`。

云联想与 AI 联想复用 Engine 的 `online_query` 代际契约（AI 联想目前随 AI 生成类功能一起关闭，见上文）：Harmony NAPI 只传递有界查询和结果，ArkTS 通过系统 HTTPS 栈异步访问云候选或用户配置的 Chat Completions 服务，结果再交回 Engine 做会话、偏好和 generation 校验。请求防抖、超时、响应大小、重复候选和控制字符检查均在宿主边界完成，失败只丢弃可选展示结果，不阻塞本地输入，也不把查询或响应写入日志。用户配置的 AI 服务地址经 NAPI 的 `aiEndpointCheck`（`msime_client_ai_endpoint_check`）交给 `crates/client-providers/src/ai/endpoint.rs` 检查，`keyboard/settings/AiEndpoint.ets` 调用、`AiEndpointPolicy.ts` 读回答，宿主不再自己判断：https 不限主机，http 只能指向本机或局域网（回环、私有地址、`100.64.0.0/10`、链路本地、IPv6 唯一本地地址、`.local`），这样 LM Studio 这类本地模型服务可以直接用 `http://192.168.x.x:1234/v1`，公网主机仍然只走 https。本工程没有 `network_config.json`，按 HarmonyOS 网络安全配置的默认值（`cleartextTrafficPermitted` 未配置时允许明文），系统 http 模块不会拦下这类请求；这一点尚未在设备上验证，若设备报 2300997（Cleartext traffic not permitted），再在 `entry/src/main/resources/base/profile/network_config.json` 里按需放开。

共享设置页的「AI 对话」由 Harmony 承载（目前随 AI 生成类功能一起关闭，见上文），对应 MSIME-Apple 的 `KeyboardChatView` 与 `BackendChatClient`。它与「AI」页那个用户自配的服务不是一回事：后者带着用户自己的 endpoint 和 token，而这一个用宿主已经持有的账号会话认证（没有登录时用本机匿名账号，额度按匿名账号算），WebView 全程看不到任何凭据。模型列表是 Rust `msime_client_account` 的 `chat_models`，回复是 `chat`，请求与响应的边界都在 Rust 里校验（16 条历史、单条 16 KiB、请求体 64 KiB），与其他平台相同。`chat` 最长阻塞 125 秒，NAPI 的 `accountThread` 给它单独起一个线程，不占工作线程池。

完成请求单独走一条 `chat` 请求类型，为的是它自己的期限。一次补全是模型在写字，共享客户端给它 125 秒；与账号请求共用一条通道就只有两种结果——要么把还差一点就返回的补全提前判成超时（随后到达的回复会因为没人在等那个编号而被丢弃），要么让一次卡住的资料拉取也空转两分钟。

账号页的「设置同步」也由 Harmony 承载，对应 MSIME-Apple 的 `SettingsSyncView` 与 `IOSPreferencePlan`。账号存的不是本机那份偏好文档，而是一张键名由服务端 schema 声明的标量表，每个宿主把自己的文档映射到它认识的那个子集上——这层间接正是要点：鸿蒙手机和 iPhone 在窗口装饰、工具栏、硬件和弦上几乎没有一处相同，但它们对"用户在用哪套输入方案"、"学习开不开"是一致的。

由此有两条规则。上传时丢弃 schema 未声明的键：带上一个未知键会让整次写入被拒，本机知道而服务端还不知道的设置因此只是暂时不旅行。应用时只读 schema 声明过的键，而声明过的键若带着错误的类型则整体拒绝——对未声明键保持沉默是服务端还没跟上，对已声明键的类型不一致则是双方对这个字段的含义有分歧，猜哪边对正是布尔值被写进整数设置的由来。

平台那一半用 `platform.harmony.*` 而不是复用 `platform.android.*`：这是两台各有键盘的设备，共用命名空间会让鸿蒙手机覆盖掉用户安卓键盘的皮肤和键距，那不是设置同步该做的事。在服务端声明它们之前，上面那条规则生效，只有共享的 `input.*` 那一半旅行——而那一半恰好是各宿主上真正同一个产品的部分。

上传前先读云端文档再合并，而不是替换：它带着用户登录过的每一台设备的字段，从手机上传没有理由清掉桌面写进去的东西。应用方向由页面报出账号 id，宿主在往返前后各核对一次当前会话——确认框还开着时的一次登出换号，会把陌生人的设置写到用户自己的设置上，而那个屏幕上没有任何东西能撤销它。写完本机文档后宿主主动把新文档推给页面，不等窗口重新切到前台：那个监听是为键盘的改动准备的，本窗口自己造成的改动不该需要切一次应用才看得见。

共享皮肤页的「我的设计」也由 Harmony 承载，对应 MSIME-Apple 的 `CustomSkinEditorView` 与 `CustomKeyboardSkin`。它不是设置文档里 `custom_theme.keyboard` 那一份（那只有一套，是全局主题选 `custom` 时正在用的那一套），而是最多十二套具名设计的独立文件——命名、改名、覆盖、删除。

来源断言审计补上了原生键盘最后一段渲染链。此前 `CustomKeyboardSkin` 虽然解析渐变、照片、图案、键帽形状/材质、阴影和透明度，真正的 `KeyboardView` 却只读取基础颜色与等宽字体，导致共享设置预览会变、系统键盘不变。现在共享 Base64 照片先经过 512 KB 与 JPEG/PNG/GIF/WebP 魔数校验，再作为 ArkUI 图片源进入根背景；渐变、照片位置/压暗和三种固定图案在键盘背板绘制，键帽消费形状半径、材质高光、阴影与填充 alpha。填充透明度不再施加到整个容器，因此不会把键文字一起淡化。`scripts/test-harmony-custom-skin-rendering.py` 固定这些产品调用点。

这条没有像账号那样在 ArkTS 里重写一份，而是走新的 C ABI `msime_client_custom_skin_library`：Tauri 宿主把 `CustomSkinLibraryStore` 当 Rust 直接调，只通过 C ABI 到达这个 crate 的宿主（本宿主就是）否则就得把同一个文件的锁、原子替换、名称规范化和十二条上限再实现一遍，而一个库两个 store 正是两边开始对"里面有什么"意见不一致的起点。请求不带 `action` 是读，带了就先改再读，两种都回整个库——每个调用方改完都要重画列表，只回自己那一条会让页面猜改名对排序做了什么。

失败码用共享社区页面已经有措辞的那几个 `community_*`，不是 `Display` 文本：后者是写给日志的英文句子，不该出现在中文对话框里。

社区皮肤页也由 Harmony 承载，对应 MSIME-Apple 的 `SkinCommunityView`、`SkinCommunityAPI` 与 `CommunitySkinTrialView`。浏览不要求登录——画廊是公开的，一个看不到画廊的未登录用户没有任何依据判断值不值得注册；有会话时才带上，那正是 `owned` 和 `my_rating` 变成"这个用户的答案"而不是"没人的答案"的原因。下载、评分、发布、下架都要求登录。

id 会进 URL 路径，所以在进去之前先按形状校验：把页面给的任何字符串直接插进去，等于让页面把一个皮肤 id 变成另一个端点。失败码用 `community_*` 而不是 `account_*`——社区页面对"已达到发布上限"、"自己的作品不能评分"各有措辞，换成账号码就只剩那一句通用的。

「获取」是 Rust `msime_client_community` 的 `keyboard_skin_install`：下载设计、开始试用并导入皮肤库是一次调用，导入失败时 Rust 把刚开始的试用结束掉——试用记录正是"被换掉的是哪套皮肤"的记忆，导入失败必须把它结束掉，否则用户身上是一套从未保存、也无从还原的设计。没有登录时用本机匿名账号下载，所以未登录也能获取皮肤。装好以后宿主把新的偏好文档推给页面。

试用的结束不碰网络：保留还是还原，是对一套已经生效的皮肤的回答，由设备作答。`restore_pending` 是崩溃恢复，在没有待处理试用时调用也是安全的——它运行的时刻正是还没人知道上次会话是不是停在试用中间的时候。

装完与结束试用之后宿主都会把新的偏好文档推给页面，理由与设置同步那条相同。

社区页的「词库 / 回复」分类也由 Harmony 承载，对应 MSIME-Apple 的 `CommunityResourcesView` 与 `BackendCommunityResourceClient`。公开列表和单个资源的详情不要求登录，理由与皮肤画廊相同；「我的作品」和「收藏」要求登录——它们是关于某个账号的问题，没有账号时答案要么是空的，要么是别人的。

资源内容按它是什么来校验：回复只有 prompt，词库是 1 到 128 条词条且没有 prompt。共享服务对其它组合是整体拒绝而不是忽略多出来的那一半，因为一个带着 prompt 的"词库"，它的作者以为自己发布的是别的东西。

合并共享词库是两次请求：服务端需要知道这是合并到用户自己词库的哪个版本上，所以先读再写，读到的版本号进写请求。共享服务也是这么做的，这也是它不能做成页面自己发一次请求的原因——页面若在两次之间持有那个版本号，就是在用户随时可以离开的一个界面上持有它。

回复模板的保留与移除完全不碰网络，写的是 `files/CommunityLibrary.json`，也就是 `KeyboardSession.communityReplyTemplates` 读的那个文件。它在 state 目录旁边而不是里面：键盘扩展拿到的是 `files/state`，写进那个目录的库会被写在读取方从不查看的地方。写入走新的 C ABI `msime_client_community_resource_library`，因为写它需要那个 store 的校验（reply 类型、非空 prompt、没有词条、五十条上限），而键盘解析这个格式是严格的——宿主自己写等于给一个严格解析的格式添了第二个作者。

请求信封上限从 64 KB 提到 512 KB。每个操作仍各自校验自己的载荷，这一条只是解析前拦掉荒谬输入的第一道；但发布一份社区词库最多带 128 条、每条最长 1024 字符，而服务端允许 350,000 字节内容，64 KB 的信封会在本宿主上拒掉服务端本会接受的资源。

AI 生成皮肤也由 Harmony 承载（目前随 AI 生成类功能一起关闭，见上文），对应 MSIME-Apple 的 `AISkinGenerationView` 与 `AISkinService`。整条流水线（提示词、模型调用、解析三套设计、三张插画的任务和轮询、失败时停掉其余任务、任务无论怎样结束都释放）在 Rust 的 `msime_client_ai_skin_generate` 里，与其他平台相同；NAPI 的 `aiSkinGenerate` 给它单独起一个线程，取消是 `aiSkinCancel`。没有登录时用本机匿名账号，额度按匿名账号算。

进度单独走一条通道，不混在回复里，方向与偏好变更通知相同，并带上页面自己选的 request id：三张图要几分钟，一次从头到尾不吭声的运行和一次已经停了的运行在屏幕上没有区别；而过期运行的计数器不该去驱动新运行的显示。Rust 每完成一张插画在生成线程上回调一次，NAPI 经线程安全函数把张数送回 ArkTS 线程。

Apple 的首页（`KeyboardHomeView`）也由 Harmony 承载，但是按本平台裁剪而不是照抄：五个动作里这里有两个。「设置」打开系统输入法列表，选择器是安装第二步的去处，两个都是本宿主的能力。

`openKeyboard` 故意不提供。Android 为它开一个独立的面板窗口；本宿主的键盘是 InputMethodExtensionAbility，编辑器要它的时候才出现，设置应用没有窗口可开。不提供这个动作时，「设置」首页状态卡片上的「试用键盘」打开共享的子页面「试用键盘」（`packages/ui/src/keyboard/try-keyboard-page.tsx`），那才是这里"让我看看键盘"的诚实版本：按 Android 试用页的聊天样式，一进页面就把焦点交给输入框，系统弹出当前输入法；AI 生成类功能打开时，登录后打的字可以作为 AI 对话发出去，走的是设置页已有的 `chat` 通道；目前总闸关着，页面不带发送键，只用来打字。这一页不带底部标签栏，键盘弹起时输入栏直接落在键盘上方。表情和剪贴板两个动作不提供的理由相同：在本宿主上它们是键盘自己键面上的界面，不是窗口。

云剪贴板在键盘里也有一份：剪贴板面板（手机的剪贴板键面、2in1 表情面板的剪贴板页）分「本机」与「云端」两栏。云端只在面板打开和点「刷新」时读取一次，没有轮询，复制时不上传，也不读系统剪贴板；点一条就插入当前编辑器。本机历史长按一条出现「发到云剪贴板」，只有已登录（或选了自建中继、WebDAV 网盘）且云剪贴板已开启时可用。密码框里没有「云端」这一栏，读取期间换了编辑器的结果直接丢弃，判断都在 `keyboard/clipboard/CloudClipboardPolicy.ts`。每个请求先问 Rust 该走哪条路（`msime_client_cloud_clipboard_source`）：设置里选了自建中继或 WebDAV 网盘就只经 `msime_client_relay_cloud_clipboard`，否则经 `msime_client_account_cloud_clipboard`，目录都是键盘沙箱自己的 `files/state`；设置应用里的登录经共享沙箱的会话镜像采用过来（见下文「账号与云端业务在 Rust 里」）。键盘不自己生成云剪贴板的设备身份：请求带 `create_identity: false`，用设置应用经共享沙箱镜像过来的密钥串，设置应用还没用过云剪贴板时提示先在应用里打开一次，一台手机只登记一台设备（`scripts/test-harmony-cloud-clipboard-identity.py` 守着调用点）。

云剪贴板是端到端加密的，协议和宿主的职责见 [docs/cloud-clipboard-e2e.md](../../docs/cloud-clipboard-e2e.md)。列表、上传和设备管理（`devices`、`approve`、`reject`、`remove`、`reset`）、删除和开关都由 Rust 驱动和发送；驱动的错误码（`clipboard_device_pending`、`clipboard_sync_disabled`、`clipboard_key_conflict` 等）原样作为 `{ok:false,error}` 交给设置页和键盘。本机的密钥串仍是与会话同目录的 `files/state/cloud-clipboard-keystore`，Rust 读写的就是原来那一份，所以升级以后不会重新登记、不会出现「等待批准」；完整体验模式下设置应用把它镜像给键盘，键盘只用这一份；退出登录、注销和会话失效时随会话一起删除，换一个账号登录时不再删除（迁移差异 M3）。登记到服务端的设备名由宿主每次带上：`@ohos.deviceInfo` 的 `marketName`，没有时用 `productModel`，再没有时是「HarmonyOS 设备」。置顶、保留期和清空 Rust 也支持，共享页面在本宿主上还没有入口，宿主不放行。

面板记下读到列表时登录的账号 id。列表返回、点按插入和发送前都会重新核对；退出登录或换了账号以后，旧列表和待发送项作废，用户点刷新后再读取。同一个账号的令牌轮换或重新登录不再作废它，与 Rust 只按 user.id 绑定一致。

### 账号与云端业务在 Rust 里（#8201）

账号、社区、云词库、云剪贴板、AI 皮肤、对话、反馈、设置同步和打字统计上传都经 Rust 的目录模式（`msime_client_account`、`msime_client_community`、`msime_client_cloud_dictionary`、`msime_client_account_cloud_clipboard`、`msime_client_ai_skin_generate`），目录是各自沙箱的 `files/state`：会话文件、刷新令牌、`account-refresh.lock` 和全部 HTTP 都在 Rust 里，与 Windows、Linux 的同一份实现。ArkTS 以前那份桥接（`AccountCloudBridge` 和它的 `account-session.lock`、`anonymous-session.lock`）一起删掉了，同一个沙箱里不再有第二个刷新方。账号请求带着宿主自报的 User-Agent（`msime_client_client_agent` 拼出的 `msime-harmony/<版本> (<机型>; HarmonyOS <版本>; edition=<id>)`），服务端「我的设备」据此显示鸿蒙和机型。以前 ArkTS 写的扁平会话（`{access_token,refresh_token,token_type,expires_at,user}`）Rust 照样读，第一次保存时写成 Rust 的布局；降级装回旧版时旧版读不了新布局，会被登出。

宿主只做 Rust 做不了的那部分（`account/HarmonyAccountCloudBridge.ets`，规则在 `account/HarmonyAccountRequests.ts`，由 `tests/account-requests.test.ts` 覆盖）：页面能发哪些操作按名单放行，带文件路径的字段只由宿主经 DocumentViewPicker 选好、复制进私有目录后填写，页面发不了 `upload_avatar`、`data_export`、`export_file` 这类带路径的操作；反馈由宿主按字段重建，不带截图，平台总是 `harmony`；登录、退出、注销前后取消旧账号排队中的词库快照；整份快照和词库导出先由 Rust 写进私有目录（`cloud-dictionary-exports`、`dictionary-exports`），再由宿主复制到用户选的位置，恢复先把选中的备份复制进 `cloud-restore` 再交给 Rust 的 `snapshot_restore_preview_file`、`snapshot_restore_file`，Rust 自己的中转目录是 `cloud-dictionary-transfer`；设置同步里宿主核对账号、取字段表和云端文档、读写按键反馈文件，映射在 Rust。长任务（对话、整份快照、AI 皮肤）在 NAPI 里各起一个线程，不占工作线程池。

两个沙箱之间的会话镜像也改由 Rust 决定（`account/AccountSessionAdoption.ets`，规则在 `AccountSessionAdoptionPolicy.ts`）。`account-session`、`anonymous-session` 两条照旧发布到共享目录，但导入时不再整份覆盖本地文件，而是调 `session_adopt`：键盘采用设置应用发布的（`from:"settings"`），设置应用采用键盘发布的（`from:"keyboard"`），在本沙箱的 `account-refresh.lock` 里按 user.id 和 expires_at 决定——同一个账号新者胜，换号只认设置应用发布的，设置应用里没有登录会话时键盘跟着清掉，匿名会话从不清掉；本地还没有匿名会话时两个方向都采用，先启用键盘、后打开设置应用时设置应用用键盘注册的那个匿名账号，不再注册第二个（规则见 C 头文件的 `session_adopt`）。匿名账号的身份 `anonymous-account` 两边都发布，也不按文件复制，而是随匿名会话一起交给 `session_adopt`（`identity_path`）：Rust 采用了另一边的匿名会话时，在同一把锁里把身份换成发布方的，发布方没有身份而换了账号时删掉本地的，同一个账号时不动，没有采用会话时身份也不动，会话和身份始终属于同一个匿名账号（迁移差异 M40）。顺序是要点：每个沙箱在进程启动时登记并排一次采用，本进程的每个认证请求（包括匿名注册）都先 `await AccountSessionAdoption.beforeRequest()`，它再排一次采用并等它做完，免得拿另一边已经轮换掉的刷新令牌去刷新、把整个会话族撤销。请求的顺序不靠生命周期事件：设置页网页回到可见时发的请求可能早于窗口的 ACTIVE 事件。回到前台时照旧再排一次，让页面读到的账号状态尽早跟上。同一份发布物采用成功一次以后不再重复调用。两个沙箱各用各的锁，两边各自拿同一枚刷新令牌刷新的风险是原来就有的，镜像消除不了。

采用发布文件时 Rust 要求文件属主是本进程的有效 uid，不看所在目录的属主（本目录的会话文件仍要求与目录同一个属主）：data-group 目录的属主在真机上没有核对过，设置应用和键盘是同一个应用的两个进程、有效 uid 相同，所以发布方写的文件属主就是读的这一方（迁移差异 M1）。真机上如果两个进程的 uid 不同，每次采用都是 `account_storage`，键盘永远拿不到设置应用里的登录。宿主为此把这种失败写成一条点名规则的错误日志（`session adopt from settings refused for account: account_storage. The published session file failed Rust's ownership or permission check ...`），真机验收时要看一次 hilog，并 `ls -ln` 共享目录和其中的 `account-session.json`。

已知的回退：已有的账号操作断网时从「网络连接失败」变成「暂不可用」（Rust 对已有操作仍报 `account_unavailable`）；云词库面板参数不合法时的码从 `account_invalid` 变成 `invalid_cloud_dictionary`，面板对每个动作有自己的失败句子，不按码写文案，所以措辞不变。

使用情况上报、公告与社区审核走 client-services 的共享实现，本宿主只决定何时调用。上报开关是共享偏好 `usage_reporting`（默认开启），队列、随机安装 id、每日一次的 `active` 和会话记录位于**各进程私有**的 `files/state/telemetry`；API 12 起 App 与键盘的 `files/state` 不同，不能把两份队列视为共用，见上文「输入法扩展的独立沙箱」。一次会话就是一个键盘进程，正常销毁时仅结束本进程实际开始的会话；设置应用只准备独立捕获并发送，不增加输入会话。API 21 起，NAPI 接入共用 POSIX 原生记录器，主线程的 ArkTS 致命异常经 `setDefaultErrorHandler` 同步落盘后交回原处理器，无前任时同步退出；不读取异常消息、不在故障回调联网。两类入口在暂停、关闭和停止时同步停写，链式处理器在进程内只安装一次，避免覆盖后来安装的处理器。HiAppEvent 另监听 `APP_CRASH`（JavaScript 与原生）及 `APP_FREEZE`。正常路径用 `setEventParam` 保存 Rust 预备的 UUID、原构建头、PID 与准备时刻，系统补报只有原身份与当前授权匹配时才入队，旧无身份报告不追认。崩溃按原 UUID 归属会话，卡死独立；原生帧保留合法 ELF build-id。延迟报告带系统原版本与发生时间，经 Rust 共享权威授权代次核对后入队，读取授权失败保持拒绝，不回退为独立文件。统计关闭与隐私政策撤回共同使权威授权失效，通知只触发重读和条件清理。原生帧只留文件名、pc 和符号；信号只留名称与 code。

App 在共享目录 `msime/reporting-authority` 发布合并统计开关与隐私政策的持续授权；键盘只读既有锁文件和记录，不创建文件。两个活跃进程每秒独立核对，网络请求不阻塞撤回；源或政策关闭重开换代，临时源读取错误暂停且不清队列。App 无共享目录时使用私有授权，键盘保持拒绝。App 导入键盘偏好保留自己的统计开关，避免陈旧镜像重新开启。系统原身份传递的真实投递与精确构建收件仍待设备验收；官方 HiAppEvent 规格从 API 22 支持输入法应用，当前最低 API 21 使用上述本进程兼容入口，其真实故障与原处理器链仍待设备验收；协议与设备缺口见[崩溃补报笔记](../../.agents/notes/implemented/feature/2026-11-01-harmony-crash-reporting.md)。`tests/run.sh` 覆盖策略和替代系统 API 后的实际适配器回归，有 NDK 时还由 `tests/check-native-capture.sh` 严格编译三架构记录器与 NAPI；HAP 编译不代表设备上新增 NAPI 及系统回调已验收。

公告在设置窗口打开或回到前台时取（client-services 一分钟内直接用缓存），显示为设置页上方一张可关闭的卡片，关闭按公告 id 记在本地。正文是 client-services 用 pulldown-cmark 渲染、原始 HTML 已转义的 HTML，放进一个禁用脚本、CSP 为 `default-src 'none'` 的小 Web 组件里；点链接一律交给系统浏览器或邮件应用。这里没有用 RichText：它没有拦截链接点击的入口，链接会在卡片里打开，而 Web 组件的 `onLoadIntercept` 可以把它拦下来交出去。

社区页仍是共享界面，请求都是 Rust `msime_client_community` 的：「我的作品」带作品自身的 `moderation` 字段，由页面只对 removed 显示「已下架」；`report`（皮肤 `skins`、词库 `dictionaries`、回复 `replies`）按固定的六个理由举报，没有登录时用本机匿名账号；422 `blocked_content`、503 `screening_unavailable`、403 `account_banned` 分别报成 `community_blocked_content`、`community_screening_unavailable`、`community_account_banned`，与桌面端同名。

屏蔽作者也是 `msime_client_community` 的操作：`block`（`{kind,id}`，kind 与举报相同，id 是作品的）、`block_list` 和 `unblock`（`{id}`，屏蔽记录的 id）。页面经设置桥自己的 `community_block` 请求发它们，名单在 `HarmonyAccountRequests.ts` 的 `communityBlockRequest`，与三条图库通道的名单不相交；它要隐私政策的授权（`PrivacyConsentPolicy`），社区投稿类功能的总闸不拦它（`CommunityContributionGate` 只管三条图库通道）。三项只认登录的水杉账号，Rust 不退回本机匿名账号，没有登录时答 `community_unauthorized`：社区目录按列表请求带的令牌过滤，用匿名账号屏蔽的作者重装后就没了，也不会跟到别的设备上。列表请求有登录会话时本来就带登录账号的令牌，屏蔽之后刷新列表，服务端就不再列出那位作者的作品，客户端不另做过滤。

`registerJavaScriptProxy` 的名单现在由 `scripts/test-harmony-bridge-parity.py` 守着。ArkTS 对注入对象暴露什么有两处决定——类上的方法，和交给 `registerJavaScriptProxy` 的名字——而页面看得见的只有后者。一个名字只加了一处仍然能通过类型检查、能编译、能打包，然后在真机上以 `msimeHarmony.<name> is not a function` 的形式失败，表现是某一块功能就是不工作，而那恰好在这里谁也跑不了的那个平台上。具名皮肤库那一片就是这么漏的：方法写了，名字没注册，三道绿灯什么都没说。

个人词库文件导入也由 Harmony 承载，对应 MSIME-Apple 的 `PersonalDictionaryImportView` 与 `PersonalDictionaryImport`。共享页面上的那张卡片只在宿主提供 `dictionary.importPersonal` 时出现，此前本宿主不提供。

这一条走队列而不是 Engine，是这里唯一这么做的词库操作。其余操作（列表、单条编辑、导入词库文件、导出）直接取 Engine 的维护锁，那对"在设置窗口里改一个词、并盯着旁边的列表看结果"是对的。导入个人词库文件不是那种操作：用户什么时候导入由他自己决定，键盘开着的可能性和关着的一样大，而"dictionary maintenance busy"不是"请把这些词加进去"的回答。

队列是 Apple 与 Android 宿主已经给出的答案：词条写进 `<preferences_directory>/PersonalDictionary`。API 12 起那是设置应用自己沙箱里的目录，键盘看不到，所以由设置应用自己在工作线程上排空、写进它的词库，再经「我的词条」交换到键盘（见上文「「我的词条」只有一份」）；键盘排空的是它自己沙箱里的队列，规则相同：在建立 Engine 会话之前先应用一批——那是确定没有会话开着的边界，和 Android 在 `scheduleEngineStartup` 里选的边界相同。每批最多四条；仍有待处理项时，宿主每两秒等一个非组字、非本地模式的空闲点，释放会话、应用下一批、重建会话并恢复中英文、九键与焦点状态。这也是卡片上写"已加入同步队列"而不是"已导入"的原因：在本宿主上那句话同样是实话。

排空是尽力而为的。store 会保留没能应用的词条，失败只是推迟而不是丢失；为一个词库问题拒绝启动键盘，会把它变成"没有键盘"。请求 UUID 是 Engine 的持久回执，同一批在共享状态写回前中断也不会重复应用；损坏状态文件原样保留，读取不存在的队列不会创建目录。

个人词库 JSON 在预览和入队边界按 Engine 规则规范化：`NI HAO` 变成 `ni'hao` 后再参与重复检查，快捷短语允许换行和制表符，其它词条仍拒绝控制字符。Apple 示例里的多行短语以及大于旧 199 UTF-16 单元限制、但不超过 Engine 4096 字节上限的快捷短语都可导入；普通 Tab 文本词库格式原有的 199 单元限制没有因此放宽。

`personal_dictionary_request_json` 此前只有 Tauri 宿主当 Rust 直接调，本次以 `msime_client_personal_dictionary_request` 发布给通过 C ABI 到达这个 crate 的宿主。NAPI 侧的 `personalDictionarySync`（排空那一半）本来就已经导出，只是没有人调用。

键盘上的「AI 润色」也由 Harmony 承载（目前随 AI 生成类功能一起关闭，见上文），对应 MSIME-Apple 的 `KeyboardAIView` 与 Android 的 `aiPolishPanel`。

Apple 润色的是**选区**：用户选中一段话，点润色，面板给出那一段的重写。HarmonyOS 不给输入法读取选区的能力——`InputClient` 只有 `getForwardSync`（光标前）和 `getBackwardSync`（光标后），选区只以一对下标的形式通知键盘——照抄那个手势等于画一个不知道自己在操作什么的按钮。所以这里润色的是光标前的文字，也就是用户刚打完的那段话。同一个意图，用这个平台确实提供的东西表达；在手机上它也是更自然的手势：先打，再理。

请求复用 `HarmonyVoicePolisher`，那本来就是本宿主的润色器。这不是顺手：它允许重写后的段落里的换行，而旁边那个 AI 候选解析器拒绝一切控制字符——后者是给候选词用的，把一段重写过的消息送进去会让每一条分段的结果被静默丢掉。会话持有自己的润色器而不是复用识别器那一个：识别器在每次录音开始、结束或放弃时都会取消它，那对转写是对的，对用户正等着的一次重写不是。

润色与 HTTP 识别服务的默认地址和模型由共享 `client-providers::voice::provider_catalog` 生成到 `VoiceProviderCatalog.ts`；HTTP 请求格式来自同 crate 的 `voice::provider::asr_request_format`，只投影 multipart 与 chat_audio，不把豆包流式、本地或系统识别纳入 HTTP 上传。润色器、`HttpAsrConfigurationPolicy` 与初始语音配置同步读取，配置中的自定义值继续优先。修改共享目录后运行 `cargo run --quiet --locked -p msime-client-providers --example export_voice_provider_catalog -- --write`，再用 `--check` 检查 UI JSON 与鸿蒙生成物；`tests/voice-provider-catalog.test.ts` 固定润色默认值、未知 id 回退和初始配置；`tests/http-asr-provider-catalog.test.ts` 固定 HTTP 默认值、格式、非 HTTP 回退、调用方修改隔离与自定义字段优先。

内置润色的四篇简版正文由 `client-providers::voice::polish_catalog` 的 `compact_prompt` 持有，通过 `export_polish_prompt_catalog --write` 生成 `VoicePolishPromptCatalog.ts`，`HarmonyVoicePolisher` 同步查表；完整版设置目录 JSON 不包含简版字段。三个自定义槽位仍由宿主 trim，空正文和未知 id 回退到精炼整理。`tests/voice-polish-prompts.test.ts` 固定迁移前的四篇正文，导出器 `--check` 检查生成物没有过期。

替换前会重新读一次光标前的文字并要求原文还在那里。一次润色要几秒，其间用户可以继续打字、移动光标或换个输入框；那时替换会删掉现在那里的东西，再把另一段话的重写放进去。不匹配就拒绝并说明，而不是硬替。

删除长度按码点计算而不是 `String.length`。删除走 `deleteForwardSync(length)`：鸿蒙输入法框架的 forward 指光标之前、backward 指光标之后，与 Android 的命名相反，设备实测如此（以前用的是 `deleteBackwardSync`，光标在末尾时退格什么也删不掉）。它的文档只写了"length of text"，两种读法对 BMP 之外的字符不一样——句中一个 emoji 是一个码点、两个 UTF-16 单元；设备上 `deleteForwardSync(1)` 把光标前的 😀 整个删掉，单位不是 UTF-16 单元，这里按码点计算与之一致（多码点组成的字形还没在设备上试过）。文档措辞留有歧义，所以上面那次重读同时给它兜底：单位错了的代价是一次拒绝，不是一条被改坏的消息。

工具面板里的入口只在配置了润色服务时可用，并随语音设置的变更通知一起重新判定——重写发到用户自己的服务，一个永远失败的卡片只会教会用户忽略这个面板。

词库页的「词库信息」也由 Harmony 承载，对应 MSIME-Apple `FeatureSettingsViews` 里的那一节。随应用安装的词库带着一份清单，写明它是按哪套规格构建的、来自上游哪个提交；Apple 直接从 app bundle 里读，而把资源暂存进沙箱的宿主读不到，那个路径也不该告诉设置页。

只回两个字段。清单里还有 journal 模式、格式契约和全部第三方引用，没有一条回答页面在问的那个问题——"我装的是什么，它从哪来"。读不到就报错而不是猜：把词库版本说错，比不说更糟。

读的是暂存后的引擎资源而不是模块的 rawfile：暂存才是对着固定锁校验的那一步，键盘实际跑的那份副本才值得报告。

键盘的无障碍标签接上了。此前 `KeyboardView.ets` 里 `.accessibilityText()` 出现零次，而 `LetterKeyFacePolicy.accessibilityLabel`、`EnglishLetterCaseState.accessibilityLabel/accessibilityValue`、`JapaneseVariantPolicy.accessibilityLabel` 和 `CandidateGlossPolicy.accessibilitySuffix` 都已经从来源移植过来、都有单测、也都只有它们自己的测试在调用——标签算出来了，键盘一个字也不念。读屏用户面对的是一块什么都没有的键面。

键面文字越短越好，念出来往往不成词：`⇧` 是空的，`123` 是一个数，`中` 是一个字而不是一个动作。来源给每一个键都起了名字，而且名字说的是这个键会做什么，不是它显示什么。`KeyAccessibilityPolicy` 补上那四份策略没有覆盖的名字，视图现在把它们全部挂上。

空格键按它将要做的事来念：组合中它选定高亮候选，看不见候选栏的用户没有别的途径知道这个键换了含义。候选念成「候选词 N：X」，还没打完的拼音接在后面念成「还需输入 …」——这是"现在就能上屏的候选"和"还得继续打的候选"之间的区别；释义后缀走已移植的 gloss 策略，它已经决定了那是念「提示」还是念「英文释义」，那个区分不该在调用点重新推一遍。

反馈页附带的系统版本由本宿主填入。`os_version` 不属于 `HostCapabilities::for_platform` —— 它不是平台假设而是关于这台机器的事实，所以每个宿主自己读了再加上去；此前只有 macOS 这么做。缺它的时候页面写的是「平台：harmony」并附上 WebView 的 User-Agent，那标识的是浏览器内核，不是复现问题所需要的系统。现在从 `deviceInfo.osFullName` 取：`OpenHarmony-6.0.1.112` 去掉产品名后是 `6.0.1.112`，页面在它前面自己会写平台名，于是读作「HarmonyOS 6.0.1.112」。取不到合规的版本号就不填——这条字符串进的是用户提交的报告，要么照系统说的写，要么什么都不写。

候选翻译复用共享 `translation_query` 与 `apply_translations` 代际契约。Harmony 原生边界负责把 Tencent TMT、NiuTrans 和自定义 provider（DeepLX 兼容服务或 DeepL 官方 API）的签名/请求描述器及响应解析暴露给 ArkTS，设置页「测试」按钮也用同一个自定义翻译描述器，网络传输仍由 Harmony HTTPS 栈完成；本地英文词典释义先在 Engine 侧解析，在线结果只补齐缺失项。手机和平板的触屏键盘只显示一种候选翻译（与 iOS、Android 相同，第二种语言会让候选栏太高）：共享查询照偏好在 `target_languages`、`offline_gloss_languages` 和 `english_gloss` 里带上第二种语言 `translation_secondary_language`，键盘在发请求前用 `TranslationPolicy.keepPrimaryOnly` 只留主目标语言 `target_language`，预留的释义行也只按主语言算。2in1 的候选窗与桌面相同，仍显示第二种语言，设置页也只在 2in1 上给出这一项。多语言释义合并为有界的 ` / ` 展示文本，按 provider、目标语言和词条缓存，过期或 generation 不匹配的结果不会污染当前候选页。英文目标的成功释义通过共享 ABI 写入用户词库覆盖层，凭据只存在于当前请求内，不写日志。

共享设置中的“流式预编辑”现在也由 Harmony 消费：关闭时识别中的临时结果不进面板，只有最终结果才显示；此前无论该开关如何，临时结果一律显示。“结果提交策略”反过来从 Harmony 的设置页移除——Windows 在 TSF / SendInput / 粘贴之间选，macOS 在系统事件与输入会话之间选，而键盘扩展只有输入客户端一条提交路径（Linux 同样只经 IBus / Fcitx5 提交，也不提供该选项），三选一在这里是一个只有一种结果的控件。该判断由 `HostCapabilities::voice_commit_mode` 决定。

录音行为的四个共享开关现在也由 Harmony 消费——设置页的说明一直写着"录音期间的提示音与静音由输入法在本机处理"，而此前本宿主一项都不做。开始与结束提示音使用 Windows 安装包同一份 `start.mp3` / `end.mp3`（同样的字节，放进模块的 `rawfile/audios/`），经 AVPlayer 播放，播放器随每次提示音创建并释放：键盘扩展不是媒体应用，为一段不到一秒的声音常驻一条音频管线不值得。`sound_enabled` 是两个提示音之上的总开关。"录音时静音其他音频"通过 `AudioSessionManager.activateAudioSession` 以 `CONCURRENCY_PAUSE_OTHERS` 实现，录音结束或取消时 `deactivateAudioSession` 归还；该项默认关闭，从正在播放的应用手里拿走音频会话是侵入性的。取消的录音不播结束音——没有识别结果可宣告。以上任何一步失败都只记日志，不影响录音本身。

语音回复的解释逻辑抽成了 `VoiceResponsePolicy`。识别器本身握着麦克风、套接字和会话代次，没有凭据和真实音频就跑不起来；但"这条回复是什么意思"不需要两者，而且恰恰是最容易写错的部分——豆包的错误码可能出现在顶层也可能在 `payload_msg` 里，文字同样两处都可能，而没有文字的最终帧仍然必须结束录音，当成"什么都没发生"会让麦克风一直开着。这部分用合成回复逐条覆盖。

2in1 硬件键盘补齐 Windows 的五个语音快捷键：右 Alt 长按录音、Ctrl+Win 与 Ctrl+右 Alt 两个长按和弦、录音中按空格锁定（松开长按键不再结束）、Ctrl+F9 开始/停止（也用于结束已锁定的录音），录音中按 Esc 取消。三个长按和 Ctrl+F9 各受共享 `voice_input.hotkey_*` 开关控制（设置页把三个长按合成一个「长按录音」选择）；空格锁定与 Windows 一样固定为开，不读 `hotkey_hold_space_lock`，设置页也不给开关。空格与 Esc 只在录音时被占用，其余时刻仍归组合输入；长按键的重复按下不算第二次请求。录音状态由绘制识别面板的视图告知会话，识别自行结束（拿到最终结果或 provider 失败）时会清掉长按与锁定，否则下一次按下长按键会被当成一次并不存在的录音的释放。

Harmony 触屏键盘的工具栏没有语音按钮，语音从功能面板的「语音输入」图块进入，所以共享设置中的“顶部语音入口”（`touch_voice_shortcut`）在 Harmony 上没有可控制的东西，设置页在 HarmonyOS 上不画这一行。`voice_input.enabled` 关闭时，语音图块保留在原位但变灰（0.4 不透明度）、不响应点按、读屏报「不可用」，保持平台特性与 Windows 的可选语音开关一致，同时不让图块网格因一个开关而重排。

语音面板、表情面板、手写面板、浮动工具栏、2in1 候选右键菜单和键盘本身一律跟随颜色模式（`HostCapabilities::surface_themes_follow`）：共享设置里存下的 `voice_theme`、`emoji_theme`、`handwriting_theme`、`toolbar_theme`、`menu_theme`、`screen_keyboard_theme` 不再读取，手机设置页本来也不提供这几行，只有候选窗口仍按 `candidate_theme` 单独设明暗。语音面板使用全局主题 keyboard 调色板的对应明暗一套。

表情面板和手写面板分别使用全局主题 keyboard 调色板的对应明暗一套。

浮动工具栏按主题约定从 candidate 调色板派生（底色取 surface、按钮文字取 text、悬停取 hover、描边与分隔线取 border、拖动柄取 secondary；工具栏按钮以按钮面上的字表示开关状态，没有单独的开启态填色），只有主题在该明暗下带 `candidate_skin` 时才再叠加那个皮肤包的安全 CSS。

2in1 候选右键菜单同样从 candidate 调色板派生（surface 底、text 字、accent 动作、border 描边），而不是键盘调色板。触摸键盘上的长按条属于键盘本身，继续用键盘调色板。

全局主题（system / shuishan / light / paper / night / ink / custom）经 ABI 3 的 `msime_client_resolve_theme` 与主题目录接入：键盘按主题的 keyboard 调色板着色（字母键、功能键、secondary 文字各有其色，回车键始终是平台 accent 白字），2in1 候选窗与 STATUS_BAR 工具栏使用 candidate 调色板，主题带固定明暗时所有面都随之固定。`global_theme` 为 `custom` 时，`custom_theme.keyboard` 若带一份设计，键盘读取它的颜色、圆角、边框、透明度和键面字体属性；否则用 `custom_theme.base` 所指主题的 keyboard 调色板。原生主题选择器会展示同一份设计，避免设置页保存了设计但键盘仍绘制默认配色。2in1 候选窗的次要文字（译文、注释、编码提示和序号）一律用调色板自带透明度的 secondary，不再叠加额外的不透明度；编号色和边框色跟随主题和皮肤（`candidate_number_color`、`candidate_border_color` 为假），解析请求里的自定义主题去掉存下的这两个槽位（`GlobalTheme.hostDrawnCustomTheme`），选了文字颜色时编号色由它推出；组字行末尾显示「当前页 / 总页」与可点的 ‹ ›，组字行关闭时它随之隐藏。输入模式提示（中/英徽章）用主题的 accent 与 on_accent 着色。触摸候选栏空闲时是一条工具栏：最左是品牌键（水杉标志，打开功能面板，面板打开时高亮），随后是表情、常用语、剪贴板、皮肤、键盘（输入方式选择器）五个工具按钮，最右是收起键盘；中/英、方案、译、标点不再是工具栏上的胶囊，设置也不再是工具栏上的 ⚙，它们都在功能面板里。输入时候选不编号、无底色，选中项以 accent 与 600 字重区分。功能面板和输入方式选择器每页四列，可左右翻页，行数与磁贴大小与 iOS、Android 同一条规则、同一组常数（`FunctionPanelPolicy.pickerGrid`）：满尺寸是手机竖屏和平板三行、行高 52vp、行距 12vp，手机横屏（屏幕宽大于高且最短边不到 600vp，转屏时随 `display` 的 change 事件重排）两行、行距 16vp。网格按面板去掉上方留白、页点和下方留白后实际能占的高度排，永远不超出面板：放不下满尺寸时磁贴连同行距整体等比缩小，只有三行要缩到磁贴矮于 44vp（触摸目标下限）时才退成两行。键区默认 197vp，三行磁贴缩到约 0.94；最矮 176vp 时三行会矮于 44vp，退成满尺寸的两行。功能面板的图块顺序由 `FunctionPanelPolicy.ts` 决定：先是设计稿的十四个图块（全角、中文标点、模糊音、繁体、手写、词库、键盘调整（原「键盘高度」）、设置、按键音、振动、单手模式、隐私模式、反馈、关于；隐私模式关掉会话学习并把会话标为私密，打字统计、按键计数、语音时长、剪贴板历史和云剪贴板都不再记录，见 `PrivacyGate.ts`），再是 Harmony 键盘自己的候选翻译、符号、本地输入、语音输入、高情商回复、AI 润色、振动强度，与 Android `FunctionPanelModel.java` 的先例一致。这个顺序与 `guides/harmony.md` 里描述的面板不同，以代码为准。词库、反馈、关于三格直接打开设置应用的对应页面（`openSettings('dictionary' | 'feedback' | 'about')`，经 want 的 `page` 参数交给 `EntryAbility`），输入方式选择器里的「添加语言」打开「输入」页，在「语言与方案」卡片中添加和管理语言。候选翻译与模糊音写的是设置页同一份 `candidate_translations` 与 `fuzzy_pinyin.enabled`：切换译文后键盘按新值重算释义行并调整高度；模糊音先存盘再交给会话，这样首次开启时由偏好存储补上的全部规则会立即生效，关闭时已选规则原样保留。

共享设置中的触摸输入方案启用列表也由 Harmony 消费：输入方案选择器只展示启用的方案，切换当前方案时保留其余启用/禁用状态，不会因为一次选择把用户隐藏的方案重新打开。

该开关此前在共享设置页写死只给 Android，Harmony 读得到却改不了；现在由 `HostCapabilities::english_suggestions` 决定，iOS 因为把同名开关放在原生 App Group 存储里而不声明该能力、继续用它自己的那个。

直接英文输入现在有只读的英文补全，与 Android / iOS 一致，并受共享 `english_suggestions` 开关控制。查询走已有的共享 C ABI `msime_client_english_completions_request`（本次由 NAPI 导出）：它不创建 Engine 会话，因此可以离开 UI 线程。光标前的词按字母向前读到边界，全角字母归一为 ASCII——用户在全角模式下打的仍然是英文单词。少于两个字母不查询：一个字母能匹配词库的大半，却要为每次按键付一次查询。补全列表画在候选条上（直接英文没有组合，候选条本来是空的），点选前重新读取光标前的词，若编辑器已经改变就放弃，避免删掉补全从未涉及的文本。回复在进入候选条之前做长度与类型校验。

悬浮工具栏的“设置”按钮现在也读共享 `floating_toolbar.settings`：此前它是唯一关不掉的按钮，使设置页那个开关在本宿主上是一个没有结果的控件。关闭后工具栏按可见按钮重新计算宽度，和其余组件一致。

路由决策到会话调用的映射从扩展里抽成了 `HardwareKeyDispatch`。此前它是扩展内部一个二十五分支的 switch，上游路由器和下游会话方法都有测试，唯独这个接头没有——左方向键接到 `moveRight`、翻页键接到移动高亮，在今天的测试下和正确代码看起来一模一样，要等有人在 2in1 上打字才会发现。现在是一个对显式接口的纯函数（ArkTS 不支持结构化类型，所以需要把硬件键路径要求会话提供什么写成接口），`KeyboardSession` 声明实现该接口，编译器因此也会检查这份契约。二十四个动作逐条有测试，包括"被占用但不产生效果"的两个。

2in1 硬件键盘补齐 Windows 基线的三个模式快捷键：`Ctrl+Shift+E` 切换中英文状态、`Ctrl+Shift+Space` 切换全角/半角、`Ctrl+.` 切换中英文标点。Linux 宿主同样实现这三个。它们不属于设置页可关闭的那四项绑定——Windows 把它们定死，关掉 Shift 单击并没有对 `Ctrl+Shift+E` 表态。标点走的是工具栏按钮已经在用的那条偏好写入路径，而不是另起一套 live session 开关：同一个开关两套机制正是两边开始不一致的起点。`InputModeRouting` 此前没有任何测试，本次连同原有的单击修饰键判定一起补上。

维护快捷键的另一半 `Ctrl+Shift+Alt+C` 也补齐：清除当前会话的 Engine 候选缓存（NAPI 此前未导出 `msime_client_reset_cache`）。与删除候选的槽位键不同，它不要求正在组合——整张候选列表看起来过期时，恰恰常常是没在组合的时候。清的是缓存不是组合，用户已经输入的内容不受影响。

2in1 硬件键盘补齐 Windows 的维护快捷键 `Ctrl+Shift+Alt+1–8`：删除候选栏对应位置的候选词。触屏上这个动作走长按菜单，而硬件键盘没有长按这个手势，该和弦是它唯一的入口。按下后由会话校验该位置是否存在候选、以及其来源是否允许词库删除——云候选、AI、Emoji 和日语候选来自 Host API 会拒绝的来源，此时不执行；但按键仍然被占用，否则一个游离的 `1` 会落进编辑器。

共享设置中的“双拼预编辑”（`shuangpin_preedit_uses_raw`）现在也出现在 Harmony 的设置页。该偏好一直由 Engine 消费，但控件此前只给 macOS；Harmony 的组合行直接绘制 Engine 的 `editing_text`，原始双拼按键与展开拼音的区别在这里是看得见的，所以由 `HostCapabilities::shuangpin_preedit` 决定而不是平台名。设置页也补上了手写说明：手写走系统文字识别，笔迹不离开本机；设备不提供该能力时手写方案会明确提示，不回退到其他识别方式。

共享设置中的“中英文状态范围”现在也由 Harmony 消费，并由 `HostCapabilities::ime_mode_scope` 能力而非平台名决定是否出现：编辑器属性自 API 14 起带 `bundleName`，键盘据此按应用记忆中英文状态，这也是共享默认值 `app`。此前无论哪个应用都共用一个模式。`global` 保持所有输入上下文同一状态。范围在编辑器激活时读取，不在组合中途改变。密码框、地址框这类要求拉丁字母的编辑器覆盖是编辑器的选择而非用户的，不写入记忆，否则在某个应用里填过一次密码就会让之后每次进入该应用都停在英文。该映射只存在于键盘进程生命周期内，不落盘：它是一份"用户在哪些应用里打字"的记录，偏好文件没有理由携带，而忘记它的代价只是重启后多按一次切换键。最多记住 64 个应用，超出时丢弃最久未使用的。

`build-native.sh` 在本机已跑通（2026-09-20）：`arm64-v8a`（真机）、`x86_64`（模拟器）和 `armeabi-v7a` 三个 ABI 的 Rust/C++/NAPI 交叉构建全部成功，各产出 `libmsimeclient.so`、`libmsime_host_api.so` 和 `libc++_shared.so`。`hvigorw assembleHap` 生成的是未压缩的中间 HAP；发布工作流随后只对 `resources/` 条目使用 DEFLATE，保留原生库、Ark 字节码和 `resources.index` 的布局。以 `harmony-v0.2.0` 为基线，发布 HAP 从 328,655,000 字节降到 197,712,995 字节，`msime-client-core` 对 `aarch64-unknown-linux-ohos` 的 `cargo check` 亦通过。

Engine 换成纯 Rust crate 之后（2026-09-30），同一脚本在三个 ABI 上重新跑通，除 NDK 之外不再需要任何准备：以前要手工编译的 `libsqlite3.a` 前缀和 32 位 `armeabi-v7a` 需要的 fmt/spdlog 替身 config 都随 C++ Engine 一起去掉了，`libmsime_host_api.so` 的动态依赖只剩 `libc.so`。这一轮没有重新打 HAP，也没有上设备。

## 首次启动的隐私政策授权

设置应用首次启动时在启动画面上弹出原生授权弹窗（`privacy/PrivacyConsentDialog.ets`，在 `pages/Settings.ets` 的 `onPageShow` 打开）：几句话说明处理什么、为什么，可以点开的《用户协议》和《隐私政策》（`https://msime.app/terms/`、`https://msime.app/privacy/`，经 `openExternalUrl` 交给系统浏览器），「不同意」「同意」两个按钮。首次弹窗不能用返回手势关掉。协议列表是 `PrivacyConsentPolicy.DOCUMENTS`，只列已经上线、点得开的页面。

选择记在设置应用的 `files/state/privacy-consent.json`（`{format, version, decision, decided_at_ms}`），经共享沙箱的 `privacy-consent` 条目镜像给键盘；键盘在扩展创建和每次获得焦点导入镜像后重读。只有同意了当前版本（`PRIVACY_POLICY_VERSION`）才联网：隐私政策或用户协议有实质变化时把它加一，已同意旧版本的用户下次打开应用重新弹窗。弹窗一次征得两份文档的同意，常量虽然名叫隐私政策版本，用户协议的重要变更（msime.app/terms/「协议的接受与变更」里要再次征得同意的那一类）也靠它重新弹窗。拒绝之后不再自动弹窗，用户在「我的 › 隐私」（2in1 上在「关于 › 法律信息」）的「隐私政策授权」一行重新打开弹窗同意或撤回。升级上来的老用户没有记录，按首次启动处理，打开应用时弹一次。

没有授权时两个进程都不发受政策约束的请求：使用统计与崩溃记录、匿名账号注册、公告、账号登录（华为账号、邮箱、手机号）、反馈、对话、设置同步、云词库、云剪贴板、词库同步（`dictionary_sync`，只有 `status` 不要授权）、统计上传、社区、AI 皮肤、AI 与凭据测试、检查更新、词库更新、本地语音模型下载（设置桥接 `startRequest` 与 `dictionary` 回 `privacy_consent_required`，页面不画这些入口），以及键盘里的云联想、AI 联想、在线翻译、高情商回复、豆包与 HTTP 云端语音、语音润色和云剪贴板。本机打字、词库、皮肤、插件、统计、手写、系统语音识别和本地语音模型照常可用。规则在 `privacy/PrivacyConsentPolicy.ts`（`tests/privacy-consent.test.ts`），调用点由 `scripts/test-harmony-privacy-consent-gates.py` 守着。模拟器上没有共享沙箱，键盘读不到授权，键盘一侧的云端功能在模拟器上始终关着。决定与备选见 `.agents/notes/implemented/feature/2026-10-27-harmony-privacy-first-launch.md`。

## 防止截屏（#10194）

「我的 › 隐私」和「关于 › 许可与隐私」里的「截屏与录屏 / 防止截屏」，默认关闭，规则与 Android（#10159）相同：第一次打开不需要验证；打开过以后，再开、再关都要通过锁屏密码或生物识别（`keyboard/settings/ScreenCaptureAuthentication.ets`，ATL1，不带冷却）；没设锁屏密码时一律打不开，否则打开后再也关不掉。规则在 `keyboard/settings/ScreenCaptureProtectionPolicy.ts`（`tests/screen-capture-protection.test.ts`）。

开关写在设置应用的 `files/state/screen-capture-protection.json`，不在共享偏好里，不随账号同步；经共享沙箱的 `screen-capture-protection` 条目单向镜像给键盘，键盘不发布它。打开后设置应用的主窗口设 `setWindowPrivacyMode`（启动时在载入页面之前，切换时当场），键盘的软键盘（2in1 上是候选窗）、浮动工具条和输入模式提示三块面板设 `Panel.setPrivacyMode`：面板新建时各设一次，每次编辑框获得焦点、导入镜像之后再按开关重设，所以在设置应用里切换后，下一次弹出键盘生效。两者都要 `ohos.permission.PRIVACY_WINDOW`（normal 级、system_grant，`module.json5` 里声明即授予）；设置失败只记日志。共享页的「恢复默认设置」只恢复共享偏好，不碰这个开关。决定与和 Android 的不同见 `.agents/notes/implemented/feature/2026-10-29-harmony-screen-capture-protection.md`。

尚未在真机上验证截屏、录屏是否变黑和认证弹窗的行为。

## 华为账号登录：Account Kit 的 ID Token

设置页的「登录水杉」面板（手机）和登录卡片（2in1）在后端开着华为账号（`/v1/auth/providers` 返回 `huawei: true`）时显示「华为账号登录」，与其他登录方式一样要先勾选同意《用户协议》和《隐私政策》。点下去以后 `account/HarmonyHuaweiSignIn.ets` 经 Rust `msime_client_account` 的 `huawei_challenge` 申请带 nonce 的 challenge，用 `authentication.AuthorizationWithHuaweiIDRequest`（`scopes: ["openid"]`、`permissions: ["idtoken"]`、`forceAuthorization: true`、nonce 原样传入、`idTokenSignAlgorithm: PS256`）拉起系统的华为账号授权页，拿到的 ID Token 经 `huawei_login` 交给后端 `/v1/auth/login`，会话由 Rust 存进 `files/state`。后端用华为公布的 JWKS 验签并核对发行方、受众（应用 Client ID）、有效期和 nonce，按 ID Token 的 `sub`（UnionID）认人；不换授权码，客户端和后端都不需要 Client Secret。challenge 回答、nonce 形状、响应核对和错误码换算在可测的 `HuaweiIdSignInPolicy.ts`（`tests/huawei-sign-in.test.ts`）。

只要 `openid` 不需要在 AppGallery Connect 申请账号权限；`compatibleSdkVersion` 是 6.0.1(21)，不需要配置公钥指纹（华为文档「配置签名和指纹」：compatibleSdkVersion ≥ 20 不需要）。需要的配置只有 Client ID：AGC「常规 > 应用」里应用的 Client ID 与 APP ID 不同时，要在 `entry/src/main/module.json5` 的 module 级 `metadata` 里写 `{"name": "client_id", "value": "<应用 Client ID>"}`（直接写数字，不能用 `$string` 引用），`scripts/test-harmony-manifest.py` 核对它的位置和形状；后端 `auth.huawei.client_ids` 要列同一个值。没有设备上的真实账号验收之前，这条路径只经过逻辑测试、HAP 编译和后端用本地签名令牌的测试。决定与备选见 `.agents/notes/implemented/feature/2026-10-27-harmony-huawei-id-sign-in.md`。

用户关掉授权页时 Account Kit 以取消结束，面板不提示；网络错误提示检查网络；指纹证书或权限错误（1001500001、1001502014）是 AGC 配置问题，日志里单独写明，页面显示「账号服务暂不可用」。

## Google 登录：系统浏览器加本机回环（默认不提供）

中国大陆连不上 Google，AppGallery 审核按「使用 Google 登录」点了没反应提过意见，所以鸿蒙设置应用默认不给共享账号页提供 `googleLogin`（`apps/harmony/src/main.tsx` 的 `HARMONY_GOOGLE_SIGN_IN`），面板上不再出现 Google 按钮，第三方登录改由上面的华为账号登录提供。下面的回环实现原样保留，把那个开关改回 true 就恢复；其他平台的 Google 登录不受影响。

设置页的「登录水杉」面板在后端开着 Google（`/v1/auth/providers` 返回 `google: true`）且宿主提供 `googleLogin` 时显示「使用 Google 登录」。鸿蒙没有 Google 服务，GoogleSignIn SDK 用不了；Google 也拒绝在应用内的 WebView 里登录。所以走与桌面端同一条流程（RFC 8252）：设置应用在 `127.0.0.1` 的随机动态端口上用 `TCPSocketServer` 监听，经 Rust `msime_client_account` 的 `google_challenge {target}` 申请 Google challenge（`target` 是这个回环地址，Rust 校验过授权链接是指向它的 Google 授权链接），用 `openLink` 把授权链接交给系统浏览器，Google 跳回本机端口后把授权码经 `google_code_login` 交给后端去换 token，会话由 Rust 存进 `files/state`。PKCE 和 client secret 都在后端，后端不需要为鸿蒙做任何改动。

哪些判定不在 ArkTS 里写：回跳地址的形状、授权链接能不能打开、等多久、一次回环请求是不是回跳、写回浏览器的页面，都由 C ABI `msime_client_google_loopback`（NAPI `googleLoopback`）给出，它和桌面端 `AccountSession::sign_in_google_with_browser` 用的是 `crates/client-services/src/account/google.rs` 里同一份代码。`account/HarmonyGoogleSignIn.ets` 只做 socket、浏览器和后台保活，请求头的拼接与截断在可测的 `GoogleLoopbackRequestHead.ts`。

用户在浏览器里登录时设置应用在后台，系统可能冻结它，回跳就没人接，所以等待期间申请延迟挂起（`requestSuspendDelay`），结束时撤销。2026-10-08 在 HarmonyOS 6.0.2 模拟器上测过：浏览器停在页面上 120 秒后才回跳，后台的应用照样立即收到；不申请延迟挂起、等 60 秒也能收到，但模拟器的冻结可能比真机宽松，真机上的表现还要验证。最长等待是 challenge 寿命减去 30 秒（目前 270 秒），超时与用户在 Google 页面拒绝一样按取消处理；面板上的「取消 Google 登录」和关闭面板也会结束等待并关掉监听。页面那一侧的请求超时为此放宽到 6 分钟（`GOOGLE_SIGN_IN_TIMEOUT_MS`），默认的 30 秒会在用户还在浏览器里时就报超时。

失败时日志会写明停在哪一步：没有端口、challenge 被拒、授权链接没通过校验、没等到授权码（含超时和取消）、授权码到了后端被拒，记的是 Rust 给的账号错误码，不记令牌或授权码。

## 应用图标切换：本平台没有这个能力

Apple 的 `AppIconSettingsView` 和 Android 的同名入口在共享页面上是 `SettingsClient.appIcon`，本宿主不声明它，于是那个控件不出现。这不是还没接，是公开 SDK 里没有对应的 API。

在 API 24 的 `command-line-tools/sdk/default/openharmony/ets` 上查过：`@ohos.bundle.bundleManager` 对外只有 `canOpenLink`、`cleanBundleCacheFilesForSelf`、`getAbilityInfo`、`getAppCloneIdentity`、`getBundleInfo*`、`getBundleNameByUid*`、`getLaunchWant*`、`getPluginBundlePathForSelf`、`getProfileBy*` 和 `getSignatureInfo`——对自身是只读的，加上链接与分身的辅助；`@ohos.bundle.shortcutManager` 只有 `getAllShortcutInfoForSelf` 和 `setShortcutVisibleForSelf`。整个 `api/` 与 `kits/` 下没有 `setAbilityEnabled`、没有 alternate/dynamic icon 的任何形式。iOS 用 `setAlternateIconName`，Android 用 activity-alias 加 `setComponentEnabledSetting`，两条路在这里都没有对应物。

所以这是按平台特性裁剪，而不是欠账：能力模型的用途正是让页面不画一个保存了却什么都不做的开关。如果将来 SDK 提供了对应 API，接法是声明 `appIcon` 并在 `module.json5` 里补上备用入口 ability——那时需要的是真机验证，不是这里的接线。

## 释义到达时候选条整体右移

开着释义时，候选格的宽度此前跟着内容走：联网那份是几百毫秒后陆续到的，一条到了它那一格就变宽，右边的候选整体位移——打字过程中候选一路往右挪。来源把这件事和「格子从一行变两行、候选忽高忽低」并称为同一件事的两半，解法是**先按可见宽度分格，答案回来往格子里填**（`AChipKeepsItsWidthWhateverTheGlossTurnsOutToBe`）。

`CandidateChipWidth` 照搬了那套算术：可见宽度去掉格间距、再去掉一格自己的左右内边距，除以三。三格而不是四格是来源写明的理由——四格时一格只剩七十来点，`draft; draw up` 这种就得截尾，而释义截了等于没写。**候选词本身永不截断**：比一格宽的词把自己那一格撑开，列宽是带释义的格子的下限，不是词的上限。

预留看的是**设置**而不是「答案到没到」——`KeyboardSession.glossesExpected()`。按到达与否来留，等于又把宽度交还给网络。

后续按来源 `GlossTakesItsOwnLineUnderTheCandidate`、`AChipKeepsItsHeightWhileTheTranslationIsStillOnItsWay` 与 `TheKeyboardGrowsByTheRowsTheStripReserves` 补齐了另一半：手机和横排 2-in-1 候选把释义放在词条下方，按设置与可用 provider 在请求发出前预留固定第二行，原生 panel 同步增加同样的高度；大候选字号会把释义行一起撑高，不截字。纵向 2-in-1 列表仍按来源纵向样式把释义放在旁边，不额外增高。

这次断言审计还找出两处比样式更直接的错误。第一，在线 `candidate_translations` 的结果被误绑到独立的 `candidate_english_gloss` 开关；关掉随包英文释义后，网络结果已经写回 Engine 却永远不画。现在展示门控是两者的并集，是否预留空行则分别按「目标语言含英语」和「已配置可用 provider」判断。第二，无释义时实现返回的是 `width('100%')`，与“按词宽”的注释和 `CandidateChipWidth.content(..., false, ...)` 测试相反，横排因此一屏只有一个候选；现在走同一份词宽算术。`scripts/test-harmony-candidate-translation.py` 固定这条宿主接线，纯逻辑套件固定 provider、空占位、面板高度和纵向适配。

## 候选英文音标

「显示读音」读共享偏好 `candidate_pronunciation`（默认关），设置页在手机「输入 › 翻译」和 2in1 的表达页一直显示这一行（`apps/harmony` 给 `candidatePronunciation`）：英文候选的音标不看释义开关，关掉释义以后仍要能把它关掉。鸿蒙没有资源包下载，ECDICT 生成的 `en-phonetic.db`（约 5.7 MB，MIT）连同 NOTICE 随 HAP 分发：`stage-resources.sh` 第五个参数（默认 `target/pronunciations`，`scripts/fetch_gloss_tables.py` 取来）按 `resources/pronunciations.lock.json` 核对后放进 `resfile/pronunciations`，发版设 `MSIME_REQUIRE_PRONUNCIATIONS=1`，键盘扩展把它暂存到引擎资源旁。`KeyboardSession` 在发布候选前经 NAPI `candidatePronunciations` 同步查还没缓存的文字（英文候选读它本身，中文候选读英语释义的第一行，规则在 `CandidatePronunciationPolicy`），按文字缓存，`CandidateEntry.pronunciation` 带给视图。音标画在释义下面单独一行，按设置预留：`glossRows()` 是 `translationRows()` 加 `pronunciationRows()`；触屏胶囊只放得下词和一行注释，两者都开时 `KeyboardMetrics.touchExtraDetailVp` 让胶囊和工具条各高 11.5vp。2in1 竖排把音标接在旁边，`CandidateWidthPolicy.widthVp` 按音标加宽窗口；只为音标预留了一行（`translationRows()` 为 0）时，横排候选格和触屏胶囊都把译文画回词旁边。带控制字符的释义不查，免得共享层拒绝整批。直接输入英文时的补全不标音标（Android 会标）。展开网格不画释义，也不画音标。见 `.agents/notes/implemented/feature/2026-11-01-mobile-candidate-pronunciation.md`。

法、日、西、俄、德、韩语目标的释义行读外语读音表（维基词典生成，CC BY-SA 4.0，约 3.6 MB）：`stage-resources.sh` 第六个参数（默认 `target/target-pronunciations`）按 `resources/target-pronunciations.lock.json` 核对后连同 NOTICE 放进 `resfile/target-pronunciations`，`MSIME_REQUIRE_PRONUNCIATIONS=1` 时也必须在，键盘扩展同样暂存到引擎资源旁；主目标语言那一行按它的语言查，日语给罗马字。见 `.agents/notes/implemented/feature/2026-11-01-target-language-gloss-readings.md`。

## 展开候选面板里没有释义，长按也没有反应

上一条把释义接到了候选条的长按菜单上。来源还有两条与之配套：`TheExpandedPanelDrawsTheSameGlossesAsTheStrip` 和 `TheExpandedPanelAnswersALongPressToo`——候选多到要展开时，那一页同样画释义、同样答应长按。

这边两条都不成立，而且是同一个原因：`allCandidates()` 从原生拿到完整候选记录（`source`、`fixed_position`、`annotation`、`translation` 都在），却只保留了 `text`。于是展开面板列着一串词，旁边的候选条却给每个词写着释义；长按也无从谈起，因为没有可读的东西。现在它返回 `ExpandedCandidate`——文字加释义加「这条释义是不是译文」。

**展开面板只给释义，不给词条管理。** 这不是照抄来源的取舍，是这边的下标空间决定的：管理操作走的是候选条那一页的位置，而展开面板经 `selectAnyCandidate` 用的是整份列表里的位置，两者是不同的原生入口。在这里放管理项，就是拿一份列表的编号去索引另一份。来源的展开面板同样只答应释义菜单，两边因此落在同一处。

`ExpandedCandidate` 刻意比 `CandidateEntry` 窄，只有面板用得上的三个字段：把 `fixedPosition` 和 `source` 一起带过来，正是在邀请上面那种串号。

顺带修掉换行估宽里的一处：原来按 `candidate.length` 算列数，那是 UTF-16 单元数，一个 emoji 会被当成两列。现在按码点数。

## 九键外框的底行和 1 键

触屏的拼音九键和笔画键盘共用九键外框的底行（`nineKeyActionRow`，键序和份额在 `NineKeyLayout.actionRow`）：`符 1.05 | 123 1.25 | 空格 4.2 | 中/英 1.05 | 回车 1.6`，与 iOS、Android 相同。符 打开符号面板，九键上打开面板只走它和长按 123。拼音网格的数字层只有 1–9，0 补在底行：拼音九键、九键双拼和笔画的数字层是 `符 1.05 | 九键 1.25 | 空格 3.15 | 0 1.05 | 中/英 | 回车`，符 和切换键与字母层同一格（#10061，与 Android #9113、iOS 相同），0 的一份从空格里让出；从 26 键、14 键、18 键、手写借来的数字九宫格是 `返回 | 空格 | 0 | 中/英 | 回车`，返回键在它们 123 那一格，0 占 符 让出的同一份宽度。假名网格、手写和 2in1 屏幕键盘仍用共享的 `classicActionRow`。

触屏 26 键一族（全拼、双拼、五笔、英文、韩文、越南语等）的字母层设计底行（`designActionRow`）为 `123 1.25 | 符 1.05 | 空格 4 | ， 1 | 中/英 1.05 | 回车 1.9`，逗号长按直接输入句号，不再放独立句号键，中英文模式相同（`KeyboardScheme.designRowUsesCommaSecondaryPeriod`）。符 同样打开符号面板、记作 `SoftSymbol`。14 键（第三行左端已有 符）、大千注音和它沿用的共享符号行仍是 `123 | ， | 空格 | 。 | 中/英 | 回车`，123 / #+= 层自带底行。

「按键布局」页（共享 React 设置页）还有两项（#10056，共享偏好、随账号同步，同步键 `platform.harmony.twenty_six_symbol_first` / `platform.harmony.number_pad_space`）：26 键分段卡片里、当前样式说明下面的「26 键「符」与「123」互换」（`touch_twenty_six_symbol_first`，默认关）把 26 键一族设计底行的 符 排到 123 左边（`TouchLayoutStylePolicy.symbolFirst`，长空格和表情键没有 符、不变），从它借来的数字九宫格带上 符、按九键一族的次序排（`KeyboardView.letterRowSymbolLeads`）；「数字键盘」一组（`touch_number_pad_space`）选空格在右列、在 0 右边或 0 左边，缺键或 `platform_default` 是鸿蒙原来的「空格在 0 左边」（`NineKeyLayout.actionRow` 的 `space`）。空格在右列时网格右列最下面一格从 ！ 换成空格（`touchNineKeyGrid`），右侧大回车不受影响。见 `.agents/notes/implemented/feature/2026-10-29-touch-symbol-first-and-number-pad-space.md`。

九键左侧符号栏可以自定义（#10223，共享偏好、随账号同步，同步键 `platform.harmony.nine_key_symbols` / `platform.harmony.nine_key_digit_symbols`，值是空格分隔的写法，空串是出厂表）：「按键布局」9 键分段卡片里的「九键左侧符号」改字母层那张表（`touch_nine_key_sidebar_symbols`，拼音九键、九键双拼和笔画共用），「数字键盘」一组里的「数字键盘左侧符号」改所有九宫格数字键盘 #+= 上面那张表（`touch_nine_key_digit_sidebar_symbols`，九键一族的数字层和 26 键、14/18 键、手写借来的数字九宫格都读）。空表是鸿蒙出厂表，键盘照原来的画法：字母层 `, . ?`（显示与编辑器里写作 ，。？，点按交给 Engine 的标点处理），右侧大回车接 !，笔画的标点栏四格，数字层四则运算加 #+= 五格等高。用户的表画成可滚动的等高栈（`NineKeyLayout.sidebarItems`）：字母层一屏最多 3 项（右侧大回车也不再接 !），数字层是四格高的滚动区加最下面一格 #+=、一屏最多 4 项，每项不矮于 28 vp。自定义项点按先按首选结束组字再字面上屏，不做全半角转换，`（）`、`()` 这类整对符号光标停在中间（`KeyboardSession.insertSidebarSymbol`、`PairedPunctuationPolicy.literalPairClosing`，配对表照 Android）；统计仍记 `SoftPunctuation`。长按左列任一项打开设置「屏幕键盘」页的「按键布局」，直接进入这一张表的编辑页（字母层和笔画进字母表并切到 9 键分段，数字层进数字表）：键盘打开设置的 Want 除 `page` 外带一个可选的 `target`，值是那张表的共享偏好名，与 Android 的 `edit_sidebar_key` 相同（`SettingsPageLink.SCREEN_KEYBOARD_TARGETS`）；只带 `page` 的链接照旧只打开页面，`target` 不属于这一页或认不得时丢掉。英文九键、2in1 屏幕键盘不读这两张表。

每个分段的样式卡片里还有「中/英键放在空格左边」（#10228，共享偏好 `touch_language_key_side`，`right` 默认、`left` 时打开，同步键 `platform.harmony.language_key_side`，26 键分段里排在互换开关之后）：样式和互换排完之后把 中/英 左边紧挨着的那一串空格、0、逗号和句号整串右移一格，中/英 挪到这一串最左边，份额不变（`TouchLayoutStylePolicy.languageBeforeSpace`、`NineKeyLayout.actionRow` 的 `languageLeft`）。空格左边没有标点的行 中/英 紧挨空格（`123 | 符 | 中/英 | 空格 | ， | 回车`、`符 | 123 | 中/英 | 空格 | 回车`），有标点的行标点仍紧挨空格（`123 | 符 | 中/英 | ， | 空格 | 。 | 回车`）。设计底行（26 键一族、14/18 键、大千注音）、九键外框和它的数字层、借来的数字九宫格、右侧大回车网格下方那一段和手写底行都算，2in1 底行和假名网格不动；中/英 离开回车后回车不再让出键帽，密码框和数字框不退回。见 `.agents/notes/implemented/feature/2026-10-29-language-key-left-of-space.md`。

拼音网格左上角的 @#（1 键）不打开符号面板：短按把当前模式的常用标点放进候选条，就是逗号长按的那一组（`QuickPunctuationPolicy.entries`，中文模式是 `， 。 ？ ！ 、 ； ：`），组字中也一样。点选一个与侧栏标点一样交给 Engine 的标点处理（组字中先上屏首选），候选条随即清空；Engine 下一次回报候选（`onChanged`）或打开别的界面时换回 Engine 的候选。长按仍输入 1。

「九键滑动输入数字」（#10229，共享偏好 `touch_nine_key_swipe_digits`，`off` / `up` / `down`，默认 `off`，同步键 `platform.harmony.nine_key_swipe`）在设置页「屏幕键盘 › 手势」里、紧跟 26 键的「滑动方向」，只在触屏宿主且版本提供全拼九键时列出。选了方向后，拼音九键字母层 1–9 键（`nineKeyHoldKey`，两处网格都经它）沿这个方向滑过 24 vp 的那一刻键面换成这个键的数字、角标藏起来，松手按长按选数字的同一条路径上屏（先结束组字再字面上屏），按键音、振动和键位统计与点按这一格相同；反方向滑过 24 vp 弹出和长按相同的数字和字母（1 键没有字母，反方向什么也不做）。判定在 `input/NineKeySwipePolicy.ts`，与 Android `NineKeySwipePolicy.gesture` 逐条相同：只看纵向位移、严格大于阈值、一次触摸只判定一次，滑出去之后点击和长按都不再做事，长按先弹出时不再判定，系统取消时什么也不输入。阈值固定 24 vp；Android 的阈值是本机「九键滑动距离」（14–48 dp，默认 24），鸿蒙和 iOS 没有这个滑块，这是三端唯一的差异。数字层、九键双拼、笔画和借来的数字九宫格不受影响；2in1 没有这项设置，同步过来的值也不在 2in1 上判定。

## 大屏单手侧边数字键盘

平板和折叠屏内屏上单手模式把「侧栏 + 间距 + 按键」缩成手机大小的一块（按键 360vp，块宽 `round((360 + 6) / 0.85) = 431`，`OneHandedPolicy.blockWidthVp`，#10204），贴到按键那一侧；空出来的那一侧可以放一块数字键盘（#10203）。开关在共享 React 设置页「屏幕键盘 › 布局」的「大屏单手时侧边显示数字键盘」，写共享偏好 `touch_one_handed_side_panel`（`off` 默认 / `number_pad`，随账号同步，同步键 `platform.harmony.one_handed_side_panel`）；`KeyboardSession` 在读 `touch_number_pad_space` 的同一处读它，不认识的值（以后的 `text_edit`）按空着画（`OneHandedPolicy.sidePanel`），client-core 把写法原样留着。见 `.agents/notes/implemented/feature/2026-10-29-tablet-one-hand-side-number-pad.md`。

- 什么时候画（`OneHandedPolicy.sidePadShown`）：偏好是 `number_pad`；单手模式此刻生效（`OneHandedPolicy.active`，2in1 不生效）；大屏且空出来的宽度放得下；主键区画的是拼音九键、九键双拼或笔画的字母层（`KeyboardView.nineKeyFamilyLettersShowing`：123 层、英文、本地模式、注音、假名九键、手写、14/18 键和 26 键都不画）。符号、表情、剪贴板这些面板盖住按键时，面板和按键在同一行（`KeyboardView.handedKeys`），整行不画，侧边数字键盘跟着不见。
- 宽度（`OneHandedPolicy.sidePadWidthVp`）：`side = (屏幕宽 - 2 × 4) - 431 - 6`，8 是键盘根的左右内边距 `KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP`，6 是面板与块之间 `Blank(RAIL_GAP_VP)` 的最小宽度；不到 150 不画，最宽 300。600vp 的设备上是 155。`readOrientation` 在显示变化（转屏、折叠屏展开和折叠）时重算。
- 摆放：面板贴屏幕边，键在右时这一行是 [面板, 空白, 块]，键在左时是 [块, 空白, 面板]，中间的 `Blank` 是不接触摸的键盘底色。面板显示时这一行按底边对齐。
- 键面（`KeyboardView.oneHandSidePad`）：3 × 4 拨号盘 `1 2 3 / 4 5 6 / 7 8 9 / . 0 ⌫`，前三行高 `gridHeight()`、第四行高 `rowHeight(3)`，数字和 `.` 用 `fixedKey`，⌫ 用 `deleteKey`（带连删）。拼音九键和九键双拼的网格是三行，逐行对齐；笔画网格只有两行，面板的前三行平分网格高度，没法逐行对齐。数字和 `.` 走数字层的 `insertNumberPadLiteral`（组字中先把拼音原样上屏），读屏是「数字 N」「小数点」，统计记普通数字键、`Period` 和 `Backspace`。面板 `.id('oneHandSidePad')`。
- 设置桥的能力位（`SettingsFormFactorCapabilities.resolve(deviceType, foldable)` 的 `oneHandedSidePanel`，报给共享设置页的 `touch_one_handed_side_panel`）：平板为真，手机在 `display.isFoldable()` 为真时为真（折叠状态下也列出，展开后生效），直板手机和 2in1 为假；`isFoldable` 出错时按不可折叠并记日志。鸿蒙平板走设置页的手机排法，这一行照样带说明，标题和说明与 Android、iOS 逐字相同。
- 测试：`tests/keyboard-logic.test.ts` 覆盖宽度、取值、显示条件、键序、读屏名称和能力位，`tests/run.sh` 钉住接线；共享设置页的 `apps/desktop/tests/settings/harmony-one-handed-side-panel.test.tsx`。

## 大屏分离式键盘

平板和折叠屏内屏横屏时，26 键一族（全拼、双拼、五笔、英文、日文罗马字、粤拼、越南语、藏文）和韩文的键盘可以分成左右两半（#10224），规则逐条移植自 Android 的 `SplitKeyboardPolicy`（`keyboard/SplitKeyboardPolicy.ts`）。开关在共享 React 设置页「屏幕键盘 › 布局」，四行与 Android 原生设置页逐字相同：「分离式键盘」（共享偏好 `touch_keyboard_split`，默认关，随账号同步，同步键 `platform.harmony.split_keyboard`，与 Android 的 `platform.android.split_keyboard` 各管各的设备）、「竖屏也分离」（`touch_keyboard_split_portrait`，与 Android 共用这个共享偏好）、「分离启用宽度」（`touch_keyboard_split_threshold`，480–1000vp、步进 20、默认 600）、「分离中间空隙」（`touch_keyboard_split_gap`，10–40%、步进 5、默认 25）。后三项只留本机，主开关关着时不可调。

- 什么时候分（`SplitKeyboardPolicy.drawn`）：主开关打开，屏幕最短边不小于启用宽度，横屏或竖屏选项打开，不是 2in1，键盘没有浮动，正在画的键面能分（`KeyboardView.splitsFace`）。九键、14/18 键、笔画、注音、手写、假名九键不分，它们的符号页和借来的数字九宫格也不分。`readOrientation` 在转屏、折叠屏展开和折叠时记下屏幕宽高，分离与否随之重算，偏好不动。
- 怎么分：字母三排、数字行、补充行、123 / #+= 层和两种底行都在键边界上断开，中间插一段不接触摸、读屏跳过的空隙（`splitGap`）。空隙至少占整行的「分离中间空隙」，两侧原始键区合计不超过 720vp，更宽的屏幕把多出来的宽度留在中间。字母第二、三排在右半边内侧重复 G、V（asdfg | ghjkl、⇧zxcv | vbnm⌫），重复的键从空隙借一个键宽，三排键宽不变；重复的键输入与左边那个相同，但不记滑行键位。底行的空格拆成两个，左边那个的份额让空隙落在整行中间，每一半至少保留原宽度的 15%。键盘高度不变。
- 单手模式在分离时让位（`oneHandActive`），存着的值不变，功能面板的单手磁贴置灰。
- 浮动键盘优先：浮动时不分离，分离设置不变，停靠后按原样回来；分离画着时浮动磁贴照常可用。与 Android `splitKeyboardDrawn` 在 `floatingDrawn()` 时返回 false 相同。
- 两个空格共用同一套空格手势状态（光标拖动、长按语音）：两根拇指同时按住两个空格时后按下的会接管，与 Android 每个空格各自一份状态不同。
- 设置桥的能力位（`SettingsFormFactorCapabilities` 的 `splitKeyboard`，报给共享设置页的 `touch_keyboard_split`）与侧边数字键盘相同：平板和可折叠手机为真，直板手机和 2in1 为假。
- 测试：`tests/split-keyboard.test.ts` 覆盖读法、启用条件、几何和能力位，`tests/run.sh` 钉住接线；共享设置页的 `apps/desktop/tests/settings/harmony-split-keyboard.test.tsx`。见 `.agents/notes/implemented/feature/2026-10-30-split-keyboard-iphone-harmony.md`。

## 键盘调整

功能面板的「键盘调整」磁贴（原「键盘高度」，图标和位置不变，id 仍是 `keyboard_height`）打开一层盖在整块键盘上的遮罩，取代原来替换工具栏的高度条 `HeightBar`。Android、iOS 按同一份契约实现，三端界面和行为一致，决定见 `.agents/notes/implemented/feature/2026-11-01-harmony-keyboard-adjust-overlay.md`。

- 样子（`KeyboardView.adjustOverlay`）：键盘外框上盖约 45% 不透明的黑色（`KeyboardAdjustPolicy.MASK_COLOR`），下面的工具栏和按键照常画作实时预览，整层挡住触摸；四角是皮肤强调色（`skin.accent`）的直角括号。上沿、下沿正中各一个三道横线的把手，左右两沿垂直居中各一个两道竖线的把手，中间一排白色的「重置」「移动」「确认」。
- 每个柄只动自己那条边，对边在屏幕上不动（下沿往上拖过高度下限时例外；换算在 `keyboard/KeyboardAdjustPolicy.ts`）：上沿改高度（往上拖变高，1vp 对 1vp）；下沿改底部抬高：往上拖时抬高跟着手指走、高度反向让出同样多，上沿不动，高度到下限 −12 后整块键盘继续往上，直到抬高到 160；往下拖时抬高减、高度加，上沿不动，抬高到 0 或高度到 48 时停（`dragBottom`）；左右沿改宽度并同时改水平位置，另一侧边不动；「移动」按住拖动整块键盘，同时改底部抬高和水平位置，大小不变。拖动按屏幕坐标（`TouchObject.displayX` / `displayY`）算，柄跟着面板移动也不影响。
- 四项：高度仍是共享偏好 `touch_keyboard_height_adjustment`（-12..48vp）；底部抬高 `bottomLift`（0–160vp，缺省 0）、宽度 `widthPercent`（面板宽的 60–100%，缺省 100，拖动时另有 240vp 下限，取两者中更宽的）、水平位置 `positionX`（两侧剩余空间里左侧所占的千分比，0–1000，缺省 500 居中）存在本机的 `key-feedback.json`（`KeyboardFeedback`），与 `oneHanded`、`floatingX/Y` 同处，不进共享偏好、不同步，设置页和账号同步写这个文件时经 `KeyboardFeedbackBridge.fromShared` 沿用它们。设置页只有「键盘高度」滑块，没有另外三项。
- 生效条件（`KeyboardAdjustPolicy.availability`）：浮动键盘、手机横屏和 2in1 上底部抬高、宽度、位置都不生效，只画上沿柄、不画「移动」；单手模式或分离式键盘画着时宽度和位置不生效，不画左右柄，「移动」只改抬高。不生效时存着的值不变，条件变了自动回来。
- 布局：宽度和位置作用于整块键盘，触屏根节点的左右内边距在 4vp 之外多出 `adjustFrame` 的左右两段，底部内边距在 6vp 之外多出抬高；让出的地方只有键盘底色，点了不落到按键。宽度和位置按 `keyboardStack` 量到的面板宽换算（`KeyboardView.adjustPanelWidthVp`），不按屏幕宽：停靠面板的宽度是扩展能力建面板时的屏幕宽，平板转屏、折叠屏开合之后与此刻的屏幕宽不同。中间一排按钮与 iOS 一样 72vp、间距 28vp，窄外框里按 `KeyboardAdjustPolicy.buttonRow` 先收间距再收按钮，不压到左右柄。上沿柄的横线贴着外框上沿画在工具栏图标上方；浮动时下面两角的括号按卡片圆角画。面板仍是建面板时的整屏宽，高度是 `KeyboardMetrics.totalHeightVp` 加抬高（最后一个参数），冷启动第一帧由扩展能力按 `KeyboardSession.firstTouchLift` 算进去，视图从它出发核对。转屏或开关浮动键盘让抬高生效或不生效时，`syncStripHeight` 改面板高度。
- 重置把四项预览回默认（高度 0、抬高 0、宽度 100%、居中），不保存；确认保存并退出。保存时高度一直写，抬高、宽度、位置只写此刻生效且和打开时不同的项（`KeyboardAdjustPolicy.saved`，与 Android `AndroidLocalSettings.keyboardAdjustEdits` 同一条规则）：在手机横屏或浮动键盘里点了重置，存着的抬高、宽度、位置不变。没有「取消」：其他任何离开方式（收起键盘、换输入框、切应用、切到别的面板）都按确认保存，都经 `KeyboardView.show` 的同一处；换输入框时系统可能只来新的 `inputStart`，会话的 `attach` 经 `onEditorAttached` 让视图离开。调整中途设置页保存引起的偏好重载不会把预览盖回去。
- 读屏：四个柄的名称是「键盘高度」「键盘底部高度」「键盘宽度（左边）」「键盘宽度（右边）」，当前值和调法在说明里；三个按钮各有说明。调整期间遮罩下的工具条和按键整块不进读屏（`keyboard()` 的列设 `accessibilityLevel('no-hide-descendants')`），读屏只走遮罩上的柄和按钮，与 Android 的 `IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS`、iOS 的 `accessibilityViewIsModal` 相同。HarmonyOS 到 API 24 为止 `AccessibilityAction` 只有点击，自定义组件做不了「增减」，读屏用户调这四项只能靠拖动（屏幕朗读下的拖动手势没有在设备上验证）。
- 测试：`tests/keyboard-adjust.test.ts` 覆盖范围、生效条件和柄、外框换算、各边拖动约束（对边不动、下沿过下限后整块上移、到限停住）、存放与不进共享结构、面板高度含抬高、磁贴改名；`tests/run.sh` 钉住离开即保存、只存生效且动过的项、调整期间键区不进读屏、重载保留预览、根节点内边距、面板高度和冷启动第一帧的接线，以及 `HeightBar` 不再存在。

## 全拼九键的三栏展开面板

候选条末尾的 ∨ 此前在手机和平板上打不开任何东西：surface 改成单值（#2503）之后，它只取了完整候选列表，却没有切到展开面板。现在它打开面板，再点一次收起；面板开着时每次 `onChanged` 都重取候选，不再是打开那一刻的一份拷贝。

网格分批建格子（`ExpandedCandidateBatchPolicy`，与 Android 的 `CandidateGridBatchPolicy` 规则相同）：一个字母常有几百个候选，一次全建成 ArkUI 格子会让打开面板和面板里每次换候选都明显卡顿。打开时先建 48 格，滚到离已建内容底边不到一屏、或第一批没填够一屏多时再追加一批。重取到同一代候选（`KeyboardSession.candidateListIdentity` 相同，比如释义晚到）建回已建的数量；换了一代从第一批建起并滚回顶部。完整候选仍由 `allCandidates()` 一次取回，只有格子是分批建的。

全拼九键组字时面板画成三栏，占满键区（`nineKeyExpandedFace`，判断在 `NineKeyPanelPolicy.threeColumn`）：左栏是引擎给的 `nine_key_spellings`（完整拼音、下一个数字键上的大写字母、未锁定时的数字本身，读屏分别说「选择拼音」「选择字母」「输入数字」），可切成横竖撇点折五个笔画键；中栏是可滚动的全部候选；右栏是返回、⌫、重输、拼音/笔画、全部/单字。选拼音、⌫、改筛选都不关面板；选了只覆盖前面一段的候选时也不关，换上剩余组字的全部候选（#9193，与 Android、iOS 相同，判断在 `CandidatePanelSelectionPolicy`），单字筛选留着、笔画筛选清空；点返回或组字结束（含选词后整句上屏）才收起。普通候选网格在部分选词后同样留着。筛选把候选滤空时面板仍开着，好让用户撤掉那一笔。

筛选走 `msime_client_set_nine_key_filter`（NAPI `setNineKeyFilter`），状态以引擎的 `nine_key_single_character`、`nine_key_strokes` 为准，界面不另存。笔画模式下 ⌫ 先撤一笔，没有笔画才是普通退格；切回拼音或关闭面板时清掉筛选。写进输入框的是 `nine_key_reading`（`ning'bai`），不是按下的数字，引擎算不出读音时才退回 `preedit`；编辑框不接受预上屏文字时读音行照旧画它（见下文「拼写写进输入框，候选栏不画读音」）。其他布局保持原来的整块面板。

## 全拼 14 键

触屏方案 `QUANPIN_FOURTEEN_KEY`（preferenceId 和 layout 都是 `fourteen_key`，卡片「拼 14」，追加在方案表末尾，默认不启用），把 QWERTY 上相邻两个字母合成一键：`QW ER TY UI OP` / `AS DF GH JK L`（不缩进）/ `[符/分词] ZX CV BN M [⌫]`，四行与 26 键等高，底行是 `123 | ， | 空格 | 。 | 中/英 | 回车`（26 键为 `123 | 符 | 空格 | ， | 中/英 | 回车`，逗号长按输入句号，14 键第三行左端已经有 符，保留带逗号句号的这一行）。键表在 `keyboard/input/FourteenKeyLayout.ts`，与 Android、iOS 和引擎 `KeyGrid::FourteenKey` 是同一张表，`scripts/test-fourteen-key-table.py` 核对。手机和平板提供，2in1 与九键一样不提供：选择器里过滤掉，同步过来的 `fourteen_key` 在 2in1 上画 26 键、不开网格。

点一下经 `pressGridKey`（NAPI `gridKey`，`msime_client_grid_key`）送这一组的首字母；会话的网格是 `keyGrid`（`KeyGrid` 的 0 / 1 / 2，与 C ABI 一致），建会话、获得焦点、词库维护后重建这三处都重发 `setKeyGrid`。组字不写进输入框，读音行显示 `nine_key_reading`；拼音选择条在读音右侧横向滚动（读音最多占半行），不盖住候选，在 123 层和三栏面板开着时照样画；展开候选用九键的三栏面板（`NineKeyPanelPolicy.threeColumn` 放宽到 14 键）。第三排左端键组字时是「分词」（送 `'`），空闲时是「符」（打开符号面板），两种状态都记作 `SoftSymbol`。两个字母的键长按弹出这两个字母（复用 `SURFACE_NINE_KEY_HOLD`），选中后先结束组字再上屏，L、M 不弹。`qwertyLettersShowing` 不含 14 键，所以没有滑行；没有 Shift、按键气泡、双拼提示和角标。英文画 26 键，`selectEnglish` 不改写 `touch_keyboard_layout`，切回中文恢复 14 键；密码框、网址框（`prefersFullFace`）和本地模式画 26 键；123 与 26 键相同，是一行数字。组字中在 123 层点字符、打开符号或表情面板这些走 `commitRaw` 的路径，Engine 对 14 键结束组字取首选，不会把组码原样上屏。

同步：导入端认识 `fourteen_key`，这个键的未知值只跳过、不再整份拒收；本版导出时本机是 `fourteen_key` 就省略这个键，上传按 merge 合到云端快照上，等于保留云端旧值。打字统计的来源是 `fourteenKey`，键位 id 是 `FourteenQW` 到 `FourteenM`（`KeyIdPolicy.fourteenKey`）。逻辑测试在 `tests/keyboard-logic.test.ts` 的 14 键三组。

## 全拼 18 键

触屏方案 `QUANPIN_EIGHTEEN_KEY` 使用 `Q WE RT Y U IO P` / `A SD FG H JK L` / `Z XC V BN M` 三排组码键，编号为共享 C ABI 的 `KeyGrid::EighteenKey`（4）。它与 14 键共用候选、读音选择、拼音条、三栏展开面板和长按字母行为；键表与引擎分组、`Eighteen*` 统计键位由 `scripts/test-harmony-eighteen-key-table.py` 核对。18 键同样只在手机和平板提供，2in1、英文、密码框、网址框和本地模式回落到 26 键。

18 键的布局偏好是 `eighteen_key`，默认不启用；鸿蒙新版本可正常同步，旧版遇到未知值时只跳过该键。打字统计来源为 `eighteenKey`，键面第三排左端保留分词/符号键，右端为删除。三排从同一左右边界铺开：第二排六个键平分整行，`A`、`L` 分别与首排 `Q`、`P` 对齐，与 Android、iOS 相同。

## 双拼 14/18 键

双拼沿用当前的 `shuangpin_profile`，键盘布局单独读取共享偏好 `touch_shuangpin_layout`，可选 `twenty_six_key`、`fourteen_key` 和 `eighteen_key`；设置页切换布局只改这个字段，不会把小鹤、自然码、微软或首道双拼互相替换。14 键使用与全拼相同的组码键表和网格值 `2`，18 键使用双拼专用网格值 `5`，两者都通过 `pressGridKey` 送组码。双拼键面把分号并入 L 键，显示为 `L;`，以便微软双拼输入 `ing`；其他键位、长按和候选交互与全拼组码键面一致。

双拼 18 键两个字母的键可以左右横滑：横向位移超过 24 vp 且大于纵向 1.5 倍时，左滑取左字母、右滑取右字母，以大写精确标记送入 `pressGridKey`，拖动时键面显示选中的字母；回到起点、两指或系统取消不输入，长按先生效时不横滑。规则与 Android、iOS 相同，判定在 `input/EighteenKeySwipePolicy.ts`。全拼 18 键和 14 键只点按。

双拼 14/18 键只在手机和平板显示，2in1、英文、密码框、网址框和本地模式回落到 26 键；布局偏好在回到中文或切换双拼方案后保留。统计仍按所选双拼方案记录，不计入全拼 14/18 键来源。

## 五笔 14 键

五笔的键盘布局单独读取共享偏好 `touch_wubi_layout`，可选 `twenty_six_key`（缺省，不写进文档）和 `fourteen_key`；设置页「屏幕键盘 › 布局」在当前方案是五笔时把「中文键盘」换成「五笔键盘」，切换只改这个字段，不改 `scheme`、`wubi_profile` 和启用列表，86 / 98 版共用这一个选择。键面与全拼 14 键相同（`QW ER TY UI OP` / `AS DF GH JK L` / `符 ZX CV BN M ⌫`），网格值同为 `2`：宿主在建会话、获得焦点和重建时按偏好自己调用 `setKeyGrid`（host-api 只在 Android 和 iOS 上自动打开它，`KeyboardScheme.keyGrid` 的 `wubiLayout` 参数），会话对五笔只放行 14 键网格（`KeyboardScheme.acceptedKeyGrid`）。点一下经 `pressGridKey` 送这一组的首字母，引擎按组展开五笔码表，最多四码。

读音行显示 `nine_key_reading`，即首选的编码前几位（`QW QW` 首选是「你」时为 `wq`），与全拼 14 键一样留在候选栏上、不写进输入框；`editing_text` 是每组的代表字母，不显示。没有拼音选择条，展开候选是五笔自己的展开，不用九键三栏面板。五笔不切音节，第三排左端键一直是「符」（`KeyboardScheme.fourteenKeySeparates`），记作 `SoftSymbol`，不送 `'`；组字中点它照常先结束组字取首选。长按两个字母的键弹出这两个字母，L、M 不弹。

五笔 14 键只在手机和平板显示，2in1、手写、英文、密码框、网址框和本地模式回落到 26 键；实体键盘的字母照常走 26 键五笔。打字统计来源仍是 `wubi`，键位 id 是 `Fourteen*`。

## 拼写写进输入框，候选栏不画读音

读音画在候选上方时，每打一个键读音变长一截；把读音放到候选左边又会推着候选左右移动，另起一行读音则让顶栏太高。现在手机与 Apple 键盘、Gboard 一样：组字总是以拼音分词形式作为预上屏文字（`InputClient.setPreviewTextSync`）写进输入框，候选栏不画读音，候选总从行首开始，不会随拼写左右移动。没有开关：手机不看 `tsf_preedit_style`（「行内预编辑」），也不看 `candidate_preedit_style`（「候选栏预编辑」），这两项只对 2in1 的候选窗口和行内拼写生效。

写进去的是拼音分词形式（`SchemeCompositionPolicy.fieldSpelling`）：全拼 26 键和双拼是 Engine 的 `preedit`（`ni'hao`），全拼九键是 `nine_key_reading`，绝不是按下的数字，笔画是笔画字形。候选栏只在拼写真的写进了输入框时才不画读音（`InlinePreeditPolicy.hidesStripReading`）；读音那一行的高度一直保留，所以候选不会上下移动。以下情况照旧把读音画在候选栏上，不写进输入框：

- 全拼 14 键和 18 键、五笔 14 键：组字是成组的组码，写进输入框对用户没有意义（`SchemeCompositionPolicy.keepsReadingOnStrip`）。
- 日语的假名读音。
- 编辑框的属性说它不接受预上屏文字（`isTextPreviewSupported`），或写入失败：拼写不能哪里都看不到。

短语里已经选定的那一段和正在运行的本地模式的名字仍留在候选栏上。韩语音节、注音转换、越南语单词这类本身就是文字的组字保持原样：写进输入框，候选栏也照旧画。逻辑测试在 `tests/keyboard-logic.test.ts` 的「a phone always writes the composition into the field」和「nine-key writes its pinyin reading into the field」两组。

## 键盘里调的高度被设置页的值盖回去

`changePreferences` 存好文档、记下版本号，却不更新缓存的偏好，`preferences()` 一直返回准备会话时的那份。视图存完立刻读回旧高度：拖动弹回原位，± 不累加，拖间距时把旧高度一起写回，面板尺寸也按旧值算；下次聚焦时版本号已经一致，不会重建，旧值就留到进程重启。现在存完按存下的整份文档重算外观（`refreshPreparedPreferences`），`selectGlobalTheme` 原来自己补的那一步也并进去了。设置页的改动仍按版本号在下次聚焦时重建会话。

## 数字层与「跟随系统」振动

数字层的排列是固定的：九键的数字层按电话拨号盘排，1 2 3 在上（`NineKeyLayout.digits()`）；触屏 26 键和 14 键按 123 出的是一行 1–0 的 123 / #+= 双层。共享偏好 `touch_number_keypad_order` 和 `touch_twenty_six_key_number_layout` 仍随账号同步（`platform.harmony.number_keypad_order`、`platform.harmony.twenty_six_key_number_layout`），但设置页不再列出它们，键盘也不读取。

共享偏好 `touch_shuangpin_key_hints` 为 `false` 时（设置页「屏幕键盘 › 布局」的「双拼键位提示」），26 键双拼的字母键不再画底部的声母/韵母提示，提示行不占高度；缺省和旧文档都按开。账号同步键是 `platform.harmony.shuangpin_key_hints`。

共享偏好 `touch_quanpin_shift`（设置页「屏幕键盘 › 布局」的「组字时保留 Shift」）默认开启，全拼 26 键组字时第三排左端保留 ⇧，点一下臂辅助码（Shift+字母），不再换成「分词」键；明确写 `false` 后改为分词，九键、14 键和 18 键自带的分词键不受它影响。键盘在 `KeyboardSession` 准备偏好时按 `!== false` 读它（缺键、类型错都按开），`KeyboardView` 的 `usesQuanpinSeparator()` 和键面 `faceKey()` 都按它分支，开关变化即重画。账号同步键是 `platform.harmony.quanpin_shift`。

全拼或双拼组字时，Shift 加字母以大写字母交给 Engine（`ChineseHelpcodePolicy.letterInput`）：Engine 只把组字中的大写字母当辅助码，不看 Shift 标志。双拼「末尾间接触发（大写字母）」和「句中触发键（大写字母）」也走这条路。分号、反引号、`/` 和「辅助码触发键」这些间接触发键在符号层里按：它们此刻会开一段辅助码时，Engine 把它们列进 `spelling_symbols`，2in1 符号行的 `tapSymbol` 按 `HardwareKeyRouter.touchSpells` 当字符送进去，外接键盘同样如此；触屏 123 层（画成「；」的分号键）和数字九宫格经 `typeHelpcodeTrigger`（`ChineseHelpcodePolicy.layerTrigger`）先交给 Engine，收下就回到字母层接着打辅助码字母，不收才照常上屏键面。全拼和双拼的「辅助码触发键」在设置页「输入 › 辅助码 › 更多选项」，宿主能力 `helpcode_trigger_key` 在 HarmonyOS 和 Linux 上打开。

振动三档以前只差时长（10/20/35 ms）、强度固定，摸不出差别。现在每档用一个预置效果加拉开的强度（`KeyboardFeedback.plan`：轻 `haptic.effect.soft` 35、中 `haptic.effect.sharp` 70、强 `haptic.effect.hard` 100），设备不支持该效果（`isSupportEffectSync`，结果按效果缓存）时退回 8/20/40 ms。新增「跟随系统」（`system`）：用 `usage: 'touch'` 和 `haptic.clock.timer`、不带强度，振不振、多强由系统的触感反馈设置决定。按键反馈文件每次聚焦都重读，设置页改了档位不必等输入法重启。这些强度是按 SDK 6.1.1（API 24）的类型声明写的，还没在真机上逐档摸过。

## 候选词的译文此前只能看，不能用

候选下方那行释义在这台宿主上一直是只读的：macOS 用 Option／Control 加数字把译文交出去，触摸键盘没有修饰键可用，来源因此把它挂在候选的长按上（`TheCandidateMenuOffersToInsertTheGlossItself`）。鸿蒙这边长按候选给的是词条管理——优先显示、固定到第几位、取消固定、删除词条——没有任何一处能把看得见的译文打出去。

来源那边长按**只给译文**，注释里写明词条管理是特意从这个菜单撤掉的（「摊在这里会把长按的主用途埋掉」）。这台宿主的管理菜单来自 Windows／Android 那条线，是它自己的正当来源，所以没有照搬撤掉，而是把译文放进同一个菜单、排在第一位：有译文的候选，长按的理由就是它；没有译文的，理由才是管理。

两处因此与来源不同，都是这个菜单的形状带来的：

- **标题带动词**。来源的标题就是译文本身，因为它的菜单里没有别的；这里译文若只写一个名词，夹在一排动词中间读不出是可以点的，所以写成 `输入“…”`。
- **标题会截断**。释义是词典里的一行，可以很长，而菜单在手机上是一行；超出就截到二十四个字符加省略号。读屏用的那句播报保留全文——那是念出来的，不受排版限制。

顺序与 macOS 一致：**先上屏译文，再作废组字**。反过来会让编辑器先看到一个空组合、把视图滚离光标，然后文字才到。

菜单开着时候选栏可能被重建，那时这个下标就是别人的候选了。来源用一个 revision 挡这一刻；这里比对动作里带的标题与候选当下的译文算出来的标题是否一致，问的是同一个问题，而不必再维护一个计数器。

`INSERT_GLOSS` 的菜单 id 与管理项不冲突，这一条有断言钉着。

## 输入法在系统列表里没有图标

要把 logo 换成 Windows 那枚母版，先查了一遍各平台现状：`platforms/windows` 的三个 `.ico` 与母版**逐字节相同**，`packages/ui/src/assets/msime.svg` 与 Windows 那份源 svg 也逐字节相同，macOS 的 `.icns`、鸿蒙的 `app_icon.png`、桌面 Tauri 整套并排看下来是同一枚标同一种构图；Android 与 iOS 在 #3434 已经换过，它们那套细边框满幅是自适应图标要的形状，五种边框颜色是「可切换应用图标」这个功能本身。也就是说美术早就统一了。

真正缺的不是图片，是清单里的声明。`module.json5` 的两个元素都只写了 `label` 和 `startWindowIcon`，**没有 `icon`**：

- `startWindowIcon` 只管启动过渡窗，不是桌面图标，也不是输入法列表里那枚。
- `mainElement` 是 `KeyboardExtensionAbility`，而模块 schema 对 `icon` 的说明是「若该扩展被配置为 MainElement，此标签必须配置」。它缺着，所以这个输入法在「系统设置 → 输入法」里只有名字、没有标。
- `EntryAbility` 带 `entity.system.home`，是桌面入口，同样没声明。

顺带修掉一处从 `bdb801b63`（加设置应用那次）起就存在的 `"abilities"` 整段重复：JSON 后者覆盖前者，所以前一块是一段长得和生效配置一模一样的死文本，改错地方会毫无反应也毫无线索。

验证看的是打出来的包而不是源文件——`$media:` 解析不到时会被丢掉而不是报错。解包 `module.json` 后两个元素都带着 `"iconId": 16777217`，说明引用真的解析到了资源。`scripts/test-harmony-manifest.py` 现在查这三件事：模块级重复键、mainElement 有没有 icon、桌面入口有没有 icon，外加包里 `icon` 有没有对应的 `iconId`。对着改之前的清单跑，三条全报。后来又加了一条：两份清单里配置的每个图标都不能含透明像素（分层图标看背景层），原因见下一段。

桌面图标用分层图标 `$media:app_icon_layered`（`AppScope/resources/base/media/app_icon_layered.json`）：背景层 `app_icon_background.png` 是 1024×1024 的 `#A8DF8E`，即标志的绿边色；前景层 `app_icon_foreground.png` 是标志 `packages/ui/src/assets/msime.svg` 撑满画布、透明底，所以桌面的圆角遮罩切出来就是标志本身，外面不再套任何底板，与其他平台一致。只有 `app.json5` 和桌面入口 `EntryAbility` 用它；`app_icon.png` 本身是透明底，直接给桌面时圆角方块外透出壁纸，看起来没有底板，而键盘里的品牌键、输入模式提示和启动窗仍按普通图片用它。

输入法扩展 `KeyboardExtensionAbility` 的 `icon`（系统输入法列表和切换器里那枚）用 `app_icon_opaque.png`：分层图标的背景层和前景层压成的一张 1024×1024 不透明图。AppGallery 上架审核要求提审页上传的图标和包内配置的图标都不含透明像素，以前这里直接用透明底的 `app_icon.png`，审核因此退回过。重新生成：在 `AppScope/resources/base/media` 下运行 `magick app_icon_background.png app_icon_foreground.png -compose over -composite -alpha off -strip PNG24:app_icon_opaque.png`。AGC 提审页上传的图标也用这张。

## 日语数字层的第十二格：从死键变成括号键

假名格的第十二格是 `小゛゜`（把刚输入的假名变成小写／浊音／半浊音）。切到数字层之后键不再产生假名，这个后置修饰符就没有东西可修饰——`JapaneseVariantPolicy.enabled` 里那个 `!symbols` 让它在整个数字层恒为停用，于是那一格是死的。来源把这个空出来的格子给了括号：`setDigits(true)` 时键面变 `（）`、读作 `括弧`、始终可用，按下给出八个括号。这些括号在这套布局上没有别的落点。

`JapaneseNineKeyLayout.digitBrackets()` 早就移植好、也有测试，只是没有调用方。现在接上了：`JapaneseVariantPolicy.brackets(japaneseNineKey, symbols)` 决定这一格当下是哪一种键，两种职责互不重叠（数字层上 `enabled` 恒 false，假名层上 `brackets` 恒 false），所以一个键可以兼任。

括号走的是条状浮层而不是菜单——`bindContextMenu` 在输入法面板里不显示，这台宿主上每一处长按列表都是这么画的，快捷标点和候选词管理共用同一条。三者互斥，打开一个会关掉另外两个。

## 在设置页关掉当前方案，键盘要能离开它

共享设置页管着「哪些输入方案出现在选择器里」。关掉键盘正在用的那一个，它从选择器里消失，而键盘照旧用着它——于是既看不见也退不出，除非随便挑另一个。来源在写入启用列表的那一刻就把当前方案归一（`DisabledSchemesAreHiddenAndCurrentSchemeFallsBack`：应用中的日语遇上只启用 `[全拼9键, 五笔]`，存下来的方案立即变成全拼9键；清空则回落到全拼）。

这里不能在写入时做，页面和键盘是两个进程，页面写的是文档而不是一个正在运行的键盘。所以改在 attach 时读：`KeyboardScheme.resolveEnabledSelection` 按「共享选择 → 已应用 → 第一个启用项」定出该用哪一个，`mappingForRuntimeSelection` 决定这算不算一次移动（原地不动时返回 null），移动了才把 `scheme`／`last_chinese_scheme`／`shuangpin_profile`／`touch_keyboard_layout` 一起写回并推给 Engine。两个都是早就移植好、此前没有调用方的。

`touch_keyboard_schemes` 整体进了设置记录：`scheme` 和 `touch_keyboard_layout` 是 Engine 的视角，它们解析出什么与选择器还提不提供它无关。

高情商回复是工具而不是输入方案（目前随 AI 生成类功能一起关闭，功能面板里没有这一格，见上文）：`KeyboardScheme.SCHEMES` 和方案选择器里没有它，功能面板里的「高情商回复」图块（读屏名 `reply`）在任何方案下都在，点一下打开回复面板（`SURFACE_REPLY`），点面板里的「完成」回到原来的键盘。旧文档里存的 `thoughtful_reply` 不是一个已知的方案 id，按未知 id 处理：`enabledFromPreferenceIds` 把它从启用列表里去掉（只剩它时回落到全拼 26 键），`resolveEnabledSelection` 把存着它的选择落到第一个启用项。插入的回复按 `reply` 来源记入打字统计，与当前方案无关。

单测覆盖了来源断言的那两种情形（已应用项不在启用列表、启用列表被清空）和写回的三种判定。

## 快捷栏的按钮此前对读屏全是哑的

候选行下面那条工具栏原本整条是图标，没有一个挂了 `accessibilityText`。现在工具栏上每个控件都有名字，从左到右：品牌键 `更多快捷设置`（`tools`，打开功能面板）、`表情与符号`（`emoji`）、`常用语`（`phrase`）、`剪贴板`（`clipboard`）、`皮肤`（`skin`）、`选择输入方案`（`scheme`，打开输入方式选择器）、`收起键盘`（`collapse`，与 `dismiss` 同名）。候选栏的展开箭头和展开网格里的返回读 `expandCandidates`（「展开候选」／「收起候选」），底排的句号键读 `period`，选择器里正在使用的一项在名字后再读 `current`（「当前使用」）。功能面板的图块名字和状态来自 `FunctionPanelPolicy`：每格读 `accessibilityLabel`（多数就是标签，「繁体输出」「按键振动」「打开设置」「语音输入」「生成高情商回复」这几格另有更明确的说法），开关类图块再读已开启或已关闭，暂不可用的读「不可用」。品牌键 `BrandTile` 的读屏名参数是必填的——按键至少还画着一个字符，它只画一个标志。

日语的 `小゛゜` 键同时补上：它靠变淡表示"还没有假名可改"，而变淡这件事读屏不会转述，所以停用态的名字直接把原因说出来（`JapaneseVariantPolicy.accessibilityLabel`，同样是移植好没人调用的）。

固定候选位次在进 Engine 前也补了一道 `CandidateManagementAction.validatePosition`。今天只有菜单会走到这里、而菜单只生成 1–5，所以它对谁都不会抛；放在这里是因为这是位次变成 Engine 参数之前的最后一点，而会话方法在宿主里是谁都能调的。

## 自动大写：两份移植好的策略终于有了调用方

`EnglishCapitalizationPolicy`（句首、词首、全大写三条规则，连撇号和右引号这类不打断单词的字符都照 Java 原样处理）和 `EditorPolicy.capitalizationMode`（把平台给的模式与字段类型调和，地址框一律不大写）都是早就移植好、单测齐全、**产品侧从没有人调用**的。也就是说英文键面上的大写此前只能靠手按 Shift。

接线的三段：`EditorAttribute.capitalizeMode` 读进来（`@ohos.inputMethodEngine` 的 `CapitalizeMode` 自 API 20 起提供，本工程 `compatibleSdkVersion` 是 21，因此不需要版本兜底；属性本身是可选的，拿不到就是 `none`）；`KeyboardSession.shouldShiftNextLetter()` 用 `getForwardSync` 取光标前最多 64 个字符交给策略——组合进行中不问，否则大写的是拼音而不是结果；`KeyboardView.applyAutomaticCase()` 在 attach 和每次文本变化后应用。

**分清「自动的」和「用户按的」是这一片的关键，两个方向都会出错。** `EnglishLetterCaseState.isAutomatic()` 正是为此存在，同样此前无人调用。一头：`tapLetter` 原本在按键后无条件清掉 SHIFTED，若自动大写设的 Shift 也被清，全大写字段就只会给出第一个大写字母——所以现在只清用户按的那一种，自动的那种交给随后的重算。另一头：自动规则若无差别地跑，用户按下 Shift 之后、字母之前只要来一次候选刷新就会把它抹掉——所以 `applyAutomaticCase` 遇到非自动的 SHIFTED 直接返回。编辑器要求 `none` 时整条路径不动手，因为「不要大写」是在说自动规则，没在说用户刚按的那个 Shift。

模拟器上能打开的编辑器——本应用设置页的 WebView 文本框、浏览器地址栏、浏览器搜索框——`attached to editor` 日志里一律是 `capitalize=none`，映射后的值能从日志读出，说明属性确实读到；要在设备上看到大写真正发生，需要一个会声明 `SENTENCES`／`WORDS` 的编辑器，策略本身的三个分支由逻辑套件覆盖。

## 每一块键面都以同一行动作键结尾

`123`／`返回`、`，`、空格、中/英、回车这一行本来写在 26 键那三排字母后面，是那个 `else` 分支的一部分。于是画自己格子的三块键面——全拼九宫格、日语假名格、手写——全都没有它：没有空格键、回车无法上屏、退不出英文、也够不到数字。九宫格上唯一的出路是去点候选，而空格键上那套 `SpaceCursorMovement` 滑动移光标的逻辑在那里根本不存在。`spaceKey()` 里写着 `this.japanese ? JapaneseNineKeyActions.spaceTitle(...)` 的那个分支尤其说明问题：代码明明为日语准备了 `空白`／`改行` 两个字面，而它的唯一调用点在 `this.japanese` 为真时到不了。

现在这一行是 `actionRow()`，三块键面各自在末尾调用它，键面本身占 `gridHeight()`（`surfaceHeight()` 减去动作行和它上面那道缝），所以换布局不改变键盘总高——这正是 Apple 那几条 `KeyLayoutsKeepNineKeyHeight`、`EveryKanaKeyConvertsAndLayoutsKeepFullHeight` 断言的东西。手写的画布随之改成按 `gridHeight()` 取比例，否则工具行和候选会被挤出底边。

**数字层保持三列。** 此前九宫格按 `123` 会掉进 26 键那排十列符号（`this.nineKey && !this.symbols` 的判断把数字层让给了 `else` 分支）。选了三列的人应当继续得到三列：`NineKeyLayout.digits()` 把同一个格子重贴成 1–9，右列的 0 照旧，侧栏换成 `@ # / -`；日语侧直接用上了早就写好却从没有人调用的 `JapaneseNineKeyLayout.digitKeys()`。数字走的是 26 键数字排同一条 `punctuation()` 插入路径，不交给 Engine，否则会被当成候选位次；日语那边 `sendKana` 对空 stroke 本来就是直接上屏，正合数字层要的“不走罗马字转换”。

两处 ForEach 身份也跟着修了。`faceKey()` 现在带上 `symbols`：格子在两层之间保留同样的单元格，2–9 在字母层和数字层送出的是同一个字符，不带这一位的话 ForEach 判定子树没变，数字层会继续写着 ABC、DEF、GHI。日语最后一行原本是两次直接 `kanaKey(...)` 调用，`$$` 是按引用绑定，换一个对象并不重建单元格——实测数字层上面九格已经是数字、这两格还是 わ 和 、，所以它们也改走带 `faceKey()` 身份的 ForEach。

模拟器实测（全拼 9 键→日语 9 键）：九宫格底行出现 `123 ， 空格 中 下一项`；按 `123` 后格子变 1–9、右列 0、侧栏 `@ # / -`、切换键变 `返回`，点 5 直接上屏，`返回` 回到 分词/ABC/DEF；打两下 ABC 出候选 把，回车键从 `下一项` 变 `选定`，按下后 `5把` 落进输入框。日语面底行的空格读 `空白`、回车读 `改行`（`JapaneseNineKeyActions` 的这两个字面此前不可达），变体键 小゛゜ 在没有假名时置灰；其数字层同样是三列 1–9 加 0。

## 地址栏与密码框拿到的是整块字母键面

九宫格靠把一串数字拿去跟词典比对来拼字，而网址、邮箱地址、密码和一次性验证码都是词典里没有的任意文本，在那里每一次点击必须是一个确定的字符。所以这几类编辑器一律给 26 键，不管用户把布局设成了什么。这条来自 MSIME-Apple 的 `testLatinFieldsUseFullKeyboardAndRestoreNineKeyHeight`。

判断写在 `EditorPolicy.prefersFullFace`，选的编辑器集合与 `prefersLatin` 相同但另起一个名字，因为问的不是同一件事——一个问用哪种语言，一个问用哪块键面。键面在每次 attach 时从方案重新算，不记住：这正是离开地址框以后网格自己回来的原因，覆盖属于编辑器而从不写回布局偏好。用户在同一个框里手动换布局则压过它，直到下一次 attach——那是指名道姓要的键面。

模拟器上按 `attached to editor` 的 `pattern` 实测过一个来回（方案设为全拼 9 键）：共享设置页 AI 那一屏的「模型」是 `pattern=0`，键面是九宫格、方案标签「全拼 9 键」；紧挨着的「接口地址」声明为 `type="url"`，WebView 把它报成 `pattern=6`（`PATTERN_URI`），键面立刻变成小写 26 键、标签「英文 26 键」、空格键写 `space`；点回「模型」，`pattern=0`，九宫格和「全拼 9 键」都回来了。小写是对的：`capitalizationMode` 对 URI 返回 `NONE`，与 Apple 那条断言里 `q` 而非 `Q` 的判断一致。

## 韩语 Dubeolsik（두벌식）

韩语是输入法里与日语并列的又一个方案，不是单独的系统语言：选择器末尾多一张「韩语 26 键」卡（`KeyboardScheme.KOREAN`，偏好 id 与 Engine 方案名都是 `korean`，Engine 编号 4），选中时和日语一样把被替换的中文方案记进 `last_chinese_scheme`。账号同步的 `input.schema` 接受 `korean`，打字统计记在 `korean` 名下。唯一的候选是组字音节的汉字（한자）列表，见下文；没有云候选、学习和繁体转换。

26 键面换成 `DubeolsikLayout` 的字母键：键帽画的是该键对应的韩文字母（ㅂㅈㄷㄱㅅ…），按住 Shift 时 Q W E R T O P 换成 ㅃㅉㄸㄲㅆㅒㅖ；点击发出的仍是 ASCII 字母，Shift 下发大写，Engine 按大小写区分 ㄱ 与 ㄲ。第二排去掉了 `;` 键，符号键面、`,` 键和快捷标点菜单都显示并输出半角 ASCII，语言键写「한」。

组字中的音节在编辑器里以预览文本内联显示（手机上也是，不看 `tsf_preedit_style`），候选条画的是音节本身而不是 `editing_text` 里的按键字母。空格、数字先提交音节再由键盘自己打出这个键；回车先同步提交音节再执行编辑器动作；标点由 Engine 与音节一起提交；切换方案、切到英文、失去焦点都提交而不是丢弃音节。

硬件键盘走 `HardwareKeyRouter.routeKorean`：字母总是组字，大小写只看 Shift、不看 Caps Lock；空闲时其余按键全部交还应用；组字时退格删一个字母、Esc 丢弃，回车、方向键、Home/End、Delete、Tab、翻页键先同步提交音节再交还应用（`COMMIT_THEN_RELEASE`）。以上只由 `tests/run.sh` 的逻辑测试覆盖，尚未在设备上验证。

### 汉字（한자）转换

只转换当前正在组字的那一个音节（已上屏的音节不转换），汉字表由 Engine 内置（取自 libhangul，BSD-3-Clause 声明随 HAP 放在 `resfile/licenses/`）。宿主发 `MSIME_CONVERT_HANJA`（`InputCommand.CONVERT_HANJA = 16`）打开列表，再发一次关闭；判断“列表开着”的依据是 Korean 规则成立且视图带候选（`KoreanCompositionPolicy.hanjaListOpen`），因为 Korean 在这条命令之前没有任何候选。

显示：候选正文只有汉字本身。Engine 把训音（훈음，如 韓 的「나라 이름 한」）放在候选的 annotation 里，宿主把它取到 `CandidateEntry.hunEum`（`CandidateGlossPolicy.hunEum`），画在候选下方的释义行上，与「随包英文释义」「候选翻译」两个开关无关：Korean 方案下 `glossRows()` 至少为 1（`CandidateGlossLayoutPolicy.schemeRows`），按方案而不是按列表是否打开预留，所以打开列表不会改变键盘高度；切换进出 Korean 时 `useScheme` 按 `译` 开关同样的方式通知视图和 ability 重算高度。共享翻译查询现在也为 Korean 的汉字行取释义，开关打开且有结果时译文接在训音后面，同一行写成「훈음 · 译文」，单行放不下就截尾；没有训音的行（约四分之三）只显示译文或留空。训音不进共享的 annotation 槽位，所以长按「插入释义」只会打出译文，训音永远不会上屏；读屏念作「训音：…」。展开候选面板同样在汉字下方显示「훈음 · 译文」。2in1 竖排候选窗没有释义行，训音以小号字画在汉字旁边。

- 触屏：组字时候选条组字行末尾出现「漢」按钮（2in1 的候选窗不画，那里用硬件键），点一下列出汉字，列表开着时底色填充，再点关闭；单个字母没有汉字，Engine 不处理，组字行提示「单个字母没有对应的汉字」。列表开着时空格和回车选高亮的汉字（回车键面已是「确认」），点候选直接上屏，长按退格清空组字时连发两次取消，否则第一次只关掉列表。
- 硬件键盘：组字时韩文键盘的汉字键（`KEYCODE_HANJA` = 2614，即 `Lang2`）或不带修饰键的 F9 触发，按住只触发一次；无论 Engine 是否处理都吞掉这个键，空闲时交还应用。列表开着时 `routeKorean` 先让给 `routeHanjaList`：空格、回车、小键盘回车选高亮项，1–9 选本页（共享偏好 `number_row_selection` 关闭时仍是提交韩文再打数字），翻页键和 Tab 总是翻页（这两组固定开启，`HostCapabilities.tab_page_paging_switches` 为 false），上下键按导航偏好翻页和移动高亮，左右键在方向键导航打开时移动高亮；关掉的方向键绑定和 Home/End 保持 Korean 原有含义。`- = [ ] , .` 仍是标点，由 Engine 关掉列表并把韩文和标点一起提交；退格、Esc 只关列表、保留音节；字母关掉列表后照常组字；切换方案、失焦和编辑器自己的改动提交的是韩文。

列表没打开时，所有按键与点按的行为与上文完全一致。以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备上验证。

## 粤语、注音与越南语

选择器末尾在「韩语 26 键」之后再加三张卡：「粤语」（`KeyboardScheme.CANTONESE`，Engine 方案名 `cantonese`，编号 5）、「注音」（`ZHUYIN`，`zhuyin`，编号 6）和「越南语」（`VIETNAMESE`，`vietnamese`，编号 7）。三者默认都不启用（`DEFAULT_ENABLED` 不含它们），由用户在设置页打开。粤语和注音是中文方案：选中时它们自己就是 `last_chinese_scheme`，「中文」回到它们，打字统计记在 `cantonese`、`zhuyin` 名下；越南语和日语、韩语一样不是中文方案，选中时保留原来的 `last_chinese_scheme`，统计记在 `vietnamese` 名下。三者都不学进主词库，所以候选长按没有置顶、降权、删除；简繁转换开关对它们不起作用（粤语与注音本来就是繁体）。快捷栏和语言键上粤语、注音显示「中」，越南语显示「越」。

方案之间的差异不再逐处写方案名，而是集中在 `SchemeTraits.ts`：它按 Engine 的方案编号逐条镜像 `crates/engine/src/types.rs` 里 `SchemeType` 的谓词（`is_chinese`、`uses_chinese_punctuation`、`commits_on_blur`、`locks_caret`、`has_openable_candidate_list` 等），每个谓词写成“对哪些编号成立”的列表，`scripts/test-scheme-traits-parity.py` 读这个文件，常量编号、`NAMES` 或任一谓词与 Engine 不一致时失败。`input/SchemeCompositionPolicy.ts` 是原 Korean 专用组字策略的推广：韩语、注音、越南语的组字行画 Engine 的 `preedit`（写出来的字，而不是按键），光标固定在末尾；韩语和注音的候选只在用户打开的列表里出现，是否打开以视图的 `candidate_list_open` 为准。英文模式和本地工具模式下这些规则都不生效。

### 词库与暂存

粤语、注音和笔画各需一份语言词库（`msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db`，由 `scripts/fetch_language_dictionaries.py` 取回或 `msime-dict-build languages` 生成），越南语不需要。`stage-resources.sh` 的第三个参数（默认 `target/language-dictionaries`）指向这些文件，每份词库只在其许可证文本（`msime-rime_cantonese_LICENSE.txt`、`msime-libchewing_data_LICENSE.txt`、`msime-rime_stroke_LICENSE.txt`）同在时才暂存到 `resfile/language-dictionaries/`，缺许可证直接失败；一份都没有时只打印提示，设 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1` 则要求 `resources/language-dictionaries.lock.json` 固定的每一份都在（`fetch_language_dictionaries.py --list-databases`）；锁固定 `msime-stroke.db` 之前，笔画词库存在就暂存，缺少也不让发版失败。键盘与设置页启动时用 `StagedResources.stageLanguageDictionaries` 把它们复制到 `files/language-dictionaries/`，与 `files/engine` 相邻，host-api 在那里找到并写进运行时选项；新包不带词库时删掉旧副本。词库缺失的方案不出现：设置页的 `hostCapabilities` 从 `input_schemes` 里去掉它（与桌面端 `drop_uninstalled_language_schemes` 一致），键盘的方案列表经 `KeyboardScheme.withInstalledDictionaries` 过滤，全部被过滤时回落到全拼。

### 注音（大千）

触屏换成 `input/ZhuyinLayout.ts` 描述的大千键面：数字行加 `-`，三排字母各自补上 `; , . /`，共 41 键四行，键帽画注音符号或声调（3 ˇ、4 ˋ、6 ˊ、7 ˙，读屏念作「三声」等），四行高度与其他键面三排字母加行距相同，切换方案时键盘高度不变。点击发出 ASCII 键，由 Engine 的大千编辑器组字，空格是一声。组字行末尾的按钮在注音下写「選」：点一下打开候选列表，再点关闭（读屏为「选字」/「关闭候选列表」）。列表开着时 Engine 把数字 1–9 当选号，而触屏在候选条上选字，所以点数字键会先关掉列表再按键，打出的是键帽上的符号。回车沿用共享规则：列表开着提交高亮行，否则提交整段转换结果。符号键面上大千要占用的键（`1234567890,./;-`）由键盘先提交转换结果再直接打出标点（中文标点开关打开时打中文标点），其余符号照常走标点路线。

硬件键盘走 `HardwareKeyRouter.routeZhuyin`，以 Engine 给出的 `spelling_symbols` 为准：空闲时能起音节的数字（`1 2 5 8 9 0`）直接组字，`, . / ; -` 经标点路线交回 Engine 组字，声调数字等其余键落回中文路线；组字时 Engine 列出的数字与标点都组字；下方向键打开列表；列表开着时交给 `routeHanjaList`，空格、回车选高亮，1–9 选本页，翻页键和 Tab 总是翻页（固定开启，`HostCapabilities.tab_page_paging_switches` 为 false），方向键按导航偏好移动；Shift+数字等标点总是标点；左右键、Home/End、Delete 和 Ctrl+退格/左/右先提交再交还应用（转换结果里没有光标）。

### 越南语（Telex / VNI）

触屏用普通 26 键，字母按实际大小写发出，Shift 只作用一次，不做自动大写；符号、逗号键与快捷标点都是半角 ASCII，第二排没有 `;` 键。组字行画 Engine 写出的带声调词。符号键面上的数字只在 VNI 且正在组字时作为声调键交给 Engine（Engine 此时把 `0123456789` 列为 `spelling_symbols`），否则先提交词再打数字；空格提交词并打出空格；回车先提交词，再照常换行或提交编辑框。失焦与切换方案提交当前词。硬件键盘走 `routeKorean` 的同一条路（不认汉字键）：字母总是组字，空闲时其余键交还应用，组字时 VNI 数字组字，标点连词一起提交，其他键先提交再交还应用。


### 藏文（EWTS 威利转写）

选择器在「越南语 26 键」之后再加一张「藏文 26 键」卡（`KeyboardScheme.TIBETAN`，偏好 id 与 Engine 方案名都是 `tibetan`，编号 8），和越南语一样默认不启用、不需要词库，由用户在设置页打开。藏文不是中文方案：选中时保留原来的 `last_chinese_scheme`，打字统计记在 `tibetan` 名下，不计入中文；不学进主词库，简繁转换、中文标点和全角都不作用于它。账号同步上传时和粤语、注音、越南语一样不写 `input.schema`。快捷栏和语言键显示「藏」。`SchemeTraits.TIBETAN` 的谓词与越南语相同：失焦与切换方案提交组字（`commits_on_blur`）、光标固定在末尾（`locks_caret`）、第一次取消保留组字（`cancel_keeps_composition`）。

输入是在拉丁字母键盘上打 EWTS（扩展威利转写），由 Engine 转成藏文：组字保存当前音节串的威利原文，组字行画 Engine 写出的藏文（`preedit`），没有候选列表。威利转写区分大小写（`T D N Sh A I U M H` 等是不同的字母），所以触屏字母按 Shift 给出的大小写发出，硬件键盘的 Caps Lock 也当作大写，和越南语一样不做自动大写；第二排没有 `;` 键，符号键面、逗号键与快捷标点都是半角 ASCII。触屏空格交给 Engine：组字时上屏藏文加音节点（U+0F0B），没有组字时 Engine 不处理，键盘照常打出空格（`KeyboardSession.pressThenType`）；回车走共享规则，组字时提交藏文（不加音节点）并吞掉回车，没有组字时照常换行或提交编辑框。符号键面上的 `/` `'` `+` `.` `-` 走标点路线（`+` 是藏文方案下第三排替换 `=` 的键，`KeyboardScheme.symbolRowKey`；符号面板直接写入编辑框，不能用来叠写），运行时按 Engine 的 `spelling_symbols`（空闲时 `'/`，组字时 `'+-./`）把它们作为字符交给 Engine：`/` 组字时上屏藏文加垂符（U+0F0D），空闲时单独上屏垂符；其余是拼写符号。数字先提交音节串再打数字本身，不转成藏文数字。

硬件键盘走 `routeKorean` 的同一条路（`tibetan` 为 true，不认汉字键）：字母总是组字；Engine 列出的拼写符号空闲时也组字（`'` 开头 achung 音节，`/` 单独上屏垂符），其余键空闲时交还应用；组字时空格经 `PRESS_THEN_TYPE` 交给 Engine 上屏藏文加音节点，回车发 `MSIME_COMMIT_RAW` 只上屏藏文，两者都不再交给应用；退格删一个原文按键，Esc 第一次退回威利原文、第二次丢弃；其他标点连音节串一起提交，数字、Tab、方向键等先提交再交还应用。
以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备或模拟器上验证。

## 笔画

选择器末尾在「藏文 26 键」之后再加一张卡「笔画」（`KeyboardScheme.STROKE`，Engine 方案名 `stroke`，编号 9，字形「笔」，角标「5」），默认不启用，需要 `msime-stroke.db`（见上文「词库与暂存」），缺词库时和粤语、注音一样不出现。笔画是中文方案：选中时它自己就是 `last_chinese_scheme`，打字统计记在 `stroke` 名下；`SchemeTraits` 里它的谓词逐项照抄粤语（中文标点、智能标点、全角加宽成立，不学进主词库、不做简繁转换、失焦不提交、光标不锁定、没有可开关的候选列表），由 `scripts/test-scheme-traits-parity.py` 对照 Engine 检查。账号同步不上传它（`AccountPreferencePlan` 的 `LOCAL_ONLY_SCHEMES`），云端写来的 `stroke` 保留本机方案。

触屏画 `input/StrokeLayout.ts` 描述的笔画键盘，套用九键的外框：左侧是九键的标点栏，中间两行三列 `一 横`、`丨 竖`、`丿 撇` / `丶 点`、`乛 折`、`＊ 通配`，右侧整列是删除键，底排与拼音九键相同（`符 | 123 | 空格 | 中/英 | 回车`，见上文「九键外框的底行和 1 键」）。卡片的布局仍记作 `twenty_six_key`，键面按方案选（与大千注音同理），所以不会打开九键拼音的数字解码；符号层沿用共用的字母面。点击发出字母 `h s p n z x`，Engine 负责组字与候选；空组合时 Engine 不接通配键，键盘也就不发送它。输入框里的预上屏文字（手机和 2in1）画 Engine 的 `preedit`，也就是笔画字形 一丨丿丶乛＊，不画键入的字母，光标沿用 Engine 的位置（字母与字形一一对应）。空格、退格和失焦沿用粤语的共享规则；回车不同：粤语有候选时回车上屏高亮候选，笔画的触屏回车与硬件回车、iOS、Android 一致，总是上屏键入的字母（`ReturnKeyAction` 的 `COMMIT_RAW`，即 MSIME_COMMIT_RAW），选字用空格。快捷栏的输入模式指示显示「笔」，触屏语言键仍显示「中」。

硬件键盘走中文的共享路线：字母交给 Engine，空组合时只有 `h s p n z` 开始组字，`x` 与其它字母由 Engine 交回应用照常输入（键盘不把这当作故障记日志）；组字中 `x` 追加通配，其它字母被吞掉；数字 1–9 选本页；`'` 不当音节分隔符（`HardwareKeyRouter.spellsWithoutSyllables`，与五笔相同）。

以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备或模拟器上验证。

## 滑行输入

在 26 键字母上一笔滑过要拼的键，抬手后由 Engine 的 `msime_client_glide` 解出最可能的全拼字母并像逐键敲入一样进入组合（与已有字母之间以 `'` 隔开）。宿主只判断一次触摸是不是滑行、记下手指经过的点；解码、纠偏和「停留确认」都在 Engine 里。NAPI 只是转发：`client.glide(handle, request)`，回复与 `character` 同形，`KeyboardSession.glide` 按字母键的回复应用；`handled` 为 false 时整笔丢弃，不改敲它经过的键。

开关是设置页「屏幕键盘 › 手势 › 滑行输入」，默认关。它和按键音、振动一样属于设备而不属于账号，存在键盘自己的 `key-feedback.json`（`FeedbackSettings.glideTyping`），经共享 DTO `MobileKeyboardFeedback.glideTyping` 读写；共享偏好没有这个键，所以不随账号同步，账号同步重写该文件时保留本机的值。只在以下条件同时成立时启用：开关打开、显示的是标准 26 键的字母面（不是韩文、注音、笔画、九键、手写、符号层或盖在键上的面板）、Engine 视图的方案是全拼（`scheme == 0`）、不在英文模式、`local_mode` 为 `none`、当前编辑框经 Engine 组字。

判定与请求在 `input/GlideTypingPolicy.ts`：手指在某个字母键上按下，移到另一个字母键上、且水平位移不少于 0.4 个键宽时才算滑行（同一个键上的竖向滑动不受影响）；滑行开始前第二根手指落下则这次触摸不再判定滑行。三排字母行而不是各个键监听触摸，因为一次触摸始终向按下时所在的行报告；键的矩形来自各字母键的 `onAreaChange`（窗口坐标），手指位置用 `windowX/windowY`，连同 `getHistoricalPoints` 合并的采样一起记下。滑行开始后起始键不再输入（字母键的点击被吞掉，直到下一次按下），其他手指的字母键同样不输入；键盘上方画一条跟随手指的强调色平滑轨迹（`hitTestBehavior(None)` 的 Canvas），抬手或取消即清除。抬手时把 a..z 的键中心、一个字母键的尺寸和全部采样（`[x, y, 毫秒]`，以字母区左上角为原点，超过 1024 个时均匀抽稀并保留首尾）组成请求；取消时什么都不发。整笔只给一次按键振动。

以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备或模拟器上验证。

## 2026-09-21：首次在模拟器上跑起来

在 API 21 的 `Mate 70 Pro` arm64 模拟器（DevEco 自带镜像，`hdc` 连 `127.0.0.1:5555`）上完成了一次装机运行，实测到的东西比之前所有交叉构建加起来都多。

**先是构建根本过不去。** `hvigorw assembleHap` 报 13 个 ArkTS 错误，分布在三个 `.ets` 文件里，全部来自最近合并的几片。`tsc` 全过、单测全绿、设置包构建成功、`verify-local.sh --quick` 通过——没有任何一道门禁编译过 ArkTS，所以 `develop` 处在打不出 HAP 的状态而没人知道。ArkTS 是 TypeScript 的一个严格子集：不认 `unknown` 和 `any`、对象字面量必须对应已声明的类或接口、不支持索引访问类型。`scripts/test-harmony-arkts-subset.py` 现在在 `--quick` 里查前两条（纯语法、零误报）；第三条依赖类型信息——ArkTS 接受 `JSON.stringify({ ok: false, error: x })` 却拒绝里面再嵌一层字面量的同一个调用——纯文本判断要么漏要么误报两百条，两种都试过了，所以那一条明写为只有真编译器能抓。后来又发生了一次：四个 PR 留下 10 个 ArkTS 错误，分布在五个 `.ets` 文件里，同样没有门禁发现。其中 7 个落在三条能从文本读出来的规则上，脚本据此加了三条只认窄写法、对编译器接受的代码零误报的检查：以展开开头的对象字面量（arkts-no-spread）、在 catch 块里原样重新抛出没有类型的 catch 变量（arkts-limited-throw，要写成 `throw error as Error`）、函数参数或返回值上的对象字面量类型（arkts-no-obj-literals-as-types）；另外 3 个是漏掉的 import 和无类型对象字面量，仍然只有真编译器能抓。

装机后确认的：

- 系统识别并接受本输入法：`ime -e app.msime.client` 成功，`ime -s` 之后 `ime -g` 返回 `app.msime.client`，`app.msime.client:inputMethod` 进程被系统输入法框架拉起。
- 共享 React 设置界面在设备上正常渲染：欢迎流程、首页（含底部导航与键盘预览）、词库页。
- 新增 C ABI 的整条链路通了。词库页显示「规格 desktop」「词库版本 e92a9c7c64e2」，与 `resources/desktop-dictionary.lock.json` 的 `source_commit` 前十二位一致——从 `msime_client_dictionary_manifest` 经 NAPI、ArkTS 桥、`registerJavaScriptProxy` 到共享 React 卡片，每一跳都真的走通了。
- 引擎资源暂存正常：`staged /data/storage/el2/base/haps/entry/files/engine`。第一次跑打出 `no packaged resources at /engine` 是因为漏了 `stage-resources.sh`，不是代码问题；补上 180 MB 的已验证词库后即正常。

**键盘作为系统输入法在模拟器上完整跑通了。** 在社区页的搜索框里打 `nihao`，组合行显示 `nihao`，候选栏给出 `1 你好`，点选后 `你好` 进入输入框——按键经 ArkTS、NAPI、`crates/host-api` 的 C ABI、`input-runtime`、`engine-bridge` 一路到 C++ Engine 并带着随包词库返回，整条链路真的走通（这是当时的链路；C++ Engine 与 `engine-bridge` 已由 `crates/engine` 取代，`input-runtime` 现在直接调用它）。回车键读的是编辑器自己的动作：搜索框上显示「前往」，组合进行中变成「选定」，组合结束又变回「前往」。

**这里有一个必须写下来的操作事实：输入法要在 `FULL_EXPERIENCE_MODE` 下才会被框架驱动。** `ime -e <bundle>` 的默认是 `-b`，也就是 `BASIC_MODE`；在那个模式下 `ime -s` 会成功、`ime -g` 会报告本输入法是当前输入法、`app.msime.client:inputMethod` 进程也会起来，但编辑器获得焦点时框架打的是 `ShowKeyboardImplWithoutLock, panel not create` 与 `OnInputStart, entry is nullptr`，而本扩展的 ArkTS 一行日志都没有——`onCreate` 从未运行。表现就是键盘完全不出现，且看不出任何错误。换成 `ime -e <bundle> -f` 之后，同一次点击立刻打出 `attached to editor: pattern=0 enter=2`，面板正常呈现。做过一次对照：同一个输入框、同一次点击，华为系统输入法在我们处于 BASIC_MODE 时照常弹出，所以这不是模拟器、WebView 或该字段的问题。

键盘在第三方应用里同样可用：华为浏览器的搜索框上打 `nihao` 得到候选 `你好`，上屏后浏览器据此拉取了联想词，说明文字确实到达了那个编辑器。回车键在那里读作「搜索」而在本应用的搜索框读作「前往」，两次都取自编辑器自己声明的动作。

那一轮里社区页显示的是离线预览数据，因为当时没有登录账号；账号、社区与 AI 服务的真实往返需要一个已登录的账号会话，麦克风相关路径需要用户授予 `ohos.permission.MICROPHONE`。个人词库队列的 4/4/1 批处理、回执不重放、Engine 规范化、空闲续排和会话状态恢复由逻辑套件覆盖。

按键音与振动现在也能从设置页调整，而不只是键盘内那张卡片：共享 `mobileKeyboardFeedback` 客户端读写键盘自己的 `key-feedback.json`，两个进程共用同一份文件（这项设置属于当前设备而非账号，所以不进共享偏好；API 12 起两个进程的 `files/state` 不是同一个目录，这份文件并不共享，见上文「输入法扩展的独立沙箱」）。设置页是第二个写入者，改动在键盘下次启动时生效。强度预览直接振一下。共享 DTO 把最强一档叫 `strong`，键盘自己的枚举叫 `heavy`，两边由 `KeyboardFeedbackBridge` 转换——直接赋值会写入键盘不认识的值，`KeyboardFeedback.parse` 会静默回退，表现为"保存了但手感没变"。

设置页的字体输入现在能列出系统已装字体：`listFontFamilies` 桥接 ArkUI 的 `font.getSystemFontList()`，宿主只过滤掉带控制字符的名字，去重和排序留给共享页面，以免各宿主给出不同顺序的同一份列表。

共享设置中的候选窗英文字体（`candidate_english_font`）现在也由 Harmony 消费：该名字排在中文字体之前进入 ArkUI 的字体族列表，由渲染器逐字形回退，因此拉丁字母取自英文字体、汉字落到中文字体。未设置时仍是单一字体族，与此前行为相同。该控件此前在共享设置页被一串平台名挡住，Harmony 不在其中——宿主读了这个字段而用户改不到它；现在改由 `HostCapabilities::candidate_english_font` 决定。

共享设置中的录音设备选择现在也由 Harmony 提供：设置页通过 `AudioRoutingManager` 列出输入设备，保存的是设备类型加地址组成的稳定标识，而不是每次会话重新分配的 `id`。HTTP ASR 与豆包两条路径各自创建 `AudioCapturer`，因此在建流前用 `AudioSessionManager.selectMediaInputDevice` 指定所选设备；路由属于音频会话而非单条流，所以每次录音都重新指定一次。所选设备被拔掉、路由被系统拒绝、或保存的 backend 属于别的平台时，都回到系统默认设备继续录音，不中断，也不把 Windows 端点标识或 PulseAudio source 名当成 Harmony 设备重新解释。系统语音识别（Core Speech Kit）由服务自行取音，不受该选择影响。

键盘自己的输入方案选择器现在和共享设置页写同一组字段：选中日语时把被替换的中文方案记进 `last_chinese_scheme`，`scheme`、`shuangpin_profile`、`touch_keyboard_layout` 一起更新。此前只写 `scheme`，于是从键盘切到日语后，设置页的「中文」单选只能退回 quanpin，五笔或双拼用户会看到方案被忘掉。当前值按磁盘上的文档读取，设置页是第二个写入者；随后的写入仍做 revision 比对，真正的冲突照样被拒绝。

共享设置中的“中英文切换提示”现在也由 Harmony 消费，并改由 `input_mode_hud` 宿主能力而非平台名决定是否出现在设置页。2in1 上模式徽标只在该偏好开启且没有悬浮工具栏时创建；关闭后不再占用那一个 STATUS_BAR 面板名额。手机形态本来就在键面上显示模式，不声明该能力。

候选的两项可选注释现在各读各的共享偏好，不再一律显示：`wubi_code_hint` 控制五笔剩余编码提示，共享默认开启，只有文档明确写 `false` 才隐藏；`candidate_english_gloss` 控制离线英文释义，共享默认关闭，只有文档明确写 `true` 才显示。Engine 注释仍优先占用同一个提示槽位。

2in1 的按键音、打字旋律、上屏音和成就音效读共享偏好的 `plugins` 段，与桌面三端同一份设置、同一套音效包，只是播放器不同：host-api 在 HarmonyOS 上不链接音频栈，所以由 `KeySoundPlayer.ets` 用 SoundPool 播放。包里有哪些文件由 NAPI `keySoundPack` 调 `msime_client_key_sound_pack` 取得，校验只有 client-services 一份；哪个事件放哪个文件、旋律怎么走、停顿 3 秒从头开始，是 `KeySoundPolicy.ts` 照 host-api 播放器移植的规则，逻辑测试钉住。WAV 样本由 `native/key_sound_render.cpp` 用 miniaudio（与 Windows 宿主同一份单头文件）解码，先按头部声明的帧数查 1.5 秒上限、解码时再以声明长度为界，然后按旋律用到的每个音高各写一个 48 kHz 的 WAV 到 cacheDir，SoundPool 只解码本宿主写出的文件；变调按播放速率算，与桌面一致，升一个八度的音也短一半。Ogg 样本在 2in1 上不播放：本宿主不解码 Vorbis，交给 SoundPool 就会在媒体服务里整段解码，解码后的长度没有任何东西能限住，所以只有 WAV 样本出声（内置包全是 WAV）。SoundPool 用音乐流类型创建，系统对短音走混音而不打断正在播放的音乐，不走录音那套 `CONCURRENCY_PAUSE_OTHERS`。这套插件设置只在 2in1 上生效，密码框里和英文模式下不出声，与桌面三端一致。手机和平板的触屏键盘也用同一个播放器，但开关是本机 `key-feedback.json` 的 `sound`（出厂关），只播按键音，包和音量取 `plugins.key_sound`，没选包时是内置的「清脆键盘」（`KeySoundPolicy.touchSettings`）；每个软键按下时经 `KeyboardSession.softKeyPress` 出声，密码框里每个键都播普通键的声音，跟随媒体音量。按键音在按键被处理之后触发，包括交还给应用的键；上屏音跟着 Engine 的每次提交；成就音效来自打字统计 `record` 应答里的 `milestone`，所以要打字统计开着才有。设置页在 2in1 上声明 `key_sound` 与 `music` 能力，`plugin_triggers` 每种形态都声明：实体键盘接上任何设备都路由 V、/ 和 @，手机和平板的触屏键盘从功能面板「本地输入」里的「指令」进入 / 模式（设置打开 / 指令且用中文标点时才列出这一项）。

2in1 的背景音乐由 `MusicPlayer.ets` 用 AVPlayer 流式播放：曲目由 NAPI `musicPack` 调 `msime_client_music_pack` 取得（绝对路径与 `max_track_seconds`），校验同样只有 client-services 一份；何时放、按什么顺序是 `MusicPolicy.ts` 照 host-api 播放器 `tick_music` 移植的规则，逻辑测试钉住。音乐开关打开且选了包时，inputStart 之后等编辑框属性回来、确认不是密码框才开始放；inputStop（系统随之收起面板）、密码框获得焦点和录音期间暂停，焦点回到普通编辑框再继续。候选窗每打完一个词就收起一次，那不是输入法被收起，不暂停音乐。每首曲子开播前先用 AVMetadataExtractor 读容器声明的时长，超过 `max_track_seconds` 或读不出来就跳过；播放中再按 `timeUpdate` 的位置查同一个上限，超出就切下一首，与桌面解码器先查声明帧数、播放中再数帧数一致。曲目按清单顺序循环。AVPlayer 边读边解码，所以 Ogg 曲目在这里也能放，不像按键音样本那样只放 WAV。包读不出来、没有一首时长合格、或 AVPlayer 报错，音乐就关掉并在日志里留一行，直到音乐设置变化（换包、开关、音量）才重试；换包或开关会从头重新加载，只改音量直接调到正在放的曲子上。AVPlayer 的 `state` 要等 play/pause 执行完才变，所以同一时间只发一个 play 或 pause，播放器落定到 playing 或 paused 后再按会话此刻的状态对一次，`MusicTransport` 的逻辑测试钉住：快速切换焦点不会让音乐在普通编辑框里卡在暂停，也不会在密码框里响。播放用音乐流类型，是否打断别的应用正在放的音乐由系统的音频焦点策略决定；别的应用拿走焦点后，系统自己暂停的曲子不会马上被重新播放，被系统停掉的播放器直接释放，下次焦点回到可以放音乐的编辑框时重新打开当前曲目。手机形态拿到的永远是关着的设置，不创建任何播放器。

插件页的包管理与 @ 名单走 NAPI `plugins` 调 `msime_client_plugins`：列出已装和内置的包、删除、读写 @ 名单都在设置桥的同步方法 `plugins` 里完成，状态目录和内置音效包目录（`resourceDir/sound-packs`）由桥补上，页面不经手任何路径，规则和失败码与桌面三端的 Tauri 命令是同一份（client-services 的 `PluginFailure`），失败时连同 client-services 给出的具体原因一起显示。导入要等系统选择器，所以走 `startRequest` 的 `plugin_import`：用 `DocumentViewPicker` 选文件夹或 `.zip`，选中的文档 URI 本库打不开，于是先复制到 cacheDir 下的临时目录，再交给 `msime_client_plugins` 按同一套规则安装，临时副本无论成败都删掉。桌面的 client-services 直接读选中的文件夹或压缩包、边复制边按上限停下，这里的临时复制也照同一组上限先查再复制（`PluginImportPolicy.ts`，数值取自 `crates/client-services/src/plugins/import.rs`）：文件夹只看顶层，遇到子文件夹、符号链接、超过 16 个文件、单个文件超过 16 MiB 或合计超过 66 MiB 就不复制直接拒绝，压缩包超过 80 MiB 同样不复制，所以误选了「下载」这类大文件夹不会先整个复制进缓存再被拒绝；真正的规则校验仍只在 client-services。临时副本用固定名字（`pack`、`pack.zip`），不用选中的文件名，包 id 本来就读自 plugin.toml。安装要解压最多 80 MB、校验后换入，所以走 NAPI `pluginsAsync` 在工作线程上跑，不占 UI 线程；列表、删除和 @ 名单这些小读写仍走同步的 `plugins`。包和 `mentions.json` 装进设置应用的 `files/state/plugins`。键盘进程每次聚焦时读的是它自己的 `<stateDirectory>/plugins`，要等两边都改用共享沙箱目录（`getGroupDir`）之后才会是同一个目录；在那之前键盘看不到设置页装的包（见上文「输入法扩展的独立沙箱」）。

手机和平板也有插件页，靠设置桥按形态报告的 `touch_plugin_packs`（`SettingsFormFactorCapabilities.touchPluginPacks`）打开，走同一套 NAPI `plugins` / `pluginsAsync` 和 `plugin_import`，不另写原生代码。页面只列触屏键盘用得上的短语表、指令表、符号集、单词本、`keys` 模式音效包和辅助码表，其余已装的包只在列表末尾说有几个；导入只给「导入 .zip」：`DocumentViewPicker` 的文件夹选择模式按华为文档只在 2in1 上提供，`plugin_import` 在没有 `FolderSelection` 系统能力时拒绝文件夹导入，手机上给这个按钮只会得到一次失败（还没在手机真机上核对过）。音效包只设 `plugins.key_sound.pack`，开关仍是本机 `key-feedback.json` 的 `sound`。`wordbook_packs` 与 `symbol_set_packs` 每种形态都声明，手机键盘的符号和颜文字面板也读镜像过来的 `plugins/` 里的符号集。社区插件经同一个 `community_plugin` 桥浏览、安装和发布，页面只列这六类。

V、`/`、`@` 三个模式的按键由 Engine 导出的 `spelling_symbols` 决定：`HardwareKeyRouter` 在组合中遇到列在其中的字符就交给 Engine 拼写，否则 Shift+1..9 选词，原先只认 `local_mode === "unicode"` 的分支因此泛化到 V 模式的数字和运算符（Shift+9 是 `(` 不是选第九个；`-`、`.` 是运算符和小数点不是翻页；小键盘的点也是小数点）。`/`、`@` 在无组合时照常作为标点交给 runtime，由 runtime 按 `spelling_symbols` 改走 Engine 进入模式。这三个模式生成的上屏内容按 `commit_context.typing_statistics` 不计入打字统计。

## 产品版本

版本表 `shared/contracts/editions.json` 里每个版本的 HarmonyOS 段（`platforms.harmony`）目前都是 `null`：HarmonyOS 只有 `build-profile.json5` 里的 `default` 一个 product，也就是 full，`bundleName` 是 `app.msime.hmos`，本地 Hvigor 工程不保存签名配置（`signingConfigs` 为空），正式发布由 `sign-release.py` 在临时目录中签名。代码已经按版本参数化，full 的行为和产物与引入版本之前相同。

已经就位的部分：

- 版本身份：`entry/src/main/ets/keyboard/AppEdition.ts`，对应 Android 的 `AppEdition.java` 和 iOS 的 `MSIMEAppEdition`。`AppEdition.current()` 目前返回 full。
- 方案：`KeyboardScheme` 的启用列表、默认启用列表和各处回退（词典缺失、列表为空、偏好里认不出的方案）都回退到本版本的默认方案，本版本不提供的入口不算启用；手写不属于任何方案，写出的是汉字，所以只在提供中文方案的版本里有（full、拼音版、五笔版），日文、越南文和藏文版没有，写进偏好、交给 Engine 的是本版本的默认方案（`KeyboardScheme.engineSchemeOf`）；只有一个方案的版本没存过启用列表时，默认要用户自己打开的入口（越南语、藏文）也启用，与 client-core 的 `TouchKeyboardSchemePreferences::for_edition` 一致。`KeyboardSession`、`KeyboardView` 的初始方案同样取版本默认。非 full 版本调 `prepareHost` 时带上版本 id（`KeyboardSession.prepareRequest`），host-api 据此收窄方案、按本版本的资源锁校验词库。
- 设置同步：映射在 Rust 的 `msime_client_account_harmony_settings_export` 和 `_apply`，按偏好目录记下的版本经 client-core 的 `filter_uploaded_account_settings`、`filter_downloaded_account_settings` 过滤：只有一个方案的版本既不上传也不应用 `input.schema`，随它一起的还有本机的 `platform.harmony.keyboard_layout`（方案加布局才决定是哪个入口）；多方案版本把本版本没有的方案当作缺失；不提供双拼、五笔的版本不上传对应的方案细项。
- 方案的规则由 `tests/run.sh` 用拼音版、五笔版、日文版、越南文版和藏文版的声明覆盖；设置同步按版本过滤的规则在 Rust 里，由 client-core 的过滤测试和 `crates/host-api/src/ffi/harmony_settings/tests.rs` 覆盖。

要发一个版本（以五笔版为例）还差这些，本分支没有做：

1. 版本表：给 wubi、pinyin 填 `platforms.harmony` 段，至少包括 `bundleName`（例如 `app.msime.hmos.wubi`）和 HAP 文件名；同时扩展 `editions.schema.json`、冻结基线 `editions.frozen.json` 和 `scripts/test-editions.py` 的跨版本唯一性检查。不同的 `bundleName` 各有各的沙盒，`files/state`、`files/engine` 和偏好文档自然分开。
2. 工程：`build-profile.json5` 为每个版本加一个 product，覆盖 `bundleName`、应用名（水杉五笔、水杉拼音；图标与 full 相同）和输入法扩展在系统列表里显示的名字，并把版本 id、方案和默认方案作为构建参数写进去，再让 `AppEdition.current()` 读出来；`entry` 的 target 用 `applyToProducts` 关联到这些 product。多 product 下 hvigor 的这些参数我没有在本仓验证过。
3. 设置页：`Settings.ets` 的 `hostCapabilities` 取自 `msime_client_host_capabilities`，这个 C ABI 目前只接受平台名、不按版本收窄方案，共享设置页因此仍会列出 full 的全部方案。需要让它（或 Harmony 一侧）按版本调用 client-core 的 `HostCapabilities::narrow_to_edition`。
4. 资源：`stage-resources.sh` 目前按 full 的 `resources/desktop-dictionary.lock.json` 暂存，要改成按版本的 `resources/editions/<id>.lock.json`；五笔版不带日文词典，五笔版和拼音版都不带粤拼、注音、笔画词库。
5. 签名与发布：零售设备装不上不签名的 HAP。要在 AppGallery Connect 为每个 `bundleName` 建应用，申请发布证书和带共享沙箱授权的 Profile，扩展 `sign-release.py` 的包名校验；`release-harmony.yml` 目前只构建并签名 default product，增加版本时要逐个构建与签名，产物名带版本 id。
- `native/`：NAPI/C++ 适配层。
- `tests/`：不依赖设备的 TypeScript 键盘逻辑测试。
- `AppScope/`、`entry/src/main/resources/`：应用元数据和资源。
- `build-native.sh`、`stage-resources.sh`、`stage-voice-runtime.sh`：共享 Host API、NAPI 库、固定资源和 sherpa-onnx HAR 的构建/暂存入口。
- `entry/src/main/resources/rawfile/settings/index.html`：设置页的单文件打包产物，由 `stage-settings.sh` 从 `apps/harmony` 生成，不提交，见下方[设置页打包](#设置页打包)。

Windows 文档里的“自定义候选窗翻译”在 Harmony 上没有设置入口。Engine 仍在每个宿主上读这份覆盖层——`prepare_translation_sidecar` 先看用户数据目录（`<state>/user/custom_translations.txt`）再看资源目录——但应用沙盒里的这个文件用户无法直接放入，设置页也不再提供编辑它的「自定义候选释义」。**不要把它写进已暂存的资源目录**：那里按锁文件逐项精确校验，多一个文件就会让键盘拒绝启动。

设置页的本地词库管理复用共享设置 UI 和 `msime_client_dictionary`：可分页查看、编辑、导入、导出和处理失败队列。ArkTS 设置桥只接受操作 JSON；引擎资源和状态目录始终由宿主从应用沙盒准备，WebView 不能提交路径。词库写操作需要 Engine 独占维护窗口：空闲时会短暂重建会话并恢复语言、九键和焦点状态；正在组合输入时会返回忙碌错误，不会替用户取消输入。读取操作可与活动会话并行。

手机的「词库」页按 Android 的 `LexiconPage` 布局，不再是那张按种类查词条的桌面表单（`packages/ui/src/settings/harmony-phone-dictionary.tsx`，只在 HarmonyOS 手机上出现，2in1 仍用桌面表单）：「已安装」列出拼音词库和命名词库，点进去启用、停用、加词或删除；「管理」新建、导入（导入成一个新词库）、导出和刷新；「发现词库」把社区词库装成一个独立的词库，社区页的「添加」也走同一条路，之后可以停用或删除。命名词库走 `msime_client_dictionary_collections`：设置页经 `startRequest` 的 `dictionary_collections` 调 NAPI `dictionaryCollectionsAsync`，导入最多 16 MiB，在原生工作线程上执行；词条由 client-services 经个人词库队列分批送进 Engine：设置应用排空它自己的队列，每排空一批就用 `dictionaryCollectionsAsync` 送下一批（`flush`），和 Android 键盘的 `flushSent` 一样，写进设置应用词库的词再经「我的词条」交换到键盘（见上文「「我的词条」只有一份」）。导入来源依次是文本文件、搜狗细胞词库（`.scel`）、Rime 词典和纯汉字词表；细胞词库是二进制文件，页面按 base64 放进同一个 `import` 动作的 `bytes_base64`（文件不超过 8 MiB），`KeyboardSession.dictionaryCollections` 只检查操作名、把整个动作原样交给原生层，由 client-services 的 `dictionary::scel` 转成标准文本后照文本导入。2in1 的桌面表单也能导入细胞词库：页面经 `startRequest` 的 `dictionary_scel_text` 调工作线程上的 NAPI `dictionaryScelTextAsync`（`msime_client_dictionary_scel_text`）把文件转成标准文本，再照常经 `dictionary` 的 `import` 导入。细胞词库文件受表单原有的 1 MiB 上限约束，但这条导入一次请求只收 64 KiB 文本、只看 1000 行，真实词库转出的文本大多比这长，所以页面只送转出文本开头的整行（最多 1001 行、不超过 64 KiB，`leadingImportRows`），报告说「文件过长，仅导入了前一部分」；要导入整个细胞词库用手机上的命名词库。`scripts/test-harmony-dictionary-collections.py` 守着这条接线。API 12 起设置应用与键盘不共用 `files/state`（见上文「输入法扩展的独立沙箱」），命名词库的元数据（名字、开关、来源）只在设置应用里，键盘只收到其中的词；没有共享沙箱时（模拟器上）这些词只在设置应用那份词库里。

「我的词条」经 WebDAV 网盘同步（[docs/dictionary-webdav-sync.md](../../docs/dictionary-webdav-sync.md)）也在共享设置页的词库页里：页面经 `startRequest` 的 `dictionary_sync` 发动作，`HarmonyAccountCloudBridge.dictionarySync` 补上设置页词库管理用的同一份 HostOptions、本沙箱的 `files/state/dictionary-sync` 和设备名（规则在 `account/HarmonyAccountRequests.ts` 的 `dictionarySyncRequest`），再交给工作线程上的 NAPI `dictionarySync`（`msime_client_dictionary_sync`）。目录不是 `files/state` 本身，那里有云剪贴板中继同名的密钥串和网盘缓存；它也不在共享沙箱的镜像条目里。同步的是设置应用这份个人词库；键盘那份经上文「「我的词条」只有一份」的交换跟它收敛成同一组，网盘来的词写进设置应用的词库后，下一次交换就带给键盘。键盘扩展不调这个入口：它没有云剪贴板那样「只用已有设备身份」的开关，键盘一调就会登记成同一台手机的第二台设备，`scripts/test-harmony-dictionary-sync.py` 守着调用点只在设置应用。

## 设置页打包

设置页是 `entry/src/main/resources/rawfile/settings/index.html`，由 `apps/harmony` 从共享设置 UI（`packages/ui`）构建，**不提交进仓库**（已加入 `.gitignore`）。每次打 HAP 之前运行：

```sh
bash platforms/harmony/stage-settings.sh   # 需先在仓库根目录 pnpm install --frozen-lockfile
```

它必须是**单文件**：`resource://` 文档的 origin 为 null，WebView 会拒绝跨 origin 拉取模块脚本和样式表，所以脚本、样式和资源全部内联进 HTML，体积在 1 MB 以上；脚本在构建后确认目录里只有这一个文件。`entry/hvigorfile.ts` 在 hvigor 配置阶段检查它存在且非空，缺失时直接报错并给出上面这条命令——没有它的 HAP 能装能跑，只是设置窗口一片空白、没有任何报错，所以不让它被打出来。

这个文件以前是提交进仓库的，理由是 `hvigorw assembleHap` 不调用 Node 工具链。它先是漂移过：52 个改动 `packages/ui/src` 的提交期间包一次都没重建，HarmonyOS 的设置页一直在渲染别处已经不存在的界面。为此加的门禁要求提交的产物与源码逐字节一致，于是每个改共享 UI 的 PR 都要重新生成这个 1 MB 的单行文件，任意两个同时在途的 PR 必然在它上面冲突，合并时只能再构建一次来解决。可打 HAP 本来就要先跑 `stage-resources.sh`、`build-native.sh`、`stage-voice-runtime.sh` 这些准备步骤，在同一处构建设置页，它就不可能比同一份检出里的 UI 旧，也没有东西可冲突。`scripts/test-harmony-settings-bundle.py`（`verify-local.sh` 与 HarmonyOS CI 都跑）现在检查它没有被重新提交、仍被忽略，并且仍能构建成单文件。

## 本地构建

准备 DevEco Studio 提供的 OpenHarmony NDK，或设置 `MSIME_OHOS_NDK` 指向包含 `build/cmake/ohos.toolchain.cmake` 的 NDK。先安装依赖（根目录 `pnpm install --frozen-lockfile`）和对应 Rust target，再运行。Engine 是纯 Rust crate（`crates/engine`），SQLite 由 `rusqlite` 的 `bundled` 特性用 NDK 的编译器包装一起编进 `libmsime_host_api.so`，不需要另备设备端依赖前缀：

```sh
resource_dir="$(cargo run --quiet -p msime-client-core --example install_resources --locked -- target/resources)"
bash platforms/harmony/stage-resources.sh "$resource_dir"
bash platforms/harmony/stage-settings.sh
MSIME_OHOS_NDK=/absolute/openharmony/native \
bash platforms/harmony/build-native.sh arm64-v8a
bash platforms/harmony/stage-voice-runtime.sh
cd platforms/harmony
# 使用 DevEco SDK 配套且已加入 PATH 的 ohpm、hvigorw、Node 与 Java 21
export DEVECO_SDK_HOME=/Applications/DevEco-Studio.app/Contents/sdk
export NODE_HOME=/Applications/DevEco-Studio.app/Contents/tools/node
ohpm install
hvigorw assembleHap --no-daemon
```

`NODE_HOME` 只让 Hvigor 使用 DevEco 配套 Node；根目录 pnpm 仍用项目要求的 Node 版本。不要把 DevEco 的 `tools/node/bin` 提到 PATH 前面：当前配套 Node 18 不满足 pnpm 的最低版本，完整 quick 会在设置页构建阶段失败。

`stage-resources.sh` 还把仓库自带的七套辅助码表（`resources/helpcodes`，不在词库发布里）连同来源声明放进 `resfile/engine/helpcodes/`：Engine 从资源目录下的 `helpcodes/` 读辅助码表，共享校验放行这个真实目录。`StagedResources` 按相对路径列出其中的文件，所以辅助码表跟其他资源一起复制到 `files/engine`，表有变化时同样重新暂存。

引擎编进了取自 libhangul `data/hanja/hanja.txt` 的韩语汉字表，其 BSD-3-Clause 许可第 2 条要求二进制分发附带声明；引擎的粤语与注音方案所用的粤拼、注音音节与词条分别取自 rime-cantonese（CC BY 4.0，要求署名）与 libchewing-data（LGPL-2.1-or-later，要求附许可证全文与源码位置），笔画方案的笔顺取自 rime-stroke（LGPL-3.0，另含 CNS11643 全字库的署名要求），鸿蒙版的这些方案只在词库随包时提供（见上文「粤语、注音与越南语」），但声明随每一份引擎走，各平台共用一份清单，所以 `stage-resources.sh` 把 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`、`rime-cantonese-CC-BY-4.0.txt`、`libchewing-data-LGPL-2.1.txt` 与 `rime-stroke-LGPL-3.0.txt` 暂存到与 `resfile/engine` 相邻的 `resfile/licenses/`，随 HAP 一起分发；放在 `engine` 里会被锁文件校验拒绝。拆字反查（工具面板「本地输入」的最后一项「拆字」，实体键盘上是 Shift+C）的部件拼音表由 kfcd/chaizi《漢語拆字字典》（CC BY 3.0，© 2015 開放詞典）改编后编进引擎，署名、固定提交和改动说明在 `resources/licenses/kfcd-chaizi-CC-BY-3.0.txt`；表里的读音、名次和繁体字形取自 Unihan，它的 Unicode License V3（`resources/licenses/Unicode-License-V3.txt`）一起暂存到 `resfile/licenses/`。同一目录里还有引擎本身和它链接的 rink-core 的 `MPL-2.0.txt`、`octagram.rs` 所移植的 librime-octagram 声明 `librime-octagram-BSD-3-Clause.txt`（末尾附 darts-clone 的 BSD-2-Clause 条款）、`cxx_sort.rs` 逐步复现的 libc++ 的 `libcxx-Apache-2.0-WITH-LLVM-exception.txt`、符号模式 LaTeX 表的 `latex-symbols-MIT.txt`，以及随包词库里 MIT 与 Apache-2.0 数据的许可证全文 `pinyin-data-MIT.txt`（`msime-pinyin.db` 的单字读音）、`ECDICT-MIT.txt`（`msime-english.db` 的中英释义）、`rime_kaomoji_dict-MIT.txt` 与 `python-pinyin-MIT.txt`（`msime-others.db` 的颜文字与关键词整词读音）、`rime-wubi86-jidian-Apache-2.0.txt`（`msime-wubi.db` 的 86 码表），`msime-pinyin.db` 与 `msime-stroke.db` 里改编自 rime-wanxiang 的权重的署名、改动说明与 CC BY 4.0 全文 `rime-wanxiang-CC-BY-4.0.txt`，以及逐个列出 `resfile/engine` 里词库来源与条款的 `msime-engine-dictionary-NOTICE.md`（含 `msime-bigram.bin`、`msime-trigram.bin` 要求的中文维基百科 CC BY-SA 4.0 署名）。水杉输入法本身和 `msime-pinyin.db` 里 rime-ice 的数据都是 GPL-3.0，仓库根目录的 `LICENSE` 以 `GPL-3.0.txt` 暂存在同一目录，与 macOS 包里的名字相同。这些文件只随 HAP 分发，设置页「关于 › 法律信息 › 开源许可」打开的是 GitHub 上的 `LICENSE`，不在应用里逐个列出 `resfile/licenses/`。

`stage-resources.sh` 的第二个参数（默认 `target/offline-glosses`）是可选的非英文离线释义，由 `scripts/build_offline_glosses.py` 生成。数据库和 `offline-glosses-NOTICE.txt` 都在时暂存到 `resfile/offline-glosses`，键盘启动时用同一个 `StagedResources` 复制到 `files/offline-glosses`，与 `files/engine` 相邻，引擎就在那里找 `zh-<lang>.db`；新包不带它们时会删掉旧副本。已安装词典的目标语言在翻译查询里以 `offline_gloss_languages` 出现：用户自己配置的在线翻译先答，离线词典只补在线没答上的候选，同一行按目标顺序合并。

支持 `arm64-v8a`、`armeabi-v7a` 和 `x86_64`。原生库暂存到 `entry/libs/<abi>/`，这些目录是构建产物，不提交到仓库。

`stage-voice-runtime.sh` 用 `scripts/fetch_voice_runtime.py --platform harmony` 取回 `resources/voice-runtime.lock.json` 按 SHA-256 锁定的 sherpa-onnx `.har`，复制到 `entry/libs/sherpa_onnx.har`（同样被忽略、不提交）。`entry/oh-package.json5` 以固定文件名依赖它，所以不先暂存 `ohpm install` 会失败。本地语音识别（`local` 提供方）在 `workers/LocalAsrWorker.ets` 里加载 `voice_input.asr_model_path` 指向的模型目录，音频不出设备；该 HAR 自带的 `libc++_shared.so` 与 `build-native.sh` 暂存的同为 NDK 运行时，`entry/build-profile.json5` 用 `pickFirsts` 只打包一份。`arm64-v8a` 和 `x86_64` 之外（即 `armeabi-v7a`）上游 HAR 不带库，本地识别在该 ABI 上不可用。

系统语音（`system` 提供方）以写音频模式（`recognitionMode: 0`）启动 CoreSpeechKit：麦克风由 `HarmonyPcmCapture` 采集，按 1280 字节切帧后 `writeAudio`，`isFinal` 只结束一句，`isLast` 才结束整个会话。资源准备仍使用根目录固定的 `resources/desktop-dictionary.lock.json`，不得把本机路径、凭据或用户输入放入 HAP。

## 应用市场正式签名与发布

`release-harmony.yml` 使用 `buildMode=release` 构建。`publish=true` 时必须配置四个仓库 secrets：`HARMONY_RELEASE_KEYSTORE_BASE64`（PKCS#12）、`HARMONY_RELEASE_KEYSTORE_PASSWORD`、`HARMONY_RELEASE_CERTIFICATE_BASE64`（PEM 证书链）、`HARMONY_RELEASE_PROFILE_BASE64`（华为签发的应用市场 release Profile）。密钥别名固定使用 `msime`，密钥密码与密钥库密码相同。签名资料不能提交到仓库，也不能将 debug Profile 用作正式发布资料。

Profile 必须允许包名 `app.msime.hmos`、共享沙箱 `group.1516753898738609107` 和受限权限 `ohos.permission.READ_PASTEBOARD`。缺共享沙箱授权时，在 AGC 手动重新生成发布 Profile，不能本地编辑 P7B。构建前会核对这些授权、有效期与证书及密钥的对应关系；不满足就停止，不会发布未签名包。

签名脚本还检查版本、原生库及必需资源，调用华为工具验证签名与代码完整性，并核对签名前后内容。GitHub release 只放 `msime-harmony-X.Y.Z.hap` 和 `SHA256SUMS`；供应用市场上传的 `msime-harmony-X.Y.Z.app` 位于 workflow artifact `appgallery-msime-harmony-X.Y.Z`。仅构建的 nightly 不读取正式密钥，产物仍标为 unsigned。

应用市场类型的签名材料用于市场分发。生成 APP 包、创建 GitHub release 与通过华为市场审核各是独立步骤；创建 GitHub release 不代表应用已在市场上架。

`Release HarmonyOS` 的 `appgallery` 选项控制华为测试专区自动化；`publish` 仍单独控制 GitHub release：

| `appgallery` | 行为 |
| --- | --- |
| `none`（默认） | 保持原来的构建、签名与 GitHub 发布行为，不读取华为 API 凭据 |
| `upload` | 自动生成正式签名 APP，上传到华为测试专区，等待 HarmonyOS 包解析通过 |
| `public-test` | 上传并解析成功后，新建或更新公开测试草稿、绑定包、提交审核 |

首次配置一次即可：

- 仓库变量 `HARMONY_AGC_APP_ID`：AGC 中该应用的 App ID。
- 推荐仓库 secret `HARMONY_AGC_SERVICE_ACCOUNT_JSON`：在华为「开发者管理 → ID、证书和Profile → 凭证 → 开发者级凭证 → 服务账号凭证」创建 APP 管理员角色，生成公私钥后下载的完整 JSON。脚本用 OpenSSL 生成 PS256 JWT，直接调用 Connect API；私钥只临时落盘，退出删除。
- 兼容已有 API Client：仓库 secrets `HARMONY_AGC_CLIENT_ID`、`HARMONY_AGC_CLIENT_SECRET`，需要开发者级 Connect API 凭据，APP 管理员或更高角色。服务账号 JSON 优先。应用内 SDK 的 client secret 不能替代这些凭据；华为建议新建凭据使用服务账号。
- 可选 secret `HARMONY_AGC_TEST_CONFIG_JSON`：测试版本更新参数的 JSON 对象，可覆盖 `languages`、`deviceTypes`、`distCountryList`、`extInfo`、`openTestInfo`、`packagePermissionIntroVideoList`、`versionVocReq`。它可能包含联系人或资质，不能提交进仓库。未设置的资料由华为继承最近测试或全网版本；首次提交前需要在 AGC 填妥真实资质、素材、隐私政策等资料。

公开测试默认从运行时刻的次日开始，持续 28 天、安装上限 10000、生成分享链接、不发送通知。可通过 `openTestInfo` 覆盖；开始时间必须在未来，持续不超过 30 天。脚本不修改公开测试不支持变更的 `groupInfos`。`test_version_id` 留空则创建公开测试，填写已有公开测试草稿 ID 则继续该草稿；不要填写邀请测试或在架版本 ID。

例如从已发布 HAP 生成市场 APP 并提交公开测试，无须重编输入法：

```sh
gh workflow run release-harmony.yml --ref develop \
  -f version=0.2.1 -f publish=false -f source_release=harmony-v0.2.1-beta \
  -f appgallery=public-test
```

新版本只需选择 `appgallery=public-test`，即可在同次构建中签名、上传、等待解析、提交审核。CI 在耗时构建前检查华为鉴权和本地配置；API 拒绝资料、权限不足、包解析失败或超时都使任务失败。不会自动重试创建或提交请求，避免重复草稿。上传后的包 ID、创建后的测试版本 ID 会立即写入 Actions 日志与步骤摘要，后续失败也保留；日志不包含 token、OBS 签名地址或原始资料。

如果上传已经完成，重新运行时设置 `test_package_id` 为上次的 `package_id`，`test_version_id` 为上次的草稿 ID（如已创建），同时设置 `publish=false`、清空 `source_release`。恢复路径直接查询已有包并继续提交，跳过构建、签名和重复上传。已经提交或生效的版本应先在控制台确认状态，避免把恢复操作当成重复发布。

提交成功的摘要为 `review_status=submitted`，表示等待华为审核；审核通过和分享链接生效仍由平台处理。日常 nightly 的 `workflow_call` 默认 `appgallery=none`，不会自动提交测试或读取市场凭据。可离线运行 `python3 scripts/test-harmony-appgallery.py` 验证上传与提交状态机；测试只用合成数据，不访问华为。

接口依据：[上传地址](https://developer.huawei.com/consumer/cn/doc/app/agc-help-upload-api-upload-url-0000002236201294)、[HarmonyOS 包解析状态](https://developer.huawei.com/consumer/cn/doc/app/agc-help-publish-api-query-compile-status-0000002236041434)、[更新测试版本](https://developer.huawei.com/consumer/cn/doc/app/agc-help-test-api-modify-test-version-0000002271160657)、[提交测试版本](https://developer.huawei.com/consumer/cn/doc/app/agc-help-test-api-submit-test-version-0000002236201334)。

## 模拟器验证（2026-09-20）

首次在 HarmonyOS 模拟器上跑起来，记录可复现路径与结果。当时的 bundleName 是 `app.msime.client`，下文日志与验证记录里的包名照原样保留；之后改成过 `app.msime.harmony`，但 AppGallery Connect 不接受包名里含保留字 `harmony`，所以现在的 bundleName 是 `app.msime.hmos`，命令已按它改写。镜像为 DevEco 自带的 HarmonyOS 6.0.1(21) phone，与项目 `compileSdkVersion` 一致：

```sh
emu=/Applications/DevEco-Studio.app/Contents/tools/emulator/Emulator
"$emu" -license accept && "$emu" -list        # 实例名
"$emu" -start "<实例名>"                       # 首次约需 6–8 分钟才向 hdc 注册
export PATH="<command-line-tools>/sdk/default/openharmony/toolchains:$PATH"
hdc list targets -v                            # 等到 Connected
hdc file send <hap> /data/local/tmp/msime.hap
hdc shell bm install -p /data/local/tmp/msime.hap
hdc shell ime -e app.msime.hmos -f             # 启用输入法
hdc shell hilog -x | grep A00051/MSIME         # 本宿主的日志域
```

干净安装后的实际输出：

```
MSIME: module loaded
MSIME: staged /data/storage/el2/base/haps/entry/files/engine
MSIME: session 1 created
MSIME: panel ready: phone, soft keyboard
```

即：系统接受了该输入法（`Succeeded in enabling IME. status:FULL_EXPERIENCE_MODE`），NAPI 模块加载、资源暂存、Engine 会话建立、软键盘面板创建，整条 ArkTS → NAPI → Rust → C++ Engine 链在设备上通。`bm install` 接受未签名 HAP（模拟器）。

切换本身已验证：解锁屏幕后 `ime -s app.msime.client` 成功，`ime -g` 返回 `status: FULL_EXPERIENCE_MODE`。2in1 实例（`const.product.devicetype` = `2in1`、API 23、aarch64）上同样验证通过：安装、启用、切换为当前输入法均成功，原生模块加载。这是硬件键盘相关功能（模式和弦、语音快捷键、维护和弦）的目标形态。

模拟器起不来的真实原因是**磁盘空间**，不是模拟器本身。上面这段曾把它记成"三个实例三种失败，都在模拟器的显示/UI 层"——那是错的，现予纠正：sceneboard 卡死、qemu 不启动、显示不出帧、`uitest` 等待 UI 服务超时，这些症状都出现在宿主机根分区被构建产物占到 97%（可用 16 GB）的时候。清掉仓库里可重建的 `target/debug`（约 6.6 GB）之后，同一个 2in1 实例 **45 秒启动**，显示、桌面、触摸注入与截图全部正常，随后完成了硬件按键的整套验证。

教训是判据问题：症状出现在模拟器的显示层，不等于成因在那里。排查顺序应当是先 `df -h` 再怪模拟器。

清理时有一处必须当心：Android AVD 就放在仓库的 `target/android/avd-home/` 下，运行中的 qemu 会持有它的镜像文件句柄。macOS 允许删除已打开的文件，进程不会崩但后备存储没了，所以不能对 `target/` 整体 `rm -rf`；用 `lsof +D target/<子目录>` 逐个确认再删。

已验证（2026-09-20）：输入法接管真实编辑器。验证办法是一个一次性探针应用——页面上一个 `TextInput` 加 `.defaultFocus(true)`，加载即自动取得焦点，因此不依赖显示层出帧、也不依赖触摸注入命中图标（这台机器上模拟器的显示栈正是这两处失效）。探针启动后本宿主日志出现 `attached to editor: pattern=-1 enter=6`，`ps` 显示 `app.msime.client:inputMethod` 进程在运行并为该输入框回报 `SetTextFieldAvoidInfo`。即系统把一个真实编辑器的输入路由给了本输入法，本输入法作出了响应。

该验证同时暴露了两个缺陷，均已修复：OHOS 的 AsyncCallback 无论成败都会传入 `BusinessError`，成功时 `code` 为 0，因此 `if (error)` 恒为真——设置页每次加载成功都会记一条"加载失败"，真正的失败反而淹没其中；手写识别更严重，`componentSnapshot.get` 的回调同样这样判断，于是每一笔都在看快照之前就走了失败分支，手写从来没有识别成功过。

硬件按键的注册不按形态一刀切：`KeyboardExtensionAbility` 早期只在 `isDesktop()` 时注册 `keyEvent`，那段注释论证的是「2in1 上这是唯一通路」，并没有说明手机不该注册。手机或平板接上蓝牙/USB 键盘时，不注册意味着框架把按键直接交给编辑器，物理键打出原文字母而完全不组字。现在是形态决定画不画键、枚举决定路不路由键（`HardwareKeyboardPolicy`），判据是 `ALPHABETIC_KEYBOARD` 而非 `sources` 含 `keyboard`——后者在每台手机上都因音量与电源键成立。因此任何接得上键盘的设备都能验证这条路径，不限于 2in1。

## 构建门禁与逻辑回归

**先 `ohpm install`，再 `hvigorw assembleHap`，两步缺一不可。** `ohpm install` 在 `entry/oh_modules/` 建出指向 `src/main/cpp/types/libmsimeclient` 的链接，`import client from 'libmsimeclient.so'` 才解析得到那份 `.d.ts`。没有这一步，ArkTS 把整个 NAPI 边界当作无类型处理并照样打包成功——一个全新的 worktree 默认就是这种状态，于是"构建通过"实际上没有检查过任何一处原生调用。本仓的 `Settings.ets` 里就藏着一处这样的错误，直到装上模块才暴露出来。

`hvigorw assembleHap` 是必须跑的一道门，不是可选项。ArkTS 的几条限制——`@Builder`/`build` 体内不能声明局部变量、修饰符不能挂在 `if/else` 上、对象字面量必须对应已声明的接口——都不会被 `tests/run.sh` 或任何 TypeScript 检查发现，因为那些只编译 `.ts`，不编译 `.ets`。曾经有 33 个这样的错误一路进到 develop，HAP 整段时间根本打不出来。改过 `.ets` 就跑一次打包。

不依赖设备的逻辑回归：

```sh
bash platforms/harmony/tests/run.sh
```

该命令编译并运行 `tests/keyboard-logic.test.ts`，CI 的 `ci-platforms.yml` harmonyos job 跑的就是这条。三道门禁各自回答不同的问题，互相替代不了：`tests/run.sh` 覆盖纯逻辑，`build-native.sh` 覆盖指定 OpenHarmony NDK 下的 Rust/C++/NAPI 交叉构建与 ELF 导出检查，`hvigorw assembleHap` 覆盖 ArkTS 编译与 HAP 打包。设备侧的系统输入法注册、焦点与编辑器接管、面板生命周期见上面的「模拟器验证」两节。

## Canvas 候选皮肤

`base = "canvas"` 的候选包由共享 Rust 校验，普通候选条、展开网格及九键中栏使用 Image Kit 解码和原生 Canvas 绘制九宫格与自由挂件。触屏固定高度时等比缩小留白，2in1 的原生面板尺寸包含留白与预编辑行距。面板成功绘制后才启用贴图配色，失败保留可读回退。GIF/WebP/APNG 的动画取决于设备 Image Kit 实际提供的帧，不能提供时尝试静态图；HAP 编译通过不能代替动画与输入法设备验收。设计边界与验证记录见[决策笔记](../../.agents/notes/implemented/feature/2026-10-19-harmony-candidate-canvas.md)。
