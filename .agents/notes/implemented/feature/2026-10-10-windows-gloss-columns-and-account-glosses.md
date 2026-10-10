# Agent Note: Windows 的释义列快捷键与「水杉账号」候选释义

Status: implemented

## Problem

macOS 上候选的释义可以按列上屏：Option+数字上屏那个候选第一种目标语言的释义，Control+数字上屏第二种；Tab / Shift+Tab 在候选和它的释义列之间预选，预选的列带下划线，之后数字、空格、回车上屏那一列（`InputController.mm` 的 `commitCandidateGlossColumn:`、`cycleArmedGlossColumnBackwards:`）。Windows 只有 Ctrl+Enter 上屏高亮候选的第一条释义，Tab 只翻页。

候选翻译服务选「水杉账号」时，macOS 用设置应用登录的账号、没有登录时用本机匿名账号把当前页的中文候选发到 `https://api.msime.app/v1/translate`（`BackendCandidateGloss.swift`）。共享层的 `msime_client_translation_query` 对每个宿主都会给出 `translation_account`，但 Windows 的 `TranslationWorker` 只认腾讯云、小牛和自定义服务，选了水杉账号等于什么也不翻；设置页也不给 Windows 列出这个选项。第一波已经让设置应用和 Server 共用 `%LOCALAPPDATA%\<版本用户目录>\account` 下的会话并提供了 `msime_client_account_access_token`（见 [Windows 设置应用与输入法共用会话](2026-10-10-windows-desktop-cloud-account.md)），只差 Server 去用它。

## Decision

### 释义列

规则在纯头 `platforms/windows/src/input/GlossColumnPolicy.h`，Server 和测试共用：

- 只按 Alt 的主键盘数字是第 1 列，只按 Ctrl 的是第 2 列（`gloss_column_for_digit_modifiers`）；AltGr 读成的 Ctrl+Alt、带 Shift 的都不是。小键盘数字不算：Alt+小键盘数字是 Windows 的 Alt 码输入。
- 第 N 列是释义按 U+2028 分开的第 N 行（`gloss_column_text`），也就是第 N 种目标语言，和第一波让候选窗每种语言画一行的约定一致。上屏的是整行，和 macOS 一样，不是 Ctrl+Enter 取的第一条义项。
- Tab / Shift+Tab 在 0 和高亮候选有释义的列之间循环（`next_armed_gloss_column`）；高亮候选两列都没有释义时返回空，Tab 照常翻页。

TIP 侧 `tsf/Key/KeyEventSink.cpp` 的 `IsGlossColumnShortcut` 在候选列表打开（`_candidateMode != CANDIDATE_NONE`，和 Ctrl+Enter 相同；排队重放路径看投影里还有没有组字）时把 Alt/Ctrl+数字归为 `FUNCTION_SERVER_CANDIDATE_KEY`；TIP 自己宿主会话组字的方案（韩文、注音、越南文、藏文）不接，和 Ctrl+Enter 一样；宿主自己画候选的 UILess 场合（游戏、全屏）和 Server 连不上时也不接，组合键留给应用。Tab、空格、数字本来就交给 Server。

Server 的释义列、Ctrl+Enter 和释义页都只认带 `PipeMetadata::CandidateActive` 的键。这一位原来只在 `CANDIDATE_ORIGINAL`（通配转换）时带，而普通组字的候选列表是 `CANDIDATE_INCREMENTAL`，于是第一版在普通组字里 Alt/Ctrl+数字交给了应用，Tab 只翻页，释义页上的数字和空格也到不了 `translation_page_key`。现在增量候选开着时，发给 Server 的包对这些路认的键带上这一位（`tsf/Global/CandidateActiveKeyPolicy.h` 的 `CandidateKeyReportsActiveList`：空格、数字 1–9、Tab/Shift+Tab、翻页和上下方向键、只按 Alt 或 Ctrl 的主键盘数字、Ctrl+Enter）。没有给增量候选的每个键都带：裸回车靠没有这一位按组字原文上屏，`[` `]` `-` `=` 的以词定字和 Ctrl+Backspace 恢复上一段也只在没有这一位时生效。因此 Server 的 `translation_page_key` 把不带这一位的按键（字母、退格、回车等）当作离开释义页，关掉它再照常处理；以前它对这种键什么也不做、释义页一直开着，在只给部分键带这一位之后，打开释义页再接着打字，下一个空格就会上屏旧义项（`gloss_column_keys.cpp` 断言了这一点）。用例 `msime-tsf-candidate-active-key-policy`（哪些键带）和 `msime-tsf-gloss-column-wiring`（源码接线）。

Server 侧 `ReplyComposer::gloss_column_key` 排在 Ctrl+Enter 和通用修饰键规则之前：

