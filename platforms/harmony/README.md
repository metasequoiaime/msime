# HarmonyOS 输入宿主

ArkTS 宿主与 NAPI 原生边界是完整实现：键盘扩展、设置应用、账号、社区、AI、语音、手写、候选与词库都在本目录内。能力对照见 [docs/harmony-parity.md](../../docs/harmony-parity.md)：用什么方法比过 MSIME-Apple、哪些是按平台特性裁剪而不是欠账。

OpenHarmony 适配保留 ArkTS/ArkUI 应用入口与 NAPI 原生边界。共享输入算法、组合状态、配置校验和资源准备继续由 Rust Host API 与 Rust Engine（`crates/engine`）提供；`platforms/harmony/native/client_napi.cpp` 只负责 NAPI 注册和 C ABI 转发，不复制候选分页或输入状态机。

繁体输出只在显示与上屏边界转换：候选条与展开候选面板的显示文字、Engine 提交、`insert` 与 `insertWithSource` 经过 `KeyboardSession.asTraditional`，Engine 的候选原文、候选身份（按序号选择）和组合文本保持简体；由 `ChineseOutputPolicy` 决定是否适用（dedicated English、日语方案、临时日语保留原文），转换本身走 NAPI `simplifiedToTraditional` 调共享导出 `msime_client_simplified_to_traditional`，即 OpenCC s2t 词级转换，与 Windows、macOS、Linux、Android、iOS 逐字一致——「头发」出「頭髮」、「发展」出「發展」，ICU `i18n.Transliterator` 的逐字转换分不开这两个「发」。C ABI 拒绝的输入（内嵌 NUL）返回 `null`，保留原文。`scripts/test-harmony-traditional-output.py` 钉住从 C 头文件到调用点的这条接线，并拒绝宿主源码里重新出现 ICU 转写。

Harmony 设置页暴露共享的模糊拼音规则、触摸输入方案启用列表、自定义触摸键盘皮肤设计和候选英文释义开关。这四项此前都只有键盘一侧在消费：`PreferencesStore` 里有值，键盘准备 Engine 会话时会读，但设置页从未开启对应的客户端开关，用户没有任何途径改动它们。它们各自只写共享偏好，不需要平台能力。

手写方案使用 HarmonyOS Core Vision Kit 的 `textRecognition`：键盘内的 ArkUI Canvas 记录受界限约束的笔画，组件快照转换为 `PixelMap` 后交给系统 OCR，候选结果仍由共享 Engine 会话提交到编辑器。OCR 服务不可用时保留明确提示，不回退到伪造的 Engine 手写模型；该路径需要设备提供 `SystemCapability.AI.OCR.TextRecognition`。

语音输入使用 HarmonyOS Core Speech Kit 的 `speechRecognizer` 离线短语音模式。工具面板可以开始、停止或取消识别，最终文字经过长度和控制字符边界检查后通过当前 `KeyboardSession` 提交；原始音频始终留在系统服务内，不写入文件、不进入日志，也不复制到 Engine。该路径需要 `SystemCapability.AI.SpeechRecognizer` 和用户授予 `ohos.permission.MICROPHONE`，单次录音受系统 60 秒上限约束。

云联想与 AI 联想复用 Engine 的 `online_query` 代际契约：Harmony NAPI 只传递有界查询和结果，ArkTS 通过系统 HTTPS 栈异步访问云候选或用户配置的 Chat Completions 服务，结果再交回 Engine 做会话、偏好和 generation 校验。请求防抖、超时、响应大小、重复候选和控制字符检查均在宿主边界完成，失败只丢弃可选展示结果，不阻塞本地输入，也不把查询或响应写入日志。

共享设置页的「AI 对话」现在也由 Harmony 承载，对应 MSIME-Apple 的 `KeyboardChatView` 与 `BackendChatClient`。它与「AI」页那个用户自配的服务不是一回事：后者带着用户自己的 endpoint 和 token，而这一个用宿主已经持有的账号会话认证，所以页面只在登录后才提供它，WebView 全程看不到任何凭据。模型列表走 `/v1/models`，回复走 `/v1/chat/completions`，请求与响应的边界在 `AccountCloudBridge` 里校验，数值与 Apple 的 `BackendChatClient` 逐条对齐——16 条历史、单条 16 KB、请求体 64 KB、模型 id 200 字节、最多 33 个模型且默认模型必须在列表内。回复内容按字节设限而不按字符类别：这里的换行是内容而不是控制字符，按桥上其它校验器的规则会把每一条分段的回答都拒掉。

完成请求单独走一条 `chat` 请求类型，为的是它自己的期限。一次补全是模型在写字，共享客户端给它 125 秒；与账号请求共用一条通道就只有两种结果——要么把还差一点就返回的补全提前判成超时（随后到达的回复会因为没人在等那个编号而被丢弃），要么让一次卡住的资料拉取也空转两分钟。读超时因此是每请求的，连接超时仍是统一的 30 秒：连不上就是连不上，哪个端点都一样。

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

下载是唯一不止一次请求的操作：设计要经系统 HTTPS 栈取回，再装到键盘上并存进皮肤库，而后两步是一次原生调用 `msime_client_community_skin_install`。合在一起不是为了省一次跳转——试用记录正是"被换掉的是哪套皮肤"的记忆，导入失败必须把它结束掉，否则用户身上是一套从未保存、也无从还原的设计。两次调用意味着这段回滚归宿主所有，而每个拥有它的宿主都会写得略有不同。

试用的结束不碰网络：保留还是还原，是对一套已经生效的皮肤的回答，由设备作答。`restore_pending` 是崩溃恢复，在没有待处理试用时调用也是安全的——它运行的时刻正是还没人知道上次会话是不是停在试用中间的时候。

装完与结束试用之后宿主都会把新的偏好文档推给页面，理由与设置同步那条相同。

社区页的「词库 / 回复」分类也由 Harmony 承载，对应 MSIME-Apple 的 `CommunityResourcesView` 与 `BackendCommunityResourceClient`。公开列表和单个资源的详情不要求登录，理由与皮肤画廊相同；「我的作品」和「收藏」要求登录——它们是关于某个账号的问题，没有账号时答案要么是空的，要么是别人的。

资源内容按它是什么来校验：回复只有 prompt，词库是 1 到 128 条词条且没有 prompt。共享服务对其它组合是整体拒绝而不是忽略多出来的那一半，因为一个带着 prompt 的"词库"，它的作者以为自己发布的是别的东西。

合并共享词库是两次请求：服务端需要知道这是合并到用户自己词库的哪个版本上，所以先读再写，读到的版本号进写请求。共享服务也是这么做的，这也是它不能做成页面自己发一次请求的原因——页面若在两次之间持有那个版本号，就是在用户随时可以离开的一个界面上持有它。

回复模板的保留与移除完全不碰网络，写的是 `files/CommunityLibrary.json`，也就是 `KeyboardSession.communityReplyTemplates` 读的那个文件。它在 state 目录旁边而不是里面：键盘扩展拿到的是 `files/state`，写进那个目录的库会被写在读取方从不查看的地方。写入走新的 C ABI `msime_client_community_resource_library`，因为写它需要那个 store 的校验（reply 类型、非空 prompt、没有词条、五十条上限），而键盘解析这个格式是严格的——宿主自己写等于给一个严格解析的格式添了第二个作者。

请求信封上限从 64 KB 提到 512 KB。每个操作仍各自校验自己的载荷，这一条只是解析前拦掉荒谬输入的第一道；但发布一份社区词库最多带 128 条、每条最长 1024 字符，而服务端允许 350,000 字节内容，64 KB 的信封会在本宿主上拒掉服务端本会接受的资源。

AI 生成皮肤也由 Harmony 承载，对应 MSIME-Apple 的 `AISkinGenerationView` 与 `AISkinService`。`BackendAiSkinService::generate` 在 Rust 里跑完整条流水线，HTTP 归 Rust 的宿主直接用它；本宿主的设置界面走系统 HTTPS 栈，所以自己发这四类请求，把不属于传输的那部分问共享客户端：对模型说什么、回答是不是三套可用的设计、返回的图是不是这个客户端会显示的图。这跟候选那边 `aiRequestForQuery` / `parseAiResponse` 的切法是同一个。

指令和解析器不能分开。系统提示词点名了解析器唯一接受的那份文档结构，所以 `compose` 把提示词一并返回，而不是让宿主自己写一份——自己写等于在向模型要一份解析器并非为之而写的文档。

步骤之间的规则放在 `AiSkinRunPolicy`，请求放在 `HarmonyAiSkins`。会出问题的是规则那一半：取消要在每一步之间生效而不只在开头；一个任务失败要停掉另外两个，而不是让它们继续为一套没人会看到的方案出图；任务无论成功、失败还是被放弃都要释放——那是记在用户账号上的上游任务，设备上不会再有任何东西回去停它；进度按已完成的图片计数，因为对着屏幕等的人只关心这个数。这些都不需要网络就能测。

进度单独走一条通道，不混在回复里，方向与偏好变更通知相同，并带上页面自己选的 request id：三张图要几分钟，一次从头到尾不吭声的运行和一次已经停了的运行在屏幕上没有区别；而过期运行的计数器不该去驱动新运行的显示。

