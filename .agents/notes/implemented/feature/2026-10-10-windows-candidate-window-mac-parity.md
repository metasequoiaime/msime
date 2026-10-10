# Agent Note: Windows 候选窗补齐 macOS 的翻页、logo、多语言释义、读音与悬停提示

Status: implemented

## Problem

对照 macOS，Windows 候选窗有两处缺陷和一串缺失。缺陷：首行 ‹ › 翻页箭头和滚轮共用 `InputState::page_candidate` 里的 `navigation.mouse_wheel` 闸，这个开关默认关，于是默认安装上箭头画着、鼠标变手形，点了却被 Server 拒掉；滚轮开关只在 Server 启动时读一次，改了设置要重启才生效。缺失：`show_app_logo` 不起作用，logo 一直画，`HostCapabilities::app_logo` 对 Windows 为 false；设了第二种释义语言时两种释义拼成一行「a / b」，而且没有任何设置入口能设它；横排候选在释义到达后才变高，打字几秒后卡片突然长高；释义后面没有 IPA 读音，整句候选没有逐词拆解，打包也不带这两张表；悬停候选没有提示；原生设置窗口的「候选词翻译」组找不到翻译服务的凭据在哪里。

## Decision

**翻页闸只管滚轮。** `CandidatePage` 带 `from_wheel`，`CandidateWindow` 的 `WM_MOUSEWHEEL` 置 true、箭头点击为 false，`InputState::page_candidate` 按 `candidate_page_allowed(from_wheel, navigation.mouse_wheel)`（`src/candidate/CandidateWheel.h`）放行，和 macOS 的 `changeCandidatePage:` 不看滚轮开关一致。滚轮开关经 `server_main.cpp` 的原子量 `candidate_mouse_wheel` 发布，主循环每轮调 `CandidateWindow::set_mouse_wheel`，与 `follow_cursor` 同一条路。

**logo 开关。** `CandidateLayoutSettings` 多读 `show_app_logo`（缺值按关，与新装默认一致）和 `reserved_gloss_lines`，一起编码在主循环取用的原子量里。`candidate_card_metrics` 多两个参数 `logo_visible`、`pager_visible`：不画 logo 时 logo 和它的间隔不占宽度，拼音从左边距开始；拼音隐藏、没有翻页、也不画 logo 时首行高度为 0。`card_bounds` 和 `paint` 用同一组参数算首行，行的位置和点击区域才对得上。`host_surface.rs` 对 Windows 打开 `app_logo`，共享「候选窗口」页的「显示水杉 logo」随之出现，原生设置窗口「候选窗口」页的「外观」组也加了同名开关；悬浮工具栏的 logo 由工具栏那篇笔记处理（[工具栏](2026-10-10-windows-floating-toolbar-mac-parity.md)）。

**每种目标语言一行。** 与 macOS 的 [U+2028 编码](../bug-fix/2026-10-08-macos-gloss-line-separator.md) 相同：`TranslationWorker` 按目标语言顺序给每个候选攒一组行，`join_translation_lines`（`src/system/TranslationDisplay.h`）先把来源自带的 U+2028、U+2029 折成空格，再用 U+2028 连接，缺的语言留空行，第 N 行始终是第 N 种目标语言；只有一种目标语言时结果同样折掉来源自带的分隔符。会话照收 U+2028，`candidate_secondary_text` 读回时按它分行。Ctrl+Enter（`ReplyComposer::commit_candidate_translation`）只取 `primary_translation_line`，第一条非空的行，分行符不会写进文档。共享设置页对 Windows 显示「第二种语言」（`translation-candidate-settings.ts` 的 `windows`），原生「标点与翻译」页同样加了这一行（「不显示第二种语言」写回 null），并加一行「翻译服务」打开共享应用的 `settings:expression`，凭据只在那里编辑。

**预留释义高度。** 横排时每行候选至少 `candidate_reserved_row_height(metrics, N, measured)` 高：一行候选文字加 N 行释义。N 是 `reserved_gloss_lines`，韩文汉字列表再加 훈음 那一行，这一页的方案或模式不请求释义（`CandidatePresentation::shows_glosses`，和 host-api 翻译查询的方案与本地模式判据相同）时只剩 훈음 那一行。竖排不预留。

N 只算有释义来源的目标语言（`candidate_reserved_gloss_lines`），算到最后一种有来源的为止，最多 2：打开候选翻译且有能回答的在线服务（水杉账号，或凭据可用的腾讯、小牛，或填了地址的自定义翻译，判据同 host-api `selected_translation_services`），或英文目标打开了离线英文释义，或这种语言的离线释义词典装在 resources 旁边（Server 启动时 `installed_offline_gloss_languages` 查一次）。macOS 打开候选翻译就预留，因为默认的本机翻译总能补上；Windows 没有本机翻译，照搬的话新装（候选翻译默认开、腾讯没有密钥、离线英文释义默认关）的横排卡片每个候选下面都挂一行永远空着的释义，比改动前和 macOS 都高。只装在下载资源包里的离线词典不在启动时看，那种语言不预留，释义到了卡片照旧变高。