- 上屏释义与 Ctrl+Enter 只有一条义项时同一条路：按繁体输出开关转换，`exact_commit(prefix + 释义)`，组字取消，不经过候选选择，引擎学不到用户没选的词。
- Alt/Ctrl+数字指向的格子没有那一列时回一个不上屏的导航回执：TIP 已经吃掉了这个键，交还不了应用。Server 不接这个键的其他场合（UILess、英文模式、焦点或候选在 TIP 与 Server 之间不同步、TIP 自己组字的方案）也一样回执：TIP 吃掉的键 `configured_key` 返回空，`SessionPump` 会当作分派失败断开这个客户端。Ctrl+Enter 打开的释义页上按 Alt/Ctrl+数字会先关掉释义页，免得它留到下一次组字，让空格、数字上屏旧义项。
- 预选的列 `armed_gloss_column_` 只活到下一个按键：`basic_key` 一进来就清掉，只有预选列的 Tab 再设回去；以词定字、全半角切换、鼠标点选候选和取消也清。鼠标翻页、候选菜单（置顶、删除）不清，候选窗也按新的高亮候选重新判断，两边一致，和 macOS 换页后高亮候选仍有那一列就保留预选相同。预选时回执的 view 带 `armed_gloss_column`，`candidate_presentation` 读出来（高亮候选确实有那一列才算，`candidate_armed_gloss_column`），`CandidateMailbox::refresh_view` 在释义或云候选送到时按同一条规则保留它，`CandidateWindow` 用 `IDWriteTextLayout::SetUnderline` 给高亮候选那一行释义画下划线（`candidate_gloss_column_range`）。
- 预选后数字只在它本来就选候选时接（`digit_selects_candidate`），空格只在它不是拼写符号时接；那一列为空时照常选候选。

TIP 的 `_HandleCandidateFinalize`（空格、候选列表里的回车，以及没有宿主会话时的数字都走它）新增 `CommitExactText` 分支，交给 `_CommitServerExactText`：宿主会话随 Server 一起丢掉组字，把回复原样插进组字并结束组字。以前没有这个分支，Ctrl+Enter 释义页上用数字或空格挑义项时，回复落到末尾直接结束组字，上屏的是组字里原有的文字而不是释义。宿主会话拥有组字时（生产环境的常态）数字不经过它，而是在 `_HandleCandidateWorker` 里向宿主会话选词；那里现在先不中断管道地读这个键的 Server 回复，是 `CommitExactText` 就同样交给 `_CommitServerExactText`，否则照旧选词。第一版漏了这条路，预选列或释义页上的数字上屏的是候选本身。

按精确文本上屏释义的几条路（释义列、Ctrl+Enter 只有一条义项、释义页上按键或点选）取消组字后，回复里的 transition 以前是把 view 本身加一个 `commit` 字段，而送达后 `candidate_presentation` 和 `confirm_ui_delivery` 都读 `transition.view`：取不到就在输入队列里抛出，队列随即停掉。现在和 Engine 的转换一样包成 `{commit, view}`，`candidate_translation_commit.cpp` 和 `gloss_column_keys.cpp` 断言了这个形状。

### 「水杉账号」候选释义

`TranslationWorker` 在查询带 `translation_account: true` 时走 `account_glosses`，不走翻译计划：

- 本机英文释义先答，账号只问共享层标了 `online_gloss` 的中文候选里本机没答上、也不在缓存里的词，一页一个 POST，`{"texts", "source_lang": "ZH", "target_lang": 大写代码}`，最多 32 个词（`account_gloss_words`、`account_gloss_target`）。两种目标语言时上层本来就按语言各调一次，所以是两个请求，和 macOS 相同。非英文目标装了离线词典时，词典答上的候选先从查询里去掉，账号只补词典留下的空（macOS `synchronizeAccountGloss` 的顺序）；用户自己的服务仍排在词典前面，整页都问。
- 回复按 macOS `BackendChatClient.translate` 的判据校验（`account_gloss_values`）：`code` 是 200，条数与问的词相同，每条不超过 4096 字节、不含换行回车和其他控制字符，任何一条不合格整批作废；共享会话的 `msime_client_apply_translations` 拒收一切 `char::is_control`（一条不合格整页释义连同本机词典的一起被拒），所以 DEL 和 C1 控制字符（U+0080 到 U+009F）也拒收，macOS 放行的制表符换成空格，只有空白的释义算没有释义；释义等于词本身算没有释义。有释义的按「水杉账号这个服务、目标语言、词」进缓存（不分登录的是哪个账号，释义与账号无关），没有释义的记八分钟否定缓存，请求失败什么也不记。英文结果照其他服务的规矩存进本机学到的释义表。
- 令牌经 `msime_client_account_access_token` 取，设置应用登录的账号优先，没有时用匿名账号；服务端回 401 时带上被拒的令牌再取一次，共享层强制刷新。两种会话都没有（`account_unauthorized`）时补调 `msime_client_ensure_anonymous_account` 再取，和 macOS `translationSession` 的 `ensureSignedIn` 一样；补注册失败后十分钟内不再试，免得离线时每一页都阻塞翻译线程。取令牌和补注册都跑在分离的线程上（`account_access_token_detached`），翻译线程每 50 毫秒看一次 Server 是否在退出：注册最多阻塞约一分钟，刷新令牌也要联网，`TranslationWorker::stop()` 的 join 不能跟着等，否则 Server 退出（注销、更新、托盘重启）会卡住这么久。
- 请求只在 Server 退出时中止，换页不中止：macOS 也让在途的请求跑完，回答进缓存。超时 10 秒（macOS 30 秒）：Windows 只有一个翻译线程，等太久后面的页都排着。
- 账号目录由 `server_main.cpp` 在注册匿名账号的同一处经 `TranslationWorker::set_account_directory` 设一次，只在 `--production` 下设；预览 Server 选了水杉账号也不联网。