Apple 的首页（`KeyboardHomeView`）也由 Harmony 承载，但是按本平台裁剪而不是照抄：五个动作里这里有两个。「设置」打开系统输入法列表，选择器是安装第二步的去处，两个都是本宿主的能力。

`openKeyboard` 故意不提供。Android 为它开一个独立的面板窗口；本宿主的键盘是 InputMethodExtensionAbility，编辑器要它的时候才出现，设置应用没有窗口可开。不提供这个动作时那张卡片会回落到共享的屏幕键盘页，那才是这里"让我看看键盘"的诚实版本。表情和剪贴板两个动作不提供的理由相同：在本宿主上它们是键盘自己键面上的界面，不是窗口。

云剪贴板在键盘里也有一份：剪贴板面板（手机的剪贴板键面、2in1 表情面板的剪贴板页）分「本机」与「云端」两栏。云端只在面板打开和点「刷新」时读取一次，没有轮询，复制时不上传，也不读系统剪贴板；点一条就插入当前编辑器。本机历史长按一条出现「发到云剪贴板」，只有已登录且云剪贴板已开启时可用。密码框里没有「云端」这一栏，读取期间换了编辑器的结果直接丢弃，判断都在 `keyboard/clipboard/CloudClipboardPolicy.ts`。键盘不持有凭据：两个进程同属 `entry` 模块，`files/state/account-session.json` 是同一个文件，键盘每次操作都按它新建一个 `AccountCloudBridge`，所以设置页里的登出、换号对键盘立即生效。设置应用和键盘扩展是两个进程，而服务端每次刷新都轮换刷新令牌，有人出示已用过的刷新令牌就吊销整个会话——两个进程同时刷新，或一个进程拿着另一个已经轮换掉的旧令牌去刷新，都会把用户在所有地方登出。所以每一次刷新都在 `files/state/account-session.lock` 的排他文件锁（`fs.File.lock`）里进行：`AccountCloudBridge` 进锁后先重读会话文件，另一个进程已为同一账号存下更新的会话就直接接过来用，只有磁盘上没有更好的令牌时才刷新；写回前再读一次，会话已被登出或换号就丢弃这次轮换。登录、登出、资料回写和令牌被拒后的清除也走同一把锁，被拒时只清除仍是被拒那份的会话，不会误删刚登录的新会话。拿不到锁就不刷新，报暂时不可用而不是去冒吊销的险。会话文件改为写临时文件再原子改名，读的一方不会读到写了一半的文档并把它当作损坏清掉。

使用情况上报、公告与社区审核走 client-core 的共享实现，本宿主只决定何时调用。上报开关是共享偏好 `usage_reporting`（默认开启，设置页关闭后立即清空本地队列）；队列、随机安装 id、每日一次的 `active` 和会话记录都在 `files/state/telemetry`，设置应用和键盘扩展两个进程共用、由 client-core 加锁。一次会话就是一个键盘进程：`KeyboardExtensionAbility` 创建时开始、被正常销毁时结束；设置应用只发送已排队的事件，不再在每次启动时发 `download`。崩溃不装自己的处理器，而是用 HiAppEvent 在下次启动时收系统上报的 `APP_CRASH`（JavaScript 与原生都有），所以崩溃仍按原来的方式结束进程。键盘在开始新会话前等两秒：这期间收到的、属于上一个键盘进程的崩溃写成那次会话的崩溃记录，于是计为 `session_crash`；其余崩溃（设置应用的、或来得太晚的）写成独立记录，只计为 `crash`。原生帧只留文件名加 pc 和符号，信号只留名称和 code，不带地址；`TelemetryPolicy.ts` 里的这些决定由 `tests/run.sh` 覆盖，HiAppEvent 的实际投递时机只能在设备上确认。

公告在设置窗口打开或回到前台时取（client-core 一分钟内直接用缓存），显示为设置页上方一张可关闭的卡片，关闭按公告 id 记在本地。正文是 client-core 用 pulldown-cmark 渲染、原始 HTML 已转义的 HTML，放进一个禁用脚本、CSP 为 `default-src 'none'` 的小 Web 组件里；点链接一律交给系统浏览器或邮件应用。这里没有用 RichText：它没有拦截链接点击的入口，链接会在卡片里打开，而 Web 组件的 `onLoadIntercept` 可以把它拦下来交出去。

社区页仍是共享界面，本宿主的 `AccountCloudBridge` 为它补上三件事：「我的作品」列表和详情带 `fields=moderation`，作品自身的 `moderation` 字段（approved、pending、removed）原样交给页面，由页面只对 removed 显示「已下架」；`report` 操作（皮肤 `community_operation`、词库与回复 `resource_operation`）按固定的六个理由发 `POST /v1/community/reports`，需要会话，设备的匿名账号也算；422 `blocked_content`、503 `screening_unavailable`、403 `account_banned` 分别报成 `community_blocked_content`、`community_screening_unavailable`、`community_account_banned`，与桌面端同名，被封禁时不再去刷新令牌。

`registerJavaScriptProxy` 的名单现在由 `scripts/test-harmony-bridge-parity.py` 守着。ArkTS 对注入对象暴露什么有两处决定——类上的方法，和交给 `registerJavaScriptProxy` 的名字——而页面看得见的只有后者。一个名字只加了一处仍然能通过类型检查、能编译、能打包，然后在真机上以 `msimeHarmony.<name> is not a function` 的形式失败，表现是某一块功能就是不工作，而那恰好在这里谁也跑不了的那个平台上。具名皮肤库那一片就是这么漏的：方法写了，名字没注册，三道绿灯什么都没说。

个人词库文件导入也由 Harmony 承载，对应 MSIME-Apple 的 `PersonalDictionaryImportView` 与 `PersonalDictionaryImport`。共享页面上的那张卡片只在宿主提供 `dictionary.importPersonal` 时出现，此前本宿主不提供。

这一条走队列而不是 Engine，是这里唯一这么做的词库操作。其余操作（列表、单条编辑、导入词库文件、导出）直接取 Engine 的维护锁，那对"在设置窗口里改一个词、并盯着旁边的列表看结果"是对的。导入个人词库文件不是那种操作：用户什么时候导入由他自己决定，键盘开着的可能性和关着的一样大，而"dictionary maintenance busy"不是"请把这些词加进去"的回答。

队列是 Apple 与 Android 宿主已经给出的答案：词条写进 `<preferences_directory>/PersonalDictionary`，键盘在建立 Engine 会话之前先应用一批——那是确定没有会话开着的边界，和 Android 在 `scheduleEngineStartup` 里选的边界相同。每批最多四条；仍有待处理项时，宿主每两秒等一个非组字、非本地模式的空闲点，释放会话、应用下一批、重建会话并恢复中英文、九键与焦点状态。这也是卡片上写"已加入同步队列"而不是"已导入"的原因：在本宿主上那句话同样是实话。

排空是尽力而为的。store 会保留没能应用的词条，失败只是推迟而不是丢失；为一个词库问题拒绝启动键盘，会把它变成"没有键盘"。请求 UUID 是 Engine 的持久回执，同一批在共享状态写回前中断也不会重复应用；损坏状态文件原样保留，读取不存在的队列不会创建目录。

个人词库 JSON 在预览和入队边界按 Engine 规则规范化：`NI HAO` 变成 `ni'hao` 后再参与重复检查，快捷短语允许换行和制表符，其它词条仍拒绝控制字符。Apple 示例里的多行短语以及大于旧 199 UTF-16 单元限制、但不超过 Engine 4096 字节上限的快捷短语都可导入；普通 Tab 文本词库格式原有的 199 单元限制没有因此放宽。

`personal_dictionary_request_json` 此前只有 Tauri 宿主当 Rust 直接调，本次以 `msime_client_personal_dictionary_request` 发布给通过 C ABI 到达这个 crate 的宿主。NAPI 侧的 `personalDictionarySync`（排空那一半）本来就已经导出，只是没有人调用。

键盘上的「AI 润色」也由 Harmony 承载，对应 MSIME-Apple 的 `KeyboardAIView` 与 Android 的 `aiPolishPanel`。

Apple 润色的是**选区**：用户选中一段话，点润色，面板给出那一段的重写。HarmonyOS 不给输入法读取选区的能力——`InputClient` 只有 `getForwardSync`（光标前）和 `getBackwardSync`（光标后），选区只以一对下标的形式通知键盘——照抄那个手势等于画一个不知道自己在操作什么的按钮。所以这里润色的是光标前的文字，也就是用户刚打完的那段话。同一个意图，用这个平台确实提供的东西表达；在手机上它也是更自然的手势：先打，再理。

请求复用 `HarmonyVoicePolisher`，那本来就是本宿主的润色器。这不是顺手：它允许重写后的段落里的换行，而旁边那个 AI 候选解析器拒绝一切控制字符——后者是给候选词用的，把一段重写过的消息送进去会让每一条分段的结果被静默丢掉。会话持有自己的润色器而不是复用识别器那一个：识别器在每次录音开始、结束或放弃时都会取消它，那对转写是对的，对用户正等着的一次重写不是。

替换前会重新读一次光标前的文字并要求原文还在那里。一次润色要几秒，其间用户可以继续打字、移动光标或换个输入框；那时替换会删掉现在那里的东西，再把另一段话的重写放进去。不匹配就拒绝并说明，而不是硬替。

