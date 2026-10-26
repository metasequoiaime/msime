# Agent Note: 候选词翻译的翻译范围默认只翻译当前高亮候选

Status: implemented

## Problem

候选词翻译开着时，候选窗里每个候选下面都有一行译文。维护者在 macOS 上看到「1.测试 / 3.测是 / 4.测试用例 / 5.测试人员 / 6.测试开发 / 7.侧视」每个都带一行英文，认为太乱，希望只翻译第一个候选，或者用户选中（高亮）的那一个。每个候选都请求一次，也让在线、账号和 AI 翻译接口的调用量按页长成倍增加。

## Decision

候选词翻译下新增共享偏好「翻译范围」`candidate_translation_scope`（`crates/client-core/src/preferences.rs` 的 `CandidateTranslationScope`），两档：

| 值 | 界面 | 行为 |
|---|---|---|
| `highlighted` | 仅当前高亮 | 只为当前高亮的候选请求并绘制候选词翻译；没移动过高亮时是第一个，方向键、Tab 或宿主支持的鼠标悬停移动高亮后跟着走 |
| `all` | 全部 | 当前页每个候选都翻译，这个选项出现以前的行为 |

新装和缺这个键的旧文档都读成 `highlighted`，不认识的值同样按 `highlighted` 读；键总是写进文档，不在 `settings_document::LOCAL_SECTIONS` 里，随设置文件和账号同步走，和相邻的翻译偏好一样。这是维护者授权的出厂值决定：老用户升级后也会从「全部」变成「仅当前高亮」，想要旧样子的在设置里改回「全部」。

范围只管「候选词翻译」：翻译服务（在线、系统、账号、AI）和只为候选词翻译查的离线词典。「显示英文释义」开关（`candidate_english_gloss`）的离线释义不受影响，照旧每个候选都有。

### 共享契约

规则放在共享 Rust 层，宿主只执行传输和绘制：

- **查询**（`crates/host-api/src/ffi/providers.rs` 的 `msime_client_translation_query`）：范围生效（候选词翻译开着且范围是 `highlighted`）时，`candidates` 只有高亮的那一个候选，宿主现有的服务路径不用改就只请求它。离线释义开关要查释义时，查询另带 `gloss_query`：同一代次、覆盖整页的完整查询文档，`provider` 为 `none`、各服务配置为空、`translation_account` 为假、不存英文翻译，宿主用处理普通查询的同一条路径算它，回答标 `"gloss": true`。高亮不在这一页（宿主报了不含高亮的显示页）时，只返回那份不带服务的文档。范围是 `all` 时查询与以前相同，没有 `gloss_query`。
- **应用**（`msime_client_apply_translations`，运行时 `Runtime::apply_translation_answers`）：每条回答多一个可选的 `gloss` 字段。范围生效且高亮在这一页时，不带标记的回答一律算候选词翻译，进 `highlight_translations`，只画在高亮行；带标记的离线释义进 `translations`，每行都画；高亮行优先画候选词翻译，没有时画离线释义。没升级的宿主把整页答回来，也只有高亮行画出来。`highlight_translations` 在同一次组字里保留，高亮移回来直接画，不等新回答；组字变化时与 `translations` 一起清掉。
- **绘制**：视图候选的 `translation` 已经按范围给好，宿主照旧画它即可。视图候选新增 `translation_reserved`：范围生效且这一页会请求释义时高亮行为真，竖排宿主据此给高亮行一直留出译文高度（行数与横排为未到释义预留的相同），移动高亮时窗口高度不跳；横排本来就按行数统一预留。
- **失效**：范围进 `translation_query_preferences_changed`，切换时清掉旧释义并推进代次，旧范围下发出的查询的回答落不下来。移动高亮本来就推进代次，宿主重新查询，旧代次的回答被拒绝。

「这一页会不会请求释义」（方案显示释义、不是临时日文或网址模式、没有脚本插件行）原来写在查询函数里，现在归 `Runtime::page_requests_translations`，查询与留行共用。

### 各平台

设置界面：共享 React 设置页（macOS、Linux、HarmonyOS，`packages/ui/src/settings/candidate-translation-options-section.tsx`）、Windows WinUI（`platforms/windows/settings/main.cpp`）与 Win7（`platforms/windows/settings/win7/GlossForm.h`）、iOS（`platforms/ios/App/Sources/app/OnboardingView.swift`）、Android（`platforms/android/java/app/msime/android/home/TypingPage.java`）都在「候选词翻译」开关下加「翻译范围」一行，选项「仅当前高亮」「全部」，候选词翻译关着时置灰、已存的值不动。