多行释义到达后由 DirectWrite 量高（`candidate_item_layout` 的 `run_height`），常见字体的行高约 1.33 倍字号，比 `translation_line` 的 1.25 倍高；所以预留两行时窗口用同一种格式量 N 行占位文字（`CandidateWindow::reserved_secondary_height`，和 macOS `reservedGlossHeightForFont` 量「X\nX」一样），预留的和画出来的一样高，一行时仍按固定行高。

**读音与逐词拆解。** 规则移植自 macOS 的 `CandidatePronunciation.h`，放在 `src/candidate/CandidateGlossReadings.h`：每行释义按第一个分隔符前的词读，有假名是日文，拉丁字母只在英文目标那一行算英文，英文候选读它自己。`TranslationWorker` 在翻译完成后（在同一条工作线程上、读本机文件）调 `msime_client_pronunciation_request` 和 `msime_client_gloss_breakdown_request`（拆解在查询带 `gloss_breakdown` 时问：候选翻译或离线英文释义打开、目标语言里有英文，和 macOS `currentGlossRequest` 的条件相同，host-api 这时也带上 `resources`），结果放进 `Result::readings`（`CandidateReadings`，按候选文字），不进会话。`SessionController` 把它随释义交给 `CandidateMailbox::translations`；这一页没有任何释义、只有拆解时走 `CandidateMailbox::readings`，换一个 `render_serial` 让候选窗重画（`reposition` 因此也比较 `render_serial`）。偏好每次发布时 `clear_readings` 摘掉旧的。挂读音时要求候选眼下的释义与算读音时那条相同，免得旧读音跟到新释义后面；繁体输出时按繁体文字也存一份。显示为「释义  /读音/」，拆解另起最后一行，和释义同色。host-api 的翻译查询在打开读音时也带 `resources`，在线翻出来的英文释义同样能标音标。`apps/desktop/src/main.tsx` 对 Windows 打开 `candidatePronunciation`，原生「标点与翻译」页的「多语言与释义」组也加了「显示读音」。日文行的罗马字后来由微软日语输入法的 IFELanguage 补上，见 [遗留项](2026-10-10-windows-mac-parity-leftovers.md)。安装包按 macOS `stage-resources.sh` 的做法，把 `target/pronunciations`、`target/character-glosses`、`target/word-glosses` 连同授权声明装到 `server_exe` 下 resources 旁边（`Prepare-PackageFiles.ps1`）；单字和词的英文释义只给提供中文方案的版本。

**悬停提示。** 候选窗用 comctl32 的 tooltip 控件，每行候选登记一个区域（`sync_tooltips`，行和快照都没变时不重登记），文字是 `candidate_tooltip_text`：候选全文，下一行起是释义，和 macOS 候选按钮的 toolTip 相同。

## Alternatives considered