删除长度按码点计算而不是 `String.length`。`deleteBackwardSync(length)` 的文档只写了"length of text"，两种读法对 BMP 之外的字符不一样——句中一个 emoji 是一个码点、两个 UTF-16 单元；本宿主唯一把单位钉死的地方是退格路径的注释，写的是"一个 scalar"，这里采用同一读法。文档措辞留有歧义，所以上面那次重读同时给它兜底：单位错了的代价是一次拒绝，不是一条被改坏的消息。

工具面板里的入口只在配置了润色服务时可用，并随语音设置的变更通知一起重新判定——重写发到用户自己的服务，一个永远失败的卡片只会教会用户忽略这个面板。

词库页的「词库信息」也由 Harmony 承载，对应 MSIME-Apple `FeatureSettingsViews` 里的那一节。随应用安装的词库带着一份清单，写明它是按哪套规格构建的、来自上游哪个提交；Apple 直接从 app bundle 里读，而把资源暂存进沙箱的宿主读不到，那个路径也不该告诉设置页。

只回两个字段。清单里还有 journal 模式、格式契约和全部第三方引用，没有一条回答页面在问的那个问题——"我装的是什么，它从哪来"。读不到就报错而不是猜：把词库版本说错，比不说更糟。

读的是暂存后的引擎资源而不是模块的 rawfile：暂存才是对着固定锁校验的那一步，键盘实际跑的那份副本才值得报告。

键盘的无障碍标签接上了。此前 `KeyboardView.ets` 里 `.accessibilityText()` 出现零次，而 `LetterKeyFacePolicy.accessibilityLabel`、`EnglishLetterCaseState.accessibilityLabel/accessibilityValue`、`JapaneseVariantPolicy.accessibilityLabel` 和 `CandidateGlossPolicy.accessibilitySuffix` 都已经从来源移植过来、都有单测、也都只有它们自己的测试在调用——标签算出来了，键盘一个字也不念。读屏用户面对的是一块什么都没有的键面。

键面文字越短越好，念出来往往不成词：`⇧` 是空的，`123` 是一个数，`中` 是一个字而不是一个动作。来源给每一个键都起了名字，而且名字说的是这个键会做什么，不是它显示什么。`KeyAccessibilityPolicy` 补上那四份策略没有覆盖的名字，视图现在把它们全部挂上。

空格键按它将要做的事来念：组合中它选定高亮候选，看不见候选栏的用户没有别的途径知道这个键换了含义。候选念成「候选词 N：X」，还没打完的拼音接在后面念成「还需输入 …」——这是"现在就能上屏的候选"和"还得继续打的候选"之间的区别；释义后缀走已移植的 gloss 策略，它已经决定了那是念「提示」还是念「英文释义」，那个区分不该在调用点重新推一遍。

反馈页附带的系统版本由本宿主填入。`os_version` 不属于 `HostCapabilities::for_platform` —— 它不是平台假设而是关于这台机器的事实，所以每个宿主自己读了再加上去；此前只有 macOS 这么做。缺它的时候页面写的是「平台：harmony」并附上 WebView 的 User-Agent，那标识的是浏览器内核，不是复现问题所需要的系统。现在从 `deviceInfo.osFullName` 取：`OpenHarmony-6.0.1.112` 去掉产品名后是 `6.0.1.112`，页面在它前面自己会写平台名，于是读作「HarmonyOS 6.0.1.112」。取不到合规的版本号就不填——这条字符串进的是用户提交的报告，要么照系统说的写，要么什么都不写。

候选翻译复用共享 `translation_query` 与 `apply_translations` 代际契约。Harmony 原生边界负责把 Tencent TMT、NiuTrans 和 DeepLX 兼容自定义 provider 的签名/请求描述器及响应解析暴露给 ArkTS，网络传输仍由 Harmony HTTPS 栈完成；本地英文词典释义先在 Engine 侧解析，在线结果只补齐缺失项。多语言释义合并为有界的 ` / ` 展示文本，按 provider、目标语言和词条缓存，过期或 generation 不匹配的结果不会污染当前候选页。英文目标的成功释义通过共享 ABI 写入用户词库覆盖层，凭据只存在于当前请求内，不写日志。

共享设置中的“流式预编辑”现在也由 Harmony 消费：关闭时识别中的临时结果不进面板，只有最终结果才显示；此前无论该开关如何，临时结果一律显示。“结果提交策略”反过来从 Harmony 的设置页移除——Windows 在 TSF / SendInput / 粘贴之间选，macOS 在系统事件与输入会话之间选，而键盘扩展只有输入客户端一条提交路径（Linux 同样只经 IBus / Fcitx5 提交，也不提供该选项），三选一在这里是一个只有一种结果的控件。该判断由 `HostCapabilities::voice_commit_mode` 决定。

录音行为的四个共享开关现在也由 Harmony 消费——设置页的说明一直写着"录音期间的提示音与静音由输入法在本机处理"，而此前本宿主一项都不做。开始与结束提示音使用 Windows 安装包同一份 `start.mp3` / `end.mp3`（同样的字节，放进模块的 `rawfile/audios/`），经 AVPlayer 播放，播放器随每次提示音创建并释放：键盘扩展不是媒体应用，为一段不到一秒的声音常驻一条音频管线不值得。`sound_enabled` 是两个提示音之上的总开关。"录音时静音其他音频"通过 `AudioSessionManager.activateAudioSession` 以 `CONCURRENCY_PAUSE_OTHERS` 实现，录音结束或取消时 `deactivateAudioSession` 归还；该项默认关闭，从正在播放的应用手里拿走音频会话是侵入性的。取消的录音不播结束音——没有识别结果可宣告。以上任何一步失败都只记日志，不影响录音本身。

语音回复的解释逻辑抽成了 `VoiceResponsePolicy`。识别器本身握着麦克风、套接字和会话代次，没有凭据和真实音频就跑不起来；但"这条回复是什么意思"不需要两者，而且恰恰是最容易写错的部分——豆包的错误码可能出现在顶层也可能在 `payload_msg` 里，文字同样两处都可能，而没有文字的最终帧仍然必须结束录音，当成"什么都没发生"会让麦克风一直开着。这部分用合成回复逐条覆盖。

2in1 硬件键盘补齐 Windows 的五个语音快捷键，各自受共享 `voice_input.hotkey_*` 开关控制：右 Alt 长按录音、Ctrl+Win 与 Ctrl+右 Alt 两个长按和弦、录音中按空格锁定（松开长按键不再结束）、Ctrl+F9 开始/停止（也用于结束已锁定的录音），录音中按 Esc 取消。设置页一直显示这五个开关，此前本宿主一个也不消费。空格与 Esc 只在录音时被占用，其余时刻仍归组合输入；长按键的重复按下不算第二次请求。录音状态由绘制识别面板的视图告知会话，识别自行结束（拿到最终结果或 provider 失败）时会清掉长按与锁定，否则下一次按下长按键会被当成一次并不存在的录音的释放。

共享设置中的“顶部语音入口”现在也会驱动 Harmony 触屏键盘：开启后，快捷栏会显示麦克风入口并直接打开系统识别；`voice_input.enabled` 关闭时，顶部入口隐藏，功能面板的语音图块保留在原位但变灰（0.4 不透明度）且不响应点按，保持平台特性与 Windows 的可选语音开关一致，同时不让图块网格因一个开关而重排。

共享设置中的“语音面板主题”也由 Harmony 消费：`dark`/`light` 覆盖全局主题，`follow` 继承全局主题；语音面板使用全局主题 keyboard 调色板的对应明暗一套。

共享设置中的“表情面板主题”和“手写面板主题”也由 Harmony 消费：各自的 `dark`/`light` 覆盖全局主题，`follow` 继承全局主题；两个面板分别使用全局主题 keyboard 调色板的对应明暗一套。

共享设置中的“工具栏主题”也由 Harmony 消费：`dark`/`light` 覆盖全局主题，`follow` 继承全局主题；浮动工具栏按主题约定从 candidate 调色板派生（底色取 surface、按钮文字取 text、悬停取 hover、描边与分隔线取 border、拖动柄取 secondary；工具栏按钮以按钮面上的字表示开关状态，没有单独的开启态填色），只有主题在该明暗下带 `candidate_skin` 时才再叠加那个皮肤包的安全 CSS。

共享设置中的“菜单主题”（`menu_theme`）由 2in1 候选右键菜单消费：`dark`/`light` 覆盖全局主题，`follow` 继承；菜单同样从 candidate 调色板派生（surface 底、text 字、accent 动作、border 描边），而不是键盘调色板。触摸键盘上的长按条属于键盘本身，继续用键盘调色板。