设置页在 Windows 上列出「水杉账号」：`translation-candidate-settings.ts` 的 `showAccountProvider` 和 `use-translation-settings.ts` 的提供方判断都加上 `windows`；原生设置窗口「翻译服务」一行的说明也列上水杉账号。

## Alternatives considered

- **预选的列像 macOS 一样跨按键保留（方向键移动高亮后下划线跟着走）** — 交互和 macOS 完全一致。但 Windows 的回车由 TIP 在进程内按组字原文上屏、不读 Server 回复，TIP 不知道 Server 预选了哪一列；要让回车上屏预选列，得新增一种回复类型让 TIP 记住预选状态，并让两边在每个按键上同步清除，任何一个漏掉的键都会让回车在两边意思不同。只活到下一个按键的规则在 Server 一侧就能说清楚，代价是预选之后按方向键会取消预选。
- **账号释义也走 `msime_client_custom_translation_plan` 和现有的逐项缓存循环** — 少一段代码。但计划会把英文候选排成 en→zh 的反向翻译，账号只翻中文；计划也不看 `online_gloss`，会把颜文字和拼音缓冲送出去。
- **在 host-api 里加一个整包完成账号翻译的 C ABI（取令牌、发请求、校验都在 Rust）** — 令牌不进 C++，和 macOS 共用一份校验。但请求就没法在 Server 退出时中止，翻译线程的超时和取消要另做；第一波已经提供了取令牌的接口，HTTP、超时和取消在 `TranslationWorker` 里现成。
- **Alt/Ctrl+数字没有释义时把键交还应用** — macOS 在没有释义时会结束组字并把键交给应用。Windows 的 TIP 在问 Server 之前就决定了吃不吃这个键，它不知道释义有没有到；只在候选列表打开时才吃，影响面只在组字中。

## Consequences

- **收益**：Windows 能按列上屏释义，键位和 macOS 一致（Alt 对应 Option）；选「水杉账号」后候选释义可用，不需要自备密钥，未登录也能用匿名账号。Ctrl+Enter 释义页用数字或空格挑义项不再上屏拼音。
- **代价与已知上限**：
  - 回车不上屏预选的列，预选后按方向键、字母等任何键都会取消预选；鼠标点选候选照常上屏候选本身（macOS 点选也上屏预选列）。
  - Alt+数字在 TSF 里是 `WM_SYSKEYDOWN`，TIP 的按键接收器能不能收到、应用的菜单栏会不会被激活，没有在真机上确认过；Ctrl+1–9 在组字期间不再给浏览器切标签。
  - 账号释义在一个翻译线程上串行，服务端慢时同一页的本机词典结果要等账号请求结束才一起交给会话。
  - 设置应用的账号页仍不显示本机匿名账号（「本机账号」标记、标识、提示）也不在登录后弃用它，这部分没有做。

## Verification

- `platforms/windows/tests/input/gloss_column_policy.cpp`（修饰键选列、按行取列、Tab 循环）和 `platforms/windows/tests/candidate/account_gloss_policy.cpp`（发哪些词、目标语言代码、回复校验）在本机用 clang++ 编译运行通过，CMake 里注册为 `windows-gloss-column-policy`、`windows-account-gloss-policy`。
- `platforms/windows/tests/input/gloss_column_keys.cpp`（`windows-gloss-column-keys`）在真实 Engine 会话上走 Tab/Shift+Tab 预选、空格和数字上屏预选列、Alt/Ctrl+数字直接上屏、没有第二种语言时的回执、其他键和鼠标点选清除预选、UILess 下 Alt+数字仍有回执；它链接 host-api，只在 Windows 构建里跑。
- `platforms/windows/tsf/tests/input/candidate_active_key_policy.cpp`（`msime-tsf-candidate-active-key-policy`，哪些键带 `CandidateActive`）在本机用 clang++ 编译运行通过；`gloss_column_wiring.cpp`（`msime-tsf-gloss-column-wiring`）核对增量候选的吃键条件、发出的这一位、宿主会话数字路径先读回复，对改动前的源码会失败。Server 用例 `gloss_column_keys.cpp` 是手动把 `CandidateActive` 放进包里的，它本身看不出 TIP 从没发过这一位。
- `apps/desktop/tests/settings/translation-candidate-settings.test.ts` 锁住 Windows 列出「水杉账号」和已保存的选择读回为账号。
- 改动的 C++ 文件都用 MinGW-w64 `-fsyntax-only` 检查过；MSVC 编译、真机上的 Alt 组合键、下划线绘制和账号请求都还没有在 Windows 上跑过。