请求与绘制：

- **Windows**：`TranslationWorker.cpp` 把 `gloss_query` 拿出来，主查询和它各走一遍原来的 `translate()`（在多目标语言拆分之前），合并时给子查询的回答标 `gloss`；判定抽成 `src/candidate/TranslationGlossQuery.h`。竖排里带 `translation_reserved` 的那一行至少留出横排同样行数的译文高度，并且这一行的译文总是画在候选文字下面（其他行的短释义仍画在右侧），否则留出的高度下面是一行空白。
- **macOS**：`InputController.mm` 原来的英文离线词典、其他语言离线词典和自定义/腾讯/小牛/AI 三路请求都按视图整页组，不看查询的 `candidates`，现在只问查询列出的候选；两份离线词典请求把 `gloss_query` 的整页一起读，回答标 `gloss`。系统翻译和账号队列本来就按查询走，只为高亮排队。竖排高亮行按「译文到了以后的行高」一直留位（`candidatePageGeometry:`，`glossHeightReservedForFont`）。
- **Linux**：IBus `ClientEngine.cpp` 与 Fcitx5 `FcitxEngine.cpp` 共用 `src/candidates/CandidateTranslationScope.h`：先只查本机词典算 `gloss_query`，再为高亮候选跑主查询；`provider` 为 `none` 时不连 provider socket；按去掉代次的主查询记回答（上限 32 条），高亮移回问过的候选直接交回。原来离线词典按视图整页查、来源按视图下标对，改为用查询自己的候选。原生画布与 canvas 皮肤给高亮行留高，classicui 用全角空格占位行。
- **HarmonyOS**：`KeyboardSession.ets` 抽出 `localTranslations`，`gloss_query` 走同一条离线路径，`TranslationPolicy.glossAnswers` 标 `gloss`，查询签名把 `gloss_query` 算进去。触屏条已统一预留；2in1 竖排的译文画在词旁边，同一页内窗口宽度不比这一页用过的窄（`CandidateWidthPolicy.ts`）。
- **iOS**：不调 `msime_client_translation_query`。`CandidateTranslationStore.swift` 范围生效时只为视图里高亮的候选请求在线、账号和 AI 翻译，高亮换走时取消在途请求、缓存保留；离线释义交给 Rust 时标 `gloss`。候选条和展开面板原来直接用 store 缓存画译文，改为范围生效时只有高亮候选取缓存。
- **Android**：同样自己组请求。`CandidateTranslationPolicy.java` 新增 `Scope`，账号和用户服务路径只把高亮候选交给 store，`CandidateGlossModel` 解出的离线释义标 `gloss`。Android 原有「每种目标语言离线优先、在线补空」的合并顺序不变，所以高亮行在离线词典有答案时画离线释义，与 Rust 的「高亮行优先候选词翻译」在这一点上不同：Android 交给 Rust 的高亮行回答已经是合并后的结果，只来自离线的那几条才标 `gloss`。

### 旧笔记审计

- [新装默认关闭候选翻译](2026-10-15-new-install-candidate-translations-off.md)：部分相关。它定候选翻译总开关的默认值和缺字段兼容；本篇是总开关之下的新偏好，缺字段读成新默认值，与那篇「缺字段沿用开启」的总开关规则互不影响。
- [同代次候选释义合并](../bug-fix/2026-10-11-generation-gloss-merge.md)：部分重叠。同代次非空合并、空数组主动清空、旧代次拒绝的规则不变；本篇在合并时按 `gloss` 标记分进两张表。
- [macOS 账号释义等待页的控制器归属](../bug-fix/2026-10-24-macos-account-gloss-queue-owner.md)：无关的同一条流水线，本篇不改它的归属规则。
- [macOS 候选显示补齐词性、单条释义与两类徽标](2026-10-21-candidate-display-toggles.md)：无关，那是释义的显示整理，不决定哪些候选有释义。

## Alternatives considered