全局主题（system / shuishan / light / paper / night / ink / custom）经 ABI 3 的 `msime_client_resolve_theme` 与主题目录接入：键盘按主题的 keyboard 调色板着色（字母键、功能键、secondary 文字各有其色，回车键始终是平台 accent 白字），2in1 候选窗与 STATUS_BAR 工具栏使用 candidate 调色板，主题带固定明暗时所有面都随之固定。`global_theme` 为 `custom` 时，`custom_theme.keyboard` 若带一份设计，键盘读取它的颜色、圆角、边框、透明度和键面字体属性；否则用 `custom_theme.base` 所指主题的 keyboard 调色板。原生主题选择器会展示同一份设计，避免设置页保存了设计但键盘仍绘制默认配色。2in1 候选窗的次要文字（译文、注释、编码提示和序号）一律用调色板自带透明度的 secondary，不再叠加额外的不透明度；组字行末尾显示「当前页 / 总页」与可点的 ‹ ›，组字行关闭时它随之隐藏。输入模式提示（中/英徽章）用主题的 accent 与 on_accent 着色。触摸候选栏空闲时是一条横栏：34vp 标志按钮（功能面板打开时填平台 accentSoft）、中/英、方案、译、标点四个胶囊（译是候选译文开关，开启时字用 accent 色、关闭时用 secondary），以及表情、语音、收起键盘和打开设置；输入时候选不编号、无底色，选中项以 accent 与 600 字重区分。功能面板为每行四格的 52vp 图块，开启态是 accentSoft 底加 accent 字形；前八格按设计排列为拼、译、中/英、全、主题、标点、模糊音、设置，原有的繁体、表情及其余工具跟在后面，滚动可达。译与模糊音写的是设置页同一份 `candidate_translations` 与 `fuzzy_pinyin.enabled`：切换译文后键盘按新值重算释义行并调整高度；模糊音先存盘再交给会话，这样首次开启时由偏好存储补上的全部规则会立即生效，关闭时已选规则原样保留。

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

`build-native.sh` 在本机已跑通（2026-09-20）：`arm64-v8a`（真机）、`x86_64`（模拟器）和 `armeabi-v7a` 三个 ABI 的 Rust/C++/NAPI 交叉构建全部成功，各产出 `libmsimeclient.so`、`libmsime_host_api.so` 和 `libc++_shared.so`；`hvigorw assembleHap` 打出的 HAP 约 25 MB，包含全部三个 `libs/<abi>/`。`msime-client-core` 对 `aarch64-unknown-linux-ohos` 的 `cargo check` 亦通过。

Engine 换成纯 Rust crate 之后（2026-09-30），同一脚本在三个 ABI 上重新跑通，除 NDK 之外不再需要任何准备：以前要手工编译的 `libsqlite3.a` 前缀和 32 位 `armeabi-v7a` 需要的 fmt/spdlog 替身 config 都随 C++ Engine 一起去掉了，`libmsime_host_api.so` 的动态依赖只剩 `libc.so`。这一轮没有重新打 HAP，也没有上设备。

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

## 展开候选面板里没有释义，长按也没有反应

上一条把释义接到了候选条的长按菜单上。来源还有两条与之配套：`TheExpandedPanelDrawsTheSameGlossesAsTheStrip` 和 `TheExpandedPanelAnswersALongPressToo`——候选多到要展开时，那一页同样画释义、同样答应长按。

这边两条都不成立，而且是同一个原因：`allCandidates()` 从原生拿到完整候选记录（`source`、`fixed_position`、`annotation`、`translation` 都在），却只保留了 `text`。于是展开面板列着一串词，旁边的候选条却给每个词写着释义；长按也无从谈起，因为没有可读的东西。现在它返回 `ExpandedCandidate`——文字加释义加「这条释义是不是译文」。

**展开面板只给释义，不给词条管理。** 这不是照抄来源的取舍，是这边的下标空间决定的：管理操作走的是候选条那一页的位置，而展开面板经 `selectAnyCandidate` 用的是整份列表里的位置，两者是不同的原生入口。在这里放管理项，就是拿一份列表的编号去索引另一份。来源的展开面板同样只答应释义菜单，两边因此落在同一处。

`ExpandedCandidate` 刻意比 `CandidateEntry` 窄，只有面板用得上的三个字段：把 `fixedPosition` 和 `source` 一起带过来，正是在邀请上面那种串号。

顺带修掉换行估宽里的一处：原来按 `candidate.length` 算列数，那是 UTF-16 单元数，一个 emoji 会被当成两列。现在按码点数。

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

验证看的是打出来的包而不是源文件——`$media:` 解析不到时会被丢掉而不是报错。解包 `module.json` 后两个元素都带着 `"iconId": 16777217`，说明引用真的解析到了资源。`scripts/test-harmony-manifest.py` 现在查这三件事：模块级重复键、mainElement 有没有 icon、桌面入口有没有 icon，外加包里 `icon` 有没有对应的 `iconId`。对着改之前的清单跑，三条全报。

## 日语数字层的第十二格：从死键变成括号键

假名格的第十二格是 `小゛゜`（把刚输入的假名变成小写／浊音／半浊音）。切到数字层之后键不再产生假名，这个后置修饰符就没有东西可修饰——`JapaneseVariantPolicy.enabled` 里那个 `!symbols` 让它在整个数字层恒为停用，于是那一格是死的。来源把这个空出来的格子给了括号：`setDigits(true)` 时键面变 `（）`、读作 `括弧`、始终可用，按下给出八个括号。这些括号在这套布局上没有别的落点。

`JapaneseNineKeyLayout.digitBrackets()` 早就移植好、也有测试，只是没有调用方。现在接上了：`JapaneseVariantPolicy.brackets(japaneseNineKey, symbols)` 决定这一格当下是哪一种键，两种职责互不重叠（数字层上 `enabled` 恒 false，假名层上 `brackets` 恒 false），所以一个键可以兼任。

括号走的是条状浮层而不是菜单——`bindContextMenu` 在输入法面板里不显示，这台宿主上每一处长按列表都是这么画的，快捷标点和候选词管理共用同一条。三者互斥，打开一个会关掉另外两个。

## 在设置页关掉当前方案，键盘要能离开它

共享设置页管着「哪些输入方案出现在选择器里」。关掉键盘正在用的那一个，它从选择器里消失，而键盘照旧用着它——于是既看不见也退不出，除非随便挑另一个。来源在写入启用列表的那一刻就把当前方案归一（`DisabledSchemesAreHiddenAndCurrentSchemeFallsBack`：应用中的日语遇上只启用 `[全拼9键, 五笔]`，存下来的方案立即变成全拼9键；清空则回落到全拼）。

这里不能在写入时做，页面和键盘是两个进程，页面写的是文档而不是一个正在运行的键盘。所以改在 attach 时读：`KeyboardScheme.resolveEnabledSelection` 按「共享选择 → 已应用 → 第一个启用项」定出该用哪一个，`mappingForRuntimeSelection` 决定这算不算一次移动（原地不动时返回 null），移动了才把 `scheme`／`last_chinese_scheme`／`shuangpin_profile`／`touch_keyboard_layout` 一起写回并推给 Engine。两个都是早就移植好、此前没有调用方的。

`touch_keyboard_schemes` 整体进了设置记录：`scheme` 和 `touch_keyboard_layout` 是 Engine 的视角，它们解析出什么与选择器还提不提供它无关。

高情商回复是工具而不是输入方案：`KeyboardScheme.SCHEMES` 和方案选择器里没有它，快捷栏上的 💬（`reply`）在任何方案下都在，点一下打开回复面板（`SURFACE_REPLY`），再点一次或点面板里的「完成」回到原来的键盘。旧文档里存的 `thoughtful_reply` 不是一个已知的方案 id，按未知 id 处理：`enabledFromPreferenceIds` 把它从启用列表里去掉（只剩它时回落到全拼 26 键），`resolveEnabledSelection` 把存着它的选择落到第一个启用项。插入的回复按 `reply` 来源记入打字统计，与当前方案无关。

单测覆盖了来源断言的那两种情形（已应用项不在启用列表、启用列表被清空）和写回的三种判定。

## 快捷栏的按钮此前对读屏全是哑的

候选行下面那条工具栏原本整条是图标，没有一个挂了 `accessibilityText`；`KeyAccessibilityPolicy` 里当时只有 `tools`、`skin`、`scheme` 三个名字（移植好、没人调用）。现在快捷栏上每个控件都有名字，从左到右：标志 `更多快捷设置`（`tools`，打开功能面板）、中/英胶囊（`languageState`，读「切换中英文，当前中文」）、方案胶囊 `选择输入方案`（`scheme`）、译胶囊（`tile(translations(), …)`，读「显示译文，已开启／已关闭」）、标点胶囊（`punctuationState`，读当前是中文还是英文标点）、`表情与符号`（`emoji`）、`语音输入`（`voice`，仅在开启语音快捷键且语音可用时出现）、`生成高情商回复`（`reply`，任何方案下都在）、`收起键盘`（`dismiss`）、⚙ `打开设置`（`settings`）。`选择主题`（`theme`，原 `skin` 的「切换皮肤」，皮肤并入全局主题后改名）和 `键盘大小与间距`（`geometry`）已不在快捷栏上，而是功能面板里的图块；面板的每个图块都经 `KeyAccessibilityPolicy.tile` 读名字，开关类图块（如「中文标点」）再读出已开启或已关闭。`shortcutIcon`/`shortcutText`/`stripPill` 的 label 参数不是可选的——按键至少还画着一个字符，这些只画图标或一两个字。

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
- 硬件键盘：组字时韩文键盘的汉字键（`KEYCODE_HANJA` = 2614，即 `Lang2`）或不带修饰键的 F9 触发，按住只触发一次；无论 Engine 是否处理都吞掉这个键，空闲时交还应用。列表开着时 `routeKorean` 先让给 `routeHanjaList`：空格、回车、小键盘回车选高亮项，1–9 选本页（共享偏好 `number_row_selection` 关闭时仍是提交韩文再打数字），上下键、翻页键、Tab 按导航偏好翻页和移动高亮，左右键在方向键导航打开时移动高亮；关掉的绑定和 Home/End 保持 Korean 原有含义。`- = [ ] , .` 仍是标点，由 Engine 关掉列表并把韩文和标点一起提交；退格、Esc 只关列表、保留音节；字母关掉列表后照常组字；切换方案、失焦和编辑器自己的改动提交的是韩文。