- **把滚轮闸整个挪进 `CandidateWindow`，Server 不再检查** — 一行改动，窗口本来就在 `WM_MOUSEWHEEL` 里看 `mouse_wheel_`。但 Server 是唯一能拒绝过期请求的地方，窗口里的开关值又可能比已发布的偏好晚一轮；请求带上来源、由 Server 按当前偏好裁决，两边各管各的。
- **第二种语言仍用 `" / "` 拼在一行** — 不用改会话里的格式，Ctrl+Enter 也不用改。但 macOS 已经按行显示，后续的按列上屏（Alt/Ctrl+数字、Tab）需要知道哪一段是哪种语言；`" / "` 也会出现在释义原文里，无法可靠地拆回来。
- **读音单开一个工作线程，按候选窗的当前页去问** — 和 macOS 的 `_pronunciationQueue` 形状最像，翻译关掉时拆解也能单独请求。但 Windows 的翻译查询本来就带着目标语言、资源目录和读音开关，翻译线程拿到释义的同一时刻正好能算读音，单开线程要再复制一套去抖、取代和缓存失效。翻译查询覆盖 macOS 请求拆解的全部情形：macOS 的拆解在候选翻译或离线英文释义打开、目标语言里有英文时请求（`currentGlossRequest`），这时 host-api 的翻译查询一定不为空。最初这里写的是「macOS 的拆解也只在离线英文释义打开时请求」，这个前提是错的，Windows 因此在默认偏好（只开候选翻译）下从不出拆解；现在 host-api 在这种情况下给查询加 `gloss_breakdown` 并带上 `resources`。
- **日文罗马字用 IFELanguage（MS-IME 的 GetPhonetic）** — Windows 上最接近 macOS 系统分词器的接口。但它依赖日文 IME 是否安装、在 Server 进程里要起 COM 并处理失败，读出的是假名还要再转罗马字；这次只标英文音标，日文行不标，半截读音不显示的规则与 macOS 相同。
- **用操作系统的本机翻译模型补齐离线词典没覆盖的候选释义** — macOS 用 Apple 的 Translation 框架（`BackendOnDeviceGloss.swift`），只用用户已下载的语言对，设置页还提示哪些语言可下载。Windows 没有可与之相比的公开本机机器翻译接口，没有系统服务可接，所以不移植：那条下载提示只在 macOS 上出现。桌面应用在每个平台都给 `SettingsClient.onDeviceTranslation`，是共享设置页按平台只在 macOS 上用它：`use-macos-settings.ts` 只在 macOS 上查可下载的语言，`use-translation-settings.ts` 只在 macOS 上列出缺的语言。离线释义靠随包的单字与词释义表，在线释义靠翻译服务和「水杉账号」。
- **候选窗自己实现 UIA 提供者给每行加读屏名称** — macOS 每个候选都有 accessibilityLabel。Windows 的读屏已经能通过 TSF 的 `ITfCandidateListUIElement` 读到候选，缺的只是翻页箭头和页码；一套 `IRawElementProviderFragment` 树在没有 Windows 真机的情况下验证不了，这次不做；后来在 [自绘窗口的 UI Automation 读屏](2026-10-10-windows-server-window-ui-automation.md) 里做了。

## Consequences

- **收益**：默认安装上翻页箭头能用；滚轮、logo、第二种语言和读音开关改了即时生效；两种目标语言各占一行，为按列上屏留好了列；横排卡片在释义到达时不再长高；打开读音后英文释义带音标，整句候选多一行逐词释义；长候选可以悬停看全文。
- **代价**：Windows 宿主和会话之间多了一条与 macOS 相同的私有约定，读会话释义的新路径必须按 U+2028 分行，否则会看到这个字符。读音和拆解跟着翻译查询走，候选翻译和离线英文释义都关掉、或目标语言里没有英文时不会有拆解（macOS 也是）。日文释义的罗马字依赖系统的微软日语输入法，没有它时含汉字的日文行不标（[遗留项](2026-10-10-windows-mac-parity-leftovers.md)）；共享设置页「显示读音」的说明不再按宿主区分。读音和拆解加上去超过候选窗能画的 4096 字节时，`candidate_secondary_text` 只画释义，不让一条很长的释义把整个候选窗弄失败。三张读音和释义表要构建机上已经有 `target/pronunciations` 等目录才会进包，发版流程目前和 macOS 一样不负责生成它们。`show_app_logo` 的取值规则沿用共享偏好：新装为关，所以新装的 Windows 候选窗不画 logo；文档里没有这个字段的老用户读成开，外观不变，和 macOS 一致。

## Verification

主机上能跑的纯策略测试：`tests/ui/candidate_wheel.cpp`（箭头不受滚轮开关管）、`tests/ui/candidate_card_size.cpp`（logo 隐藏后的宽度与首行高度、横排预留高度）、`tests/ui/candidate_layout_reload.cpp`（`show_app_logo`、预留行数及其编码）、`tests/core/translation_display.cpp`（U+2028 拼接、折叠、4096 字节上限、Ctrl+Enter 取首行）、`tests/candidate/candidate_gloss_readings.cpp`（读音与拆解的显示规则）。依赖 Engine 的：`tests/candidate/korean_hanja_presentation.cpp`（多行释义、读音、拆解和提示的拼法）、`tests/ui/candidate_mailbox.cpp`（读音单独到达时重画、释义变了不挂旧读音、偏好变化摘掉）。`crates/host-api/src/tests.rs` 的 `translation_query_carries_the_pronunciation_switch_only_when_on` 钉住只开在线翻译时查询也带 `resources`，`translation_queries_use_latest_preferences_without_resetting_composition` 钉住只开候选翻译、英文目标时带 `gloss_breakdown`，没有英文目标时不带；`tests/ui/candidate_layout_reload.cpp` 钉住新装默认偏好不预留、各种来源的预留行数和离线词典的检测，`tests/ui/candidate_card_size.cpp` 钉住两行释义按量出的高度预留，`tests/candidate/candidate_pointer_input.cpp` 钉住哪些方案的页请求释义，`host_surface/tests.rs` 钉住 Windows 的 `app_logo`。Windows 真机上要确认的：箭头点击翻页、logo 开关、两种语言的卡片、悬停提示在不抢焦点的候选窗上的表现。