- **查询里给每个候选加「要不要候选词翻译」的标记，整页照旧列出** — 宿主改动后请求侧最精确，离线释义也不用另起一份文档。但每个宿主都有好几条服务分支（Windows 的账号、计划、AI、非英文离线词典，macOS 的系统翻译和账号队列），每条都要按标记过滤，漏一条就照旧整页请求；没升级的宿主更会把标记当不存在，整页照问。只列高亮候选让现有路径天然只问一个，离线释义另成一份不带服务的文档，宿主只多一处「算子查询、打标记、合并」。
- **只在绘制侧过滤，请求照旧整页** — 共享运行时一处改动、宿主完全不动。但翻译接口的调用量一点没少，而这正是维护者提出的理由之一；系统翻译这类串行的本机模型还会让高亮那一行排在整页后面才出来。
- **单张释义表，按「是不是高亮那一行的文字」判断回答的归属** — 不需要 `gloss` 标记。但高亮那一行的文字同时出现在主查询和 `gloss_query` 的回答里，一张表分不出哪条是候选词翻译；没升级的宿主整页答回来时，非高亮行的在线译文也会被当成离线释义画出来。
- **默认值保持「全部」，只给新装「仅当前高亮」** — 老用户的界面不变。维护者认为整页译文本身就乱，且明确要求老用户也用新默认，按授权执行。

## Consequences

- **收益**：默认每次只有一个候选下面有译文，翻译接口的调用量从整页降到一条；高亮移回来复用同一次组字里已有的译文。离线英文释义的行为不变。
- **代价与已知上限**：老用户升级后默认看不到非高亮候选的译文，要在设置里改回「全部」。宿主处理 `gloss_query` 是一份额外的离线词典查询（整页，含高亮那一行），只读本机词典。`highlight_translations` 只在一次组字内保留，换一次输入就要重新请求，跨组字的复用靠各宿主自己的翻译缓存。宿主的鼠标悬停若只改宿主自己的高亮、不经运行时，译文不跟着悬停走。竖排只保证高度不跳：macOS 与 HarmonyOS 2in1 的窗口宽度仍可能随高亮行译文变化（HarmonyOS 同一页内不收窄），Windows 竖排高亮行的译文改画到候选下面，和其他行右侧的短释义样子不同。Linux 的单路请求不取消在途的慢请求，只丢弃它的回答，高亮移动时新请求要等它返回。

## Verification

- 共享层：`crates/host-api/src/tests/translation_scope.rs` 五项，经真实会话的 C ABI 打字、移动高亮、应用回答：只问高亮候选、旧代次拒绝、移回复用、`gloss_query` 不带服务且覆盖整页、离线释义每行都画、换范围清旧释义、高亮不在显示页时只剩离线文档。把 host-api 与 input-runtime 的实现换回 develop、只保留偏好字段和测试时，五项里四项失败（「全部」那项本来就该通过）。`crates/client-core/src/preferences/tests.rs` 的 `candidate_translation_scope_defaults_to_highlighted_for_new_and_legacy_documents` 钉住新装、缺键、未知值和往返。`cargo test -p msime-host-api --lib`（990 项）、`-p msime-input-runtime`（391 项，含回包分配上限）、`-p msime-client-core` 通过，三个 crate 的 clippy `-D warnings` 干净。
- 共享设置页：`apps/desktop/tests/settings/candidate-translation-options-section.test.tsx`、`translation-candidate-settings.test.ts` 的新用例在改动前失败；全套 vitest 860 文件 5494 项、两个 `tsc --noEmit`、`format:check` 通过。
- macOS：`shortcut-test` 的 `TestHighlightedTranslationScope` 与 `candidate-row-layout-test` 的 `TestVerticalReservedGlossRow` 改前失败、改后通过，完整 `shortcut-test` 在 Studio 上通过。
- iOS：Studio 上专用模拟器跑相关五个测试类（`MSIMEKeyboardTests` 59 项、`MSIMESharedTests` 11 项）通过，其中经真实 Rust 会话的 `testOfflineGlossesStayOnEveryRowWhileTranslationIsScopedToTheHighlight` 在离线释义不带 `gloss` 时失败。
- Android：`check-host.sh` 的 278 项 JVM 冒烟通过，新冒烟改前编译失败。
- HarmonyOS：`platforms/harmony/tests/run.sh` 通过（调度 20 项、2in1 宽度用例），ArkTS 编译通过。
- Windows：`translation_gloss_query.cpp`、`candidate_card_size.cpp` 用本机 clang++ 改前失败、改后通过；`translation_reserved_presentation.cpp`、`windows7_settings.cpp` 和改过的宿主源文件只做了 MinGW 语法检查，完整交叉构建、WinUI 设置编译和 Win7 设置测试由 PR CI 承担。
- Linux：范围规则与两种画布的单测在本机用 clang++ 加桩库改前失败、改后通过，`fcitx5_contract.py` 通过；容器构建、ctest 和需要词库的端到端用例由 PR CI 承担。
- 没有任何平台在安装好的系统输入法里验收过，Windows 与 Linux 的候选窗、各原生设置页都没有截图验收，Android 没在模拟器上跑设置套件（没改资源文件）。