列表没打开时，所有按键与点按的行为与上文完全一致。以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备上验证。

## 粤语、注音与越南语

选择器末尾在「韩语 26 键」之后再加三张卡：「粤语」（`KeyboardScheme.CANTONESE`，Engine 方案名 `cantonese`，编号 5）、「注音」（`ZHUYIN`，`zhuyin`，编号 6）和「越南语」（`VIETNAMESE`，`vietnamese`，编号 7）。三者默认都不启用（`DEFAULT_ENABLED` 不含它们），由用户在设置页打开。粤语和注音是中文方案：选中时它们自己就是 `last_chinese_scheme`，「中文」回到它们，打字统计记在 `cantonese`、`zhuyin` 名下；越南语和日语、韩语一样不是中文方案，选中时保留原来的 `last_chinese_scheme`，统计记在 `vietnamese` 名下。三者都不学进主词库，所以候选长按没有置顶、降权、删除；简繁转换开关对它们不起作用（粤语与注音本来就是繁体）。快捷栏和语言键上粤语、注音显示「中」，越南语显示「越」。

方案之间的差异不再逐处写方案名，而是集中在 `SchemeTraits.ts`：它按 Engine 的方案编号逐条镜像 `crates/engine/src/types.rs` 里 `SchemeType` 的谓词（`is_chinese`、`uses_chinese_punctuation`、`commits_on_blur`、`locks_caret`、`has_openable_candidate_list` 等），每个谓词写成“对哪些编号成立”的列表，`scripts/test-scheme-traits-parity.py` 读这个文件，常量编号、`NAMES` 或任一谓词与 Engine 不一致时失败。`input/SchemeCompositionPolicy.ts` 是原 Korean 专用组字策略的推广：韩语、注音、越南语的组字行画 Engine 的 `preedit`（写出来的字，而不是按键），光标固定在末尾；韩语和注音的候选只在用户打开的列表里出现，是否打开以视图的 `candidate_list_open` 为准。英文模式和本地工具模式下这些规则都不生效。

### 词库与暂存

粤语、注音和笔画各需一份语言词库（`msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db`，由 `scripts/fetch_language_dictionaries.py` 取回或 `msime-dict-build languages` 生成），越南语不需要。`stage-resources.sh` 的第三个参数（默认 `target/language-dictionaries`）指向这些文件，每份词库只在其许可证文本（`msime-rime_cantonese_LICENSE.txt`、`msime-libchewing_data_LICENSE.txt`、`msime-rime_stroke_LICENSE.txt`）同在时才暂存到 `resfile/language-dictionaries/`，缺许可证直接失败；一份都没有时只打印提示，设 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1` 则要求 `resources/language-dictionaries.lock.json` 固定的每一份都在（`fetch_language_dictionaries.py --list-databases`）；锁固定 `msime-stroke.db` 之前，笔画词库存在就暂存，缺少也不让发版失败。键盘与设置页启动时用 `StagedResources.stageLanguageDictionaries` 把它们复制到 `files/language-dictionaries/`，与 `files/engine` 相邻，host-api 在那里找到并写进运行时选项；新包不带词库时删掉旧副本。词库缺失的方案不出现：设置页的 `hostCapabilities` 从 `input_schemes` 里去掉它（与桌面端 `drop_uninstalled_language_schemes` 一致），键盘的方案列表经 `KeyboardScheme.withInstalledDictionaries` 过滤，全部被过滤时回落到全拼。

### 注音（大千）

触屏换成 `input/ZhuyinLayout.ts` 描述的大千键面：数字行加 `-`，三排字母各自补上 `; , . /`，共 41 键四行，键帽画注音符号或声调（3 ˇ、4 ˋ、6 ˊ、7 ˙，读屏念作「三声」等），四行高度与其他键面三排字母加行距相同，切换方案时键盘高度不变。点击发出 ASCII 键，由 Engine 的大千编辑器组字，空格是一声。组字行末尾的按钮在注音下写「選」：点一下打开候选列表，再点关闭（读屏为「选字」/「关闭候选列表」）。列表开着时 Engine 把数字 1–9 当选号，而触屏在候选条上选字，所以点数字键会先关掉列表再按键，打出的是键帽上的符号。回车沿用共享规则：列表开着提交高亮行，否则提交整段转换结果。符号键面上大千要占用的键（`1234567890,./;-`）由键盘先提交转换结果再直接打出标点（中文标点开关打开时打中文标点），其余符号照常走标点路线。

硬件键盘走 `HardwareKeyRouter.routeZhuyin`，以 Engine 给出的 `spelling_symbols` 为准：空闲时能起音节的数字（`1 2 5 8 9 0`）直接组字，`, . / ; -` 经标点路线交回 Engine 组字，声调数字等其余键落回中文路线；组字时 Engine 列出的数字与标点都组字；下方向键打开列表；列表开着时交给 `routeHanjaList`，空格、回车选高亮，1–9 选本页，方向、翻页、Tab 按导航偏好移动；Shift+数字等标点总是标点；左右键、Home/End、Delete 和 Ctrl+退格/左/右先提交再交还应用（转换结果里没有光标）。

### 越南语（Telex / VNI）

触屏用普通 26 键，字母按实际大小写发出，Shift 只作用一次，不做自动大写；符号、逗号键与快捷标点都是半角 ASCII，第二排没有 `;` 键。组字行画 Engine 写出的带声调词。符号键面上的数字只在 VNI 且正在组字时作为声调键交给 Engine（Engine 此时把 `0123456789` 列为 `spelling_symbols`），否则先提交词再打数字；空格提交词并打出空格；回车先提交词，再照常换行或提交编辑框。失焦与切换方案提交当前词。硬件键盘走 `routeKorean` 的同一条路（不认汉字键）：字母总是组字，空闲时其余键交还应用，组字时 VNI 数字组字，标点连词一起提交，其他键先提交再交还应用。


### 藏文（EWTS 威利转写）

选择器在「越南语 26 键」之后再加一张「藏文 26 键」卡（`KeyboardScheme.TIBETAN`，偏好 id 与 Engine 方案名都是 `tibetan`，编号 8），和越南语一样默认不启用、不需要词库，由用户在设置页打开。藏文不是中文方案：选中时保留原来的 `last_chinese_scheme`，打字统计记在 `tibetan` 名下，不计入中文；不学进主词库，简繁转换、中文标点和全角都不作用于它。账号同步上传时和粤语、注音、越南语一样不写 `input.schema`。快捷栏和语言键显示「藏」。`SchemeTraits.TIBETAN` 的谓词与越南语相同：失焦与切换方案提交组字（`commits_on_blur`）、光标固定在末尾（`locks_caret`）、第一次取消保留组字（`cancel_keeps_composition`）。

输入是在拉丁字母键盘上打 EWTS（扩展威利转写），由 Engine 转成藏文：组字保存当前音节串的威利原文，组字行画 Engine 写出的藏文（`preedit`），没有候选列表。威利转写区分大小写（`T D N Sh A I U M H` 等是不同的字母），所以触屏字母按 Shift 给出的大小写发出，硬件键盘的 Caps Lock 也当作大写，和越南语一样不做自动大写；第二排没有 `;` 键，符号键面、逗号键与快捷标点都是半角 ASCII。触屏空格交给 Engine：组字时上屏藏文加音节点（U+0F0B），没有组字时 Engine 不处理，键盘照常打出空格（`KeyboardSession.pressThenType`）；回车走共享规则，组字时提交藏文（不加音节点）并吞掉回车，没有组字时照常换行或提交编辑框。符号键面上的 `/` `'` `+` `.` `-` 走标点路线（`+` 是藏文方案下第三排替换 `=` 的键，`KeyboardScheme.symbolRowKey`；符号面板直接写入编辑框，不能用来叠写），运行时按 Engine 的 `spelling_symbols`（空闲时 `'/`，组字时 `'+-./`）把它们作为字符交给 Engine：`/` 组字时上屏藏文加垂符（U+0F0D），空闲时单独上屏垂符；其余是拼写符号。数字先提交音节串再打数字本身，不转成藏文数字。

硬件键盘走 `routeKorean` 的同一条路（`tibetan` 为 true，不认汉字键）：字母总是组字；Engine 列出的拼写符号空闲时也组字（`'` 开头 achung 音节，`/` 单独上屏垂符），其余键空闲时交还应用；组字时空格经 `PRESS_THEN_TYPE` 交给 Engine 上屏藏文加音节点，回车发 `MSIME_COMMIT_RAW` 只上屏藏文，两者都不再交给应用；退格删一个原文按键，Esc 第一次退回威利原文、第二次丢弃；其他标点连音节串一起提交，数字、Tab、方向键等先提交再交还应用。
以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备或模拟器上验证。

## 笔画

选择器末尾在「藏文 26 键」之后再加一张卡「笔画」（`KeyboardScheme.STROKE`，Engine 方案名 `stroke`，编号 9，字形「笔」，角标「5」），默认不启用，需要 `msime-stroke.db`（见上文「词库与暂存」），缺词库时和粤语、注音一样不出现。笔画是中文方案：选中时它自己就是 `last_chinese_scheme`，打字统计记在 `stroke` 名下；`SchemeTraits` 里它的谓词逐项照抄粤语（中文标点、智能标点、全角加宽成立，不学进主词库、不做简繁转换、失焦不提交、光标不锁定、没有可开关的候选列表），由 `scripts/test-scheme-traits-parity.py` 对照 Engine 检查。账号同步不上传它（`AccountPreferencePlan` 的 `LOCAL_ONLY_SCHEMES`），云端写来的 `stroke` 保留本机方案。

触屏画 `input/StrokeLayout.ts` 描述的笔画键盘，套用九键的外框：左侧是九键的标点栏，中间两行三列 `一 横`、`丨 竖`、`丿 撇` / `丶 点`、`乛 折`、`＊ 通配`，右侧整列是删除键，底排与九键相同（「符」代替逗号）。卡片的布局仍记作 `twenty_six_key`，键面按方案选（与大千注音同理），所以不会打开九键拼音的数字解码；符号层沿用共用的字母面。点击发出字母 `h s p n z x`，Engine 负责组字与候选；空组合时 Engine 不接通配键，键盘也就不发送它。组字行和 2in1 的预览文本画 Engine 的 `preedit`，也就是笔画字形 一丨丿丶乛＊，不画键入的字母，光标沿用 Engine 的位置（字母与字形一一对应）。空格、退格和失焦沿用粤语的共享规则；回车不同：粤语有候选时回车上屏高亮候选，笔画的触屏回车与硬件回车、iOS、Android 一致，总是上屏键入的字母（`ReturnKeyAction` 的 `COMMIT_RAW`，即 MSIME_COMMIT_RAW），选字用空格。快捷栏的输入模式指示显示「笔」，触屏语言键仍显示「中」。

硬件键盘走中文的共享路线：字母交给 Engine，空组合时只有 `h s p n z` 开始组字，`x` 与其它字母由 Engine 交回应用照常输入（键盘不把这当作故障记日志）；组字中 `x` 追加通配，其它字母被吞掉；数字 1–9 选本页；`'` 不当音节分隔符（`HardwareKeyRouter.spellsWithoutSyllables`，与五笔相同）。

以上由 `tests/run.sh` 的逻辑测试和 `hvigorw assembleHap` 的 ArkTS 编译覆盖，尚未在设备或模拟器上验证。

## 2026-09-21：首次在模拟器上跑起来

在 API 21 的 `Mate 70 Pro` arm64 模拟器（DevEco 自带镜像，`hdc` 连 `127.0.0.1:5555`）上完成了一次装机运行，实测到的东西比之前所有交叉构建加起来都多。

**先是构建根本过不去。** `hvigorw assembleHap` 报 13 个 ArkTS 错误，分布在三个 `.ets` 文件里，全部来自最近合并的几片。`tsc` 全过、单测全绿、设置包构建成功、`verify-local.sh --quick` 通过——没有任何一道门禁编译过 ArkTS，所以 `develop` 处在打不出 HAP 的状态而没人知道。ArkTS 是 TypeScript 的一个严格子集：不认 `unknown` 和 `any`、对象字面量必须对应已声明的类或接口、不支持索引访问类型。`scripts/test-harmony-arkts-subset.py` 现在在 `--quick` 里查前两条（纯语法、零误报）；第三条依赖类型信息——ArkTS 接受 `JSON.stringify({ ok: false, error: x })` 却拒绝里面再嵌一层字面量的同一个调用——纯文本判断要么漏要么误报两百条，两种都试过了，所以那一条明写为只有真编译器能抓。

装机后确认的：

- 系统识别并接受本输入法：`ime -e app.msime.client` 成功，`ime -s` 之后 `ime -g` 返回 `app.msime.client`，`app.msime.client:inputMethod` 进程被系统输入法框架拉起。
- 共享 React 设置界面在设备上正常渲染：欢迎流程、首页（含底部导航与键盘预览）、词库页。
- 新增 C ABI 的整条链路通了。词库页显示「规格 desktop」「词库版本 e92a9c7c64e2」，与 `resources/desktop-dictionary.lock.json` 的 `source_commit` 前十二位一致——从 `msime_client_dictionary_manifest` 经 NAPI、ArkTS 桥、`registerJavaScriptProxy` 到共享 React 卡片，每一跳都真的走通了。
- 引擎资源暂存正常：`staged /data/storage/el2/base/haps/entry/files/engine`。第一次跑打出 `no packaged resources at /engine` 是因为漏了 `stage-resources.sh`，不是代码问题；补上 180 MB 的已验证词库后即正常。

**键盘作为系统输入法在模拟器上完整跑通了。** 在社区页的搜索框里打 `nihao`，组合行显示 `nihao`，候选栏给出 `1 你好`，点选后 `你好` 进入输入框——按键经 ArkTS、NAPI、`crates/host-api` 的 C ABI、`input-runtime`、`engine-bridge` 一路到 C++ Engine 并带着随包词库返回，整条链路真的走通（这是当时的链路；C++ Engine 与 `engine-bridge` 已由 `crates/engine` 取代，`input-runtime` 现在直接调用它）。回车键读的是编辑器自己的动作：搜索框上显示「前往」，组合进行中变成「选定」，组合结束又变回「前往」。

**这里有一个必须写下来的操作事实：输入法要在 `FULL_EXPERIENCE_MODE` 下才会被框架驱动。** `ime -e <bundle>` 的默认是 `-b`，也就是 `BASIC_MODE`；在那个模式下 `ime -s` 会成功、`ime -g` 会报告本输入法是当前输入法、`app.msime.client:inputMethod` 进程也会起来，但编辑器获得焦点时框架打的是 `ShowKeyboardImplWithoutLock, panel not create` 与 `OnInputStart, entry is nullptr`，而本扩展的 ArkTS 一行日志都没有——`onCreate` 从未运行。表现就是键盘完全不出现，且看不出任何错误。换成 `ime -e <bundle> -f` 之后，同一次点击立刻打出 `attached to editor: pattern=0 enter=2`，面板正常呈现。做过一次对照：同一个输入框、同一次点击，华为系统输入法在我们处于 BASIC_MODE 时照常弹出，所以这不是模拟器、WebView 或该字段的问题。

键盘在第三方应用里同样可用：华为浏览器的搜索框上打 `nihao` 得到候选 `你好`，上屏后浏览器据此拉取了联想词，说明文字确实到达了那个编辑器。回车键在那里读作「搜索」而在本应用的搜索框读作「前往」，两次都取自编辑器自己声明的动作。

那一轮里社区页显示的是离线预览数据，因为当时没有登录账号；账号、社区与 AI 服务的真实往返需要一个已登录的账号会话，麦克风相关路径需要用户授予 `ohos.permission.MICROPHONE`。个人词库队列的 4/4/1 批处理、回执不重放、Engine 规范化、空闲续排和会话状态恢复由逻辑套件覆盖。

按键音与振动现在也能从设置页调整，而不只是键盘内那张卡片：共享 `mobileKeyboardFeedback` 客户端读写键盘自己的 `key-feedback.json`，两个进程共用同一份文件（这项设置属于当前设备而非账号，所以不进共享偏好）。设置页是第二个写入者，改动在键盘下次启动时生效。强度预览直接振一下。共享 DTO 把最强一档叫 `strong`，键盘自己的枚举叫 `heavy`，两边由 `KeyboardFeedbackBridge` 转换——直接赋值会写入键盘不认识的值，`KeyboardFeedback.parse` 会静默回退，表现为"保存了但手感没变"。

设置页的字体输入现在能列出系统已装字体：`listFontFamilies` 桥接 ArkUI 的 `font.getSystemFontList()`，宿主只过滤掉带控制字符的名字，去重和排序留给共享页面，以免各宿主给出不同顺序的同一份列表。

共享设置中的候选窗英文字体（`candidate_english_font`）现在也由 Harmony 消费：该名字排在中文字体之前进入 ArkUI 的字体族列表，由渲染器逐字形回退，因此拉丁字母取自英文字体、汉字落到中文字体。未设置时仍是单一字体族，与此前行为相同。该控件此前在共享设置页被一串平台名挡住，Harmony 不在其中——宿主读了这个字段而用户改不到它；现在改由 `HostCapabilities::candidate_english_font` 决定。

共享设置中的录音设备选择现在也由 Harmony 提供：设置页通过 `AudioRoutingManager` 列出输入设备，保存的是设备类型加地址组成的稳定标识，而不是每次会话重新分配的 `id`。HTTP ASR 与豆包两条路径各自创建 `AudioCapturer`，因此在建流前用 `AudioSessionManager.selectMediaInputDevice` 指定所选设备；路由属于音频会话而非单条流，所以每次录音都重新指定一次。所选设备被拔掉、路由被系统拒绝、或保存的 backend 属于别的平台时，都回到系统默认设备继续录音，不中断，也不把 Windows 端点标识或 PulseAudio source 名当成 Harmony 设备重新解释。系统语音识别（Core Speech Kit）由服务自行取音，不受该选择影响。

键盘自己的输入方案选择器现在和共享设置页写同一组字段：选中日语时把被替换的中文方案记进 `last_chinese_scheme`，`scheme`、`shuangpin_profile`、`touch_keyboard_layout` 一起更新。此前只写 `scheme`，于是从键盘切到日语后，设置页的「中文」单选只能退回 quanpin，五笔或双拼用户会看到方案被忘掉。当前值按磁盘上的文档读取，设置页是第二个写入者；随后的写入仍做 revision 比对，真正的冲突照样被拒绝。

共享设置中的“中英文切换提示”现在也由 Harmony 消费，并改由 `input_mode_hud` 宿主能力而非平台名决定是否出现在设置页。2in1 上模式徽标只在该偏好开启且没有悬浮工具栏时创建；关闭后不再占用那一个 STATUS_BAR 面板名额。手机形态本来就在键面上显示模式，不声明该能力。

候选的两项可选注释现在各读各的共享偏好，不再一律显示：`wubi_code_hint` 控制五笔剩余编码提示，共享默认开启，只有文档明确写 `false` 才隐藏；`candidate_english_gloss` 控制离线英文释义，共享默认关闭，只有文档明确写 `true` 才显示。Engine 注释仍优先占用同一个提示槽位。

2in1 的按键音、打字旋律、上屏音和成就音效读共享偏好的 `plugins` 段，与桌面三端同一份设置、同一套音效包，只是播放器不同：host-api 在 HarmonyOS 上不链接音频栈，所以由 `KeySoundPlayer.ets` 用 SoundPool 播放。包里有哪些文件由 NAPI `keySoundPack` 调 `msime_client_key_sound_pack` 取得，校验只有 client-core 一份；哪个事件放哪个文件、旋律怎么走、停顿 3 秒从头开始，是 `KeySoundPolicy.ts` 照 host-api 播放器移植的规则，逻辑测试钉住。WAV 样本由 `native/key_sound_render.cpp` 用 miniaudio（与 Windows 宿主同一份单头文件）解码，先按头部声明的帧数查 1.5 秒上限、解码时再以声明长度为界，然后按旋律用到的每个音高各写一个 48 kHz 的 WAV 到 cacheDir，SoundPool 只解码本宿主写出的文件；变调按播放速率算，与桌面一致，升一个八度的音也短一半。Ogg 样本在 2in1 上不播放：本宿主不解码 Vorbis，交给 SoundPool 就会在媒体服务里整段解码，解码后的长度没有任何东西能限住，所以只有 WAV 样本出声（内置包全是 WAV）。SoundPool 用音乐流类型创建，系统对短音走混音而不打断正在播放的音乐，不走录音那套 `CONCURRENCY_PAUSE_OTHERS`。只在 2in1 上配置，手机形态保留自己的 `key-feedback.json`；密码框里和英文模式下不出声，与桌面三端一致。按键音在按键被处理之后触发，包括交还给应用的键；上屏音跟着 Engine 的每次提交；成就音效来自打字统计 `record` 应答里的 `milestone`，所以要打字统计开着才有。设置页在 2in1 上声明 `key_sound`、`music` 与 `plugin_triggers` 能力。

2in1 的背景音乐由 `MusicPlayer.ets` 用 AVPlayer 流式播放：曲目由 NAPI `musicPack` 调 `msime_client_music_pack` 取得（绝对路径与 `max_track_seconds`），校验同样只有 client-core 一份；何时放、按什么顺序是 `MusicPolicy.ts` 照 host-api 播放器 `tick_music` 移植的规则，逻辑测试钉住。音乐开关打开且选了包时，inputStart 之后等编辑框属性回来、确认不是密码框才开始放；inputStop（系统随之收起面板）、密码框获得焦点和录音期间暂停，焦点回到普通编辑框再继续。候选窗每打完一个词就收起一次，那不是输入法被收起，不暂停音乐。每首曲子开播前先用 AVMetadataExtractor 读容器声明的时长，超过 `max_track_seconds` 或读不出来就跳过；播放中再按 `timeUpdate` 的位置查同一个上限，超出就切下一首，与桌面解码器先查声明帧数、播放中再数帧数一致。曲目按清单顺序循环。AVPlayer 边读边解码，所以 Ogg 曲目在这里也能放，不像按键音样本那样只放 WAV。包读不出来、没有一首时长合格、或 AVPlayer 报错，音乐就关掉并在日志里留一行，直到音乐设置变化（换包、开关、音量）才重试；换包或开关会从头重新加载，只改音量直接调到正在放的曲子上。AVPlayer 的 `state` 要等 play/pause 执行完才变，所以同一时间只发一个 play 或 pause，播放器落定到 playing 或 paused 后再按会话此刻的状态对一次，`MusicTransport` 的逻辑测试钉住：快速切换焦点不会让音乐在普通编辑框里卡在暂停，也不会在密码框里响。播放用音乐流类型，是否打断别的应用正在放的音乐由系统的音频焦点策略决定；别的应用拿走焦点后，系统自己暂停的曲子不会马上被重新播放，被系统停掉的播放器直接释放，下次焦点回到可以放音乐的编辑框时重新打开当前曲目。手机形态拿到的永远是关着的设置，不创建任何播放器。

插件页的包管理与 @ 名单走 NAPI `plugins` 调 `msime_client_plugins`：列出已装和内置的包、删除、读写 @ 名单都在设置桥的同步方法 `plugins` 里完成，状态目录和内置音效包目录（`resourceDir/sound-packs`）由桥补上，页面不经手任何路径，规则和失败码与桌面三端的 Tauri 命令是同一份（client-core 的 `PluginFailure`），失败时连同 client-core 给出的具体原因一起显示。导入要等系统选择器，所以走 `startRequest` 的 `plugin_import`：用 `DocumentViewPicker` 选文件夹或 `.zip`，选中的文档 URI 本库打不开，于是先复制到 cacheDir 下的临时目录，再交给 `msime_client_plugins` 按同一套规则安装，临时副本无论成败都删掉。桌面的 client-core 直接读选中的文件夹或压缩包、边复制边按上限停下，这里的临时复制也照同一组上限先查再复制（`PluginImportPolicy.ts`，数值取自 `crates/client-core/src/plugins/import.rs`）：文件夹只看顶层，遇到子文件夹、符号链接、超过 16 个文件、单个文件超过 16 MiB 或合计超过 66 MiB 就不复制直接拒绝，压缩包超过 80 MiB 同样不复制，所以误选了「下载」这类大文件夹不会先整个复制进缓存再被拒绝；真正的规则校验仍只在 client-core。临时副本用固定名字（`pack`、`pack.zip`），不用选中的文件名，包 id 本来就读自 plugin.toml。安装要解压最多 80 MB、校验后换入，所以走 NAPI `pluginsAsync` 在工作线程上跑，不占 UI 线程；列表、删除和 @ 名单这些小读写仍走同步的 `plugins`。装进 `files/state/plugins` 的包和 `mentions.json`，键盘进程在下一次聚焦时就读到。

V、`/`、`@` 三个模式的按键由 Engine 导出的 `spelling_symbols` 决定：`HardwareKeyRouter` 在组合中遇到列在其中的字符就交给 Engine 拼写，否则 Shift+1..9 选词，原先只认 `local_mode === "unicode"` 的分支因此泛化到 V 模式的数字和运算符（Shift+9 是 `(` 不是选第九个；`-`、`.` 是运算符和小数点不是翻页；小键盘的点也是小数点）。`/`、`@` 在无组合时照常作为标点交给 runtime，由 runtime 按 `spelling_symbols` 改走 Engine 进入模式。这三个模式生成的上屏内容按 `commit_context.typing_statistics` 不计入打字统计。

## 产品版本

版本表 `shared/contracts/editions.json` 里每个版本的 HarmonyOS 段（`platforms.harmony`）目前都是 `null`：HarmonyOS 只有 `build-profile.json5` 里的 `default` 一个 product，也就是 full，`bundleName` 是 `app.msime.harmony`，HAP 不签名（`signingConfigs` 为空）。代码已经按版本参数化，full 的行为和产物与引入版本之前相同。

已经就位的部分：

- 版本身份：`entry/src/main/ets/keyboard/AppEdition.ts`，对应 Android 的 `AppEdition.java` 和 iOS 的 `MSIMEAppEdition`。`AppEdition.current()` 目前返回 full。
- 方案：`KeyboardScheme` 的启用列表、默认启用列表和各处回退（词典缺失、列表为空、偏好里认不出的方案）都回退到本版本的默认方案，本版本不提供的入口不算启用；手写不属于任何方案，写出的是汉字，所以只在提供中文方案的版本里有（full、拼音版、五笔版），日文、越南文和藏文版没有，写进偏好、交给 Engine 的是本版本的默认方案（`KeyboardScheme.engineSchemeOf`）；只有一个方案的版本没存过启用列表时，默认要用户自己打开的入口（越南语、藏文）也启用，与 client-core 的 `TouchKeyboardSchemePreferences::for_edition` 一致。`KeyboardSession`、`KeyboardView` 的初始方案同样取版本默认。非 full 版本调 `prepareHost` 时带上版本 id（`KeyboardSession.prepareRequest`），host-api 据此收窄方案、按本版本的资源锁校验词库。
- 设置同步：`AccountPreferencePlan` 的 `localAccountPreferences` 和 `applyAccountPreferences` 分别经 `filterUploadedAccountSettings`、`filterDownloadedAccountSettings` 过滤，规则与 client-core 的 `filter_uploaded_account_settings`、`filter_downloaded_account_settings` 相同：只有一个方案的版本既不上传也不应用 `input.schema`，随它一起的还有本机的 `platform.harmony.keyboard_layout`（方案加布局才决定是哪个入口）；多方案版本把本版本没有的方案当作缺失；不提供双拼、五笔的版本不上传对应的方案细项。
- 以上规则由 `tests/run.sh` 用拼音版、五笔版、日文版、越南文版和藏文版的声明覆盖。

要发一个版本（以五笔版为例）还差这些，本分支没有做：

1. 版本表：给 wubi、pinyin 填 `platforms.harmony` 段，至少包括 `bundleName`（例如 `app.msime.harmony.wubi`）和 HAP 文件名；同时扩展 `editions.schema.json`、冻结基线 `editions.frozen.json` 和 `scripts/test-editions.py` 的跨版本唯一性检查。不同的 `bundleName` 各有各的沙盒，`files/state`、`files/engine` 和偏好文档自然分开。
2. 工程：`build-profile.json5` 为每个版本加一个 product，覆盖 `bundleName`、应用名（水杉五笔、水杉拼音；图标与 full 相同）和输入法扩展在系统列表里显示的名字，并把版本 id、方案和默认方案作为构建参数写进去，再让 `AppEdition.current()` 读出来；`entry` 的 target 用 `applyToProducts` 关联到这些 product。多 product 下 hvigor 的这些参数我没有在本仓验证过。
3. 设置页：`Settings.ets` 的 `hostCapabilities` 取自 `msime_client_host_capabilities`，这个 C ABI 目前只接受平台名、不按版本收窄方案，共享设置页因此仍会列出 full 的全部方案。需要让它（或 Harmony 一侧）按版本调用 client-core 的 `HostCapabilities::narrow_to_edition`。
4. 资源：`stage-resources.sh` 目前按 full 的 `resources/desktop-dictionary.lock.json` 暂存，要改成按版本的 `resources/editions/<id>.lock.json`；五笔版不带日文词典，五笔版和拼音版都不带粤拼、注音、笔画词库。
5. 签名与发布：零售设备装不上不签名的 HAP。要在 AppGallery Connect 为每个 `bundleName` 建应用，申请发布证书和 profile，填进 `signingConfigs` 并按 product 关联；`release-harmony.yml` 现在只打一个 HAP、按 `find ... | head -1` 取产物，要改成按版本逐个构建，产物名带版本 id。
- `native/`：NAPI/C++ 适配层。
- `tests/`：不依赖设备的 TypeScript 键盘逻辑测试。
- `AppScope/`、`entry/src/main/resources/`：应用元数据和资源。
- `build-native.sh`、`stage-resources.sh`、`stage-voice-runtime.sh`：共享 Host API、NAPI 库、固定资源和 sherpa-onnx HAR 的构建/暂存入口。
- `entry/src/main/resources/rawfile/settings/index.html`：设置页的单文件打包产物，由 `stage-settings.sh` 从 `apps/harmony` 生成，不提交，见下方[设置页打包](#设置页打包)。

Windows 文档里的“自定义候选窗翻译”在 Harmony 上没有设置入口。Engine 仍在每个宿主上读这份覆盖层——`prepare_translation_sidecar` 先看用户数据目录（`<state>/user/custom_translations.txt`）再看资源目录——但应用沙盒里的这个文件用户无法直接放入，设置页也不再提供编辑它的「自定义候选释义」。**不要把它写进已暂存的资源目录**：那里按锁文件逐项精确校验，多一个文件就会让键盘拒绝启动。

设置页的本地词库管理复用共享设置 UI 和 `msime_client_dictionary`：可分页查看、编辑、导入、导出和处理失败队列。ArkTS 设置桥只接受操作 JSON；引擎资源和状态目录始终由宿主从应用沙盒准备，WebView 不能提交路径。词库写操作需要 Engine 独占维护窗口：空闲时会短暂重建会话并恢复语言、九键和焦点状态；正在组合输入时会返回忙碌错误，不会替用户取消输入。读取操作可与活动会话并行。

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
# 使用 DevEco SDK 配套且已加入 PATH 的 hvigorw
ohpm install
hvigorw assembleHap
```

`stage-resources.sh` 还把仓库自带的六套辅助码表（`resources/helpcodes`，不在词库发布里）连同来源声明放进 `resfile/engine/helpcodes/`：Engine 从资源目录下的 `helpcodes/` 读辅助码表，共享校验放行这个真实目录。`StagedResources` 按相对路径列出其中的文件，所以辅助码表跟其他资源一起复制到 `files/engine`，表有变化时同样重新暂存。

引擎编进了取自 libhangul `data/hanja/hanja.txt` 的韩语汉字表，其 BSD-3-Clause 许可第 2 条要求二进制分发附带声明；引擎的粤语与注音方案所用的粤拼、注音音节与词条分别取自 rime-cantonese（CC BY 4.0，要求署名）与 libchewing-data（LGPL-2.1-or-later，要求附许可证全文与源码位置），笔画方案的笔顺取自 rime-stroke（LGPL-3.0，另含 CNS11643 全字库的署名要求），鸿蒙版的这些方案只在词库随包时提供（见上文「粤语、注音与越南语」），但声明随每一份引擎走，各平台共用一份清单，所以 `stage-resources.sh` 把 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`、`rime-cantonese-CC-BY-4.0.txt`、`libchewing-data-LGPL-2.1.txt` 与 `rime-stroke-LGPL-3.0.txt` 暂存到与 `resfile/engine` 相邻的 `resfile/licenses/`，随 HAP 一起分发；放在 `engine` 里会被锁文件校验拒绝。

`stage-resources.sh` 的第二个参数（默认 `target/offline-glosses`）是可选的非英文离线释义，由 `scripts/build_offline_glosses.py` 生成。数据库和 `offline-glosses-NOTICE.txt` 都在时暂存到 `resfile/offline-glosses`，键盘启动时用同一个 `StagedResources` 复制到 `files/offline-glosses`，与 `files/engine` 相邻，引擎就在那里找 `zh-<lang>.db`；新包不带它们时会删掉旧副本。已安装词典的目标语言在翻译查询里以 `offline_gloss_languages` 出现：用户自己配置的在线翻译先答，离线词典只补在线没答上的候选，同一行按目标顺序合并。

支持 `arm64-v8a`、`armeabi-v7a` 和 `x86_64`。原生库暂存到 `entry/libs/<abi>/`，这些目录是构建产物，不提交到仓库。

`stage-voice-runtime.sh` 用 `scripts/fetch_voice_runtime.py --platform harmony` 取回 `resources/voice-runtime.lock.json` 按 SHA-256 锁定的 sherpa-onnx `.har`，复制到 `entry/libs/sherpa_onnx.har`（同样被忽略、不提交）。`entry/oh-package.json5` 以固定文件名依赖它，所以不先暂存 `ohpm install` 会失败。本地语音识别（`local` 提供方）在 `workers/LocalAsrWorker.ets` 里加载 `voice_input.asr_model_path` 指向的模型目录，音频不出设备；该 HAR 自带的 `libc++_shared.so` 与 `build-native.sh` 暂存的同为 NDK 运行时，`entry/build-profile.json5` 用 `pickFirsts` 只打包一份。`arm64-v8a` 和 `x86_64` 之外（即 `armeabi-v7a`）上游 HAR 不带库，本地识别在该 ABI 上不可用。

系统语音（`system` 提供方）以写音频模式（`recognitionMode: 0`）启动 CoreSpeechKit：麦克风由 `HarmonyPcmCapture` 采集，按 1280 字节切帧后 `writeAudio`，`isFinal` 只结束一句，`isLast` 才结束整个会话。资源准备仍使用根目录固定的 `resources/desktop-dictionary.lock.json`，不得把本机路径、凭据或用户输入放入 HAP。

## 模拟器验证（2026-09-20）

首次在 HarmonyOS 模拟器上跑起来，记录可复现路径与结果。当时的 bundleName 是 `app.msime.client`，下文日志与验证记录里的包名照原样保留；现在的 bundleName 是 `app.msime.harmony`，命令已按它改写。镜像为 DevEco 自带的 HarmonyOS 6.0.1(21) phone，与项目 `compileSdkVersion` 一致：

```sh
emu=/Applications/DevEco-Studio.app/Contents/tools/emulator/Emulator
"$emu" -license accept && "$emu" -list        # 实例名
"$emu" -start "<实例名>"                       # 首次约需 6–8 分钟才向 hdc 注册
export PATH="<command-line-tools>/sdk/default/openharmony/toolchains:$PATH"
hdc list targets -v                            # 等到 Connected
hdc file send <hap> /data/local/tmp/msime.hap
hdc shell bm install -p /data/local/tmp/msime.hap
hdc shell ime -e app.msime.harmony -f          # 启用输入法
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
