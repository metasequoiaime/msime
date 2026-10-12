# Agent Note: 五笔反查：`z` 引导临时拼音与设置页查码

Status: implemented

## Problem

#6789：五笔用户想用拼音找到一个字，同时看到它的五笔编码。引擎早就会给五笔方案里的候选附上反查编码（`ImeSession::refresh_wubi_codes` 查 `wubi86`/`wubi98` 里最长的码，`host/session.rs` 把它接在注释位上），拼音候选也一样附码，但拼音只能从「五笔拼音混输」进来。完整版默认关掉混输，关掉以后五笔方案里没有任何键能打出拼音：`WubiScheme` 拒绝 `z`，其余字母都被当成五笔码，最多四个。

## Decision

混输关闭时，五笔组字开头的 `z` 打开临时拼音反查，默认可用，没有单独的开关。

- **为什么是 `z`**：五笔 86 和 98 的码都只用 `a`–`y`，`z` 在五笔方案里没有任何用途。引擎里没有五笔万能键或学习键：`WubiScheme` 在混输关闭时直接拒绝 `z`，交回宿主原样输入。极点五笔、QQ 五笔和 Rime 的五笔方案都用 `z` 引导拼音反查，用户的手感和文档也都按这个来。
- **只在组字开头**：`z` 只有在组字为空时引导反查。五笔码中间的 `z`（`wq` 之后）照旧拒绝、交回宿主。混输打开时 `z` 本来就是拼音字母，所以 `ImeSession::bind_wubi_scheme` 让反查和混输互斥：`set_pinyin_lookup_allowed(!mixed_pinyin)`。
- **状态放在组字里，不另起本地模式**：反查组字仍是 `SchemeType::Wubi` 的组字，原文就是键入的 `z...`，`local_mode` 保持 `none`。`WubiScheme::in_pinyin_lookup` 只看「允许反查且原文以 `z` 开头」，退格删掉 `z`、取消、上屏都自然结束反查。`ImeSession::decode` 遇到反查组字时不查五笔码表，把 `z` 后面的字母交给混输共用的 `QuanpinScheme`，只出全拼候选。反查组字最多 32 个字母，与混输的上限相同。
- **编码提示照旧**：反查的候选都是全拼行，组字仍在五笔方案、`local_mode` 是 `none`，所以 `refresh_wubi_codes` 和 `host/session.rs` 现有的规则直接给它们附上当前码表版本（86/98）的五笔编码。单字显示的是最长的全码（你 → `wqiy`，不是简码 `wq`），沿用 `REVERSE_QUERY_SQL_86`/`_98` 的取法。
- **选词按拼音推进**：选走一个只覆盖一部分拼音的候选后，剩下的拼音仍在反查里。`InputSession::selection_completes_composition` 和 `advance_composition_after_selection` 只在 `z` 后面那段拼音里计算消耗了多少字母，再把 `z` 接回剩余原文前面；不接回去，剩下的 `hao` 会被当成五笔码。回车（`CommitRaw`）和没有候选时结束组字（`finish`）上屏的是键入的拼音，`z` 与临时英文的 `Y` 一样只是标记，不上屏。
- **不学习**：反查只是查编码。反查里的选择不调词频、不记整句（`learn_candidate` 直接返回），多段选择也不造词（`commit_selection` 的 `store_user_phrase_from_canonical_pinyin` 跳过）。五笔码表从来不会因为反查写入；拼音词库也不会，否则一个五笔用户偶尔查一次字，就会改掉他在拼音方案里的候选顺序。
- **宿主不用改**：快照里反查就是一个五笔组字，`answered_by_pinyin_fallback` 为真，所以 `input-runtime` 的四码顶字（`wubi_four_code_is_complete`）和四码唯一自动上屏都不会对 `zhao` 这样的四个字母动手。各宿主本来就把五笔里的小写字母交给引擎，由引擎决定收不收，所以 Windows、macOS、Linux、Android、iOS、HarmonyOS 不需要各自加一份判断。

## 汉字查码的数据通路

另一半是「已经有汉字，想知道它怎么打」。`msime_engine::host::wubi_code_lookup`（实现在 `crates/engine/src/wubi/lookup.rs`）给一段文字列出每个不同汉字在 `wubi86` 和 `wubi98` 两张表里的全部编码，短码在前（你 → `wq`、`wqiy`），整段是 2 到 16 个汉字时再把整段当一个词查一次。它读会话用的同一份五笔码表（`wubi_database` 选出的文件），所以用户自己加的五笔词也查得到；一次最多 64 个不同汉字，超出时标 `truncated`。

宿主经已有的词库请求入口拿到它：`dictionary_request_json` 新增只读操作 `{operation:"wubi_codes",text}`，`personal_dictionary_request_json` 给出同样的回答。

- **走词库请求，不另开 C 接口**：设置页在每个宿主上都已经有一条词库请求通路——桌面和 iOS 的 Tauri 命令 `dictionary_request`、Android 的个人词库队列入口、HarmonyOS 设置页的 `dictionary` 桥方法（经 `KeyboardSession.dictionary` 到 `msime_client_dictionary`）。新操作沿着它们走，不需要给 C ABI、JNI、NAPI 或 Harmony 的 `registerJavaScriptProxy` 方法表加任何东西。Harmony 的 `DictionaryMaintenancePolicy` 把不认识的操作当作只读，所以查码在组字时也能用、不会停掉键盘会话。
- **不拿维护租约**：码表以 SQLite 只读方式打开，维护进行时读到的是替换前或替换后的完整文件。其余只读操作用 `DictionaryAccess::try_session` 是因为它们读用户日志和词库代次；查码只读码表，拿租约只会让它在维护时报忙，也会让 Android 设置进程里的一次查码挡住键盘进程的维护。
- **Android 不进队列**：Android 设置页把所有词库操作送进个人词库队列（`personal_dictionary_request_json`），队列是为了等键盘空闲再写词库。查码不写任何东西，所以在入口直接回答，不排队，也不建 `PersonalDictionary` 目录。
- **上限**：`text` 超过 4096 字节报 `invalid wubi lookup`。粘贴一整篇文章时只列前 64 个不同汉字，结果里标出截断。

## 设置页的「五笔查码」

共享设置页的「输入」页在五笔的设置显示时（当前方案是五笔、触屏方案里启用了五笔，或只有五笔的版本），在方案组之后画一个「五笔查码」组（`packages/ui/src/settings/wubi-code-lookup-section.tsx`）：一个文本框、一个「查码」按钮和一张表，表里先列整段词（有的话），再逐字列出 86 和 98 两列编码，当前所选码表那一列标「（当前）」，码表里没有的写「—」。说明文字同时告诉用户打字时可以按 `z` 加拼音查。

共享设置页的输入文字一变化，就作废尚未完成的查码请求，清掉旧结果和错误，并允许对新文字立即查码；过期请求即使晚于新请求返回也不再回写。否则用户把「你好」改成「世界」后，会继续看到「你好」的编码，旧请求未结束时还无法查询新文字。

页面只经 `DictionaryClient.wubiCodes` 拿结果；宿主没有提供它时整组不画。各宿主的接法：

- macOS、Linux：Tauri 设置应用的 `dictionary` 客户端（`apps/desktop/src/main.tsx`）发 `dictionary_request`，到 `dictionary_request_json`。
- Android：分发的产品是 `build-apk.sh` 打的原生宿主（`release-android.yml`），没有 WebView，共享设置页那一组到不了用户手里，所以原生设置的「输入」页（`TypingPage`）在启用了五笔时加一组「五笔」，里面一行「五笔查码」打开原生的 `WubiLookupPage`：文本框、「查码」按钮和结果卡片，每行是文字和 86、98 两列编码，文字与排法照共享设置页（`WubiCodeLookupPolicy`）。查码在设置应用进程里经 `NativeClient.personalDictionaryRequest` 发 `wubi_codes`，`personal_dictionary_request_json` 在入口直接回答、不排队，只读打开安装时解出来的 `msime-wubi.db`。Tauri 合包（`build-client-apk.sh`）的共享前端仍经 `mobileDictionary` 带上 `wubiCodes`，走同一个入口。
- HarmonyOS：`apps/harmony/src/main.tsx` 的 `dictionary` 客户端经桥方法 `dictionary` 到 `KeyboardSession.dictionary`，再到 `msime_client_dictionary`。
- Windows：设置窗口是原生 WinUI 的 `msime-client-settings.exe`（`platforms/windows/settings/main.cpp`），自己不画查码组，但同目录的共享应用 `MSIME.exe` 画「输入」页，它用的就是上面 macOS、Linux 那个 `dictionary` 客户端，`dictionary_request` 在 Windows 上同样到 `dictionary_request_json`。所以 WinUI「输入」页的五笔组末尾加一行「五笔查码」，和辅助码插件那一行一样用 `nav::shell_links::input` 打开共享应用的「输入」页。这一行不能省：五笔版只提供五笔，辅助码组不显示，原来本窗口没有任何一行通到共享应用的「输入」页。
- iOS：分发的产品是原生 `MSIMEApp`（`platforms/ios/README.md`），没有 WebView，承载不了共享设置页。「输入设置」（`InputSettingsView`，`platforms/ios/App/Sources/app/OnboardingView.swift`）的「更多」组在启用了五笔时，三个五笔开关后面加一行「五笔查码」，打开原生的 `WubiCodeLookupView`，文字和排法照共享设置页（`WubiCodeLookup`）。查码在 App 进程里经 `MetasequoiaInputSessionBridge.wubiCodes`：先 `msime_client_prepare_host` 解析选项，再 `msime_client_dictionary` 发 `{operation:"wubi_codes",text}`；资源目录取 `PersonalDictionaryBridge.packagedResources`。App 包里只有词库清单，找到的是键盘扩展包里的 `EngineResources`，和键盘准备宿主用的是同一个目录：状态目录里的资源校验标记（`verified-resources.json`）记着目录路径，App 若用别的路径准备，键盘下次启动会把整套词库重新哈希一遍。共享前端的 iOS 工程（`apps/desktop/src-tauri/gen/apple`）仍经 `mobileDictionary` 带上 `wubiCodes`，但那个工程只是公共组件，不作为产品分发。`z` 反查在 iOS 键盘上照常可用，它在共享引擎里。

Android 原生页的 `WubiLookupRequestState` 也按输入文字和请求代次保存忙碌状态及结果：编辑时清掉旧表格、允许新查询，旧回答不再覆盖新结果；视图销毁时解除未完成请求的忙碌状态，已经完成的结果继续留在 ViewModel 中。`HostTask` 只向发起请求时那棵视图交回结果，旋转后不会调用旧完成回调，因此只靠回调解除 `busy` 会把新视图的查码按钮永久锁住。

iOS 原生页同样用 `WubiLookupRequestState` 管理输入、请求身份和显示状态：`TextField` 的文字变化先保持原有长度截断，再清掉旧表格与失败提示，并解除旧请求占用；完成回调只有身份仍属于当前输入时才写回。状态留在 SwiftUI `@State`，不跨导航页面保存。

字根拆分不做：仓库里没有授权清楚的 86/98 拆分数据（`resources/helpcodes` 里只有拼音辅助码用的字根数据），而且很多字根不是 Unicode 字符，要另配字体或图片。

## Golden 基准

三个场景原本记录的是参考实现「五笔里 `z` 被拒绝」的行为，现在按新行为更新：`wubi_mixed_plain_mode` 和 `wubi_mixed_matched_code` 在混输关闭时键入的 `zi` 记录为反查组字 `zi`；`wubi_select_and_gates` 把 `z` 的拒绝挪到五笔码中间（`wqb` 之后）检验，四码、第五码和撇号的检验不变。`tools/engine-golden/gen_scenarios.py` 同步改了这两个场景的步骤与说明。

## Alternatives considered

- **新增一个 `LocalInputMode`（像 `R` 临时日文那样切换方案）**：最强的理由是隔离得干净，预编辑可以显示成独立的模式，选词和学习各走本地模式那一套，不碰五笔组字的代码。没有采用：`LocalInputMode` 的名字是跨进程契约，六个宿主都有各自的模式标签、打字统计和中文输出策略按名字分支（Linux `CandidateLocalModeLabels.h`、Android `LocalInputMode.java`、Harmony `LocalInputMode.ts` 等），新增一个值要在每个宿主补一遍；本地模式的选择总是整段上屏，也就没法选走一部分拼音再接着反查。留在五笔组字里，宿主侧一行都不用改，现有的编码提示规则直接生效。
- **只告诉用户去打开「五笔拼音混输」**：不改代码。没有采用：混输会让每个五笔码后面都跟着拼音候选，打不出的码还会延长到拼音；只想偶尔查一个字的五笔用户不该为此改变整个输入手感。
- **加一个反查开关**：可以让不想要的人关掉。没有采用：`z` 在混输关闭的五笔里原本没有任何作用，占用它不会挡住任何五笔码；开关要进共享偏好、账号同步、MCP 偏好和六个宿主的设置界面，而它保护的只是「误按 `z` 时原样输入一个 `z`」，这时按退格或回车（回车上屏的是 `z` 之后的字母，单独的 `z` 什么也不上屏）即可。真有用户要求关掉，再按 [四码唯一自动上屏开关](2026-10-08-wubi-auto-commit-unique-preference.md) 那条路接进共享偏好。
- **查码给一个新的 C 接口（`msime_client_wubi_codes`）**：语义更直接，调用方不用知道它和词库请求共用一个入口。没有采用：每个宿主都要为它接一遍 JNI、NAPI、Tauri 命令和 Harmony 桥方法表，而词库请求通路已经在所有宿主上通到设置页，多一种只读操作只是多一个 `operation` 值。
- **反查里照常学习**：选过的拼音词下次排前面，查同一个字更快。没有采用：学习写进的是拼音词库，五笔用户在反查里的一次选择会改掉拼音方案（和混输）里的候选顺序；反查查的是编码，不是输入习惯。

## Consequences

- 混输关闭的五笔用户按 `z` 加拼音就能看到候选和它们的五笔编码，跟随所选的 86/98 码表；混输打开的用户行为不变。
- 代价：混输关闭时，五笔里单独按 `z` 不再原样输入 `z`，而是开始一个反查组字。反查的预编辑显示键入的原文（`znihao`），没有拼音分词。反查不支持撇号分隔，这一点与混输相同。
- 反查的拼音候选按拼音词库原有的顺序排列，不受个人学习影响，也不留下学习记录。
- iOS 和 Android 的产品设置里各有一个原生的「五笔查码」页（见上文两条），文字和排法照共享设置页，改一处要同时改三处：`wubi-code-lookup-section.tsx`、`WubiCodeLookupPolicy.java`、`WubiCodeLookup.swift`。Windows 经 WinUI「输入」页的一行跳到共享应用里查。

## Verification

- `cargo test -p msime-engine --lib`：`wubi::scheme` 新增三条（开头 `z` 引导、反查上限 32 与四个字母不是完整码、中间 `z` 仍拒绝、退格离开；不允许时 `z` 被滤掉；宿主编辑整段保留反查原文），`host::tests` 新增六条：反查附码且单字是全码、跟随 98 码表、进入与离开（退格、取消、离开后五笔码照常）、混输里没有 `z` 反查、选走一部分后剩余拼音仍在反查且回车只上屏拼音、反查不学习（并以混输里同样的选择会写日志作对照，证明测得到学习）。不学习那条在去掉 `learn_candidate` 的判断后失败。另有 `a_multi_pick_in_the_pinyin_lookup_creates_no_phrase`：打开学习，在反查里分两次选出码表没有的 `你号`，用户日志里没有这个新词；混输里同样分两次选会造出它，作为对照。去掉 `commit_selection` 里 `!self.engine.wubi_pinyin_lookup_active()` 那个条件后这一条失败。
- `cargo test -p msime-engine --test golden`：更新后的三个场景与其余场景全部通过。
- `cargo test -p msime-engine --lib -- wubi::lookup`：每个字全部编码短码在前、重复编码并成一个；非汉字跳过、重复的字只列一次、带标点的一段不当词查；码表没有的字列出但没有编码；缺 `wubi98` 表或整个文件时只有那一列为空；超过 64 个不同汉字截断。
- `pnpm --filter @msime/desktop exec vitest run`（全量 524 个文件通过），其中 `tests/settings/wubi-code-lookup-section.test.tsx`：整段词在前、两列编码与当前码表标记；没有汉字和截断的提示；失败时报错并清掉旧结果；「输入」页在五笔方案且宿主有 `wubiCodes` 时画出这一组并用它查，全拼方案或宿主没给 `wubiCodes` 时不画。
- 查码输入变化和过期响应由 `tests/settings/wubi-code-lookup-section.test.tsx` 覆盖：编辑立即隐藏旧表格、允许新请求，旧请求晚返回也不覆盖新结果。
- Android 原生页的请求状态由 `WubiLookupRequestStateSmoke` 覆盖：编辑后新旧请求逆序完成、视图销毁后旧回调失效且新视图可重试、已完成结果跨视图重建保留。`check-host.sh` 的 216 个 JVM 冒烟与设备套件源码编译、原生 Gradle 工程的 `compileFullDebugJavaWithJavac` 通过；此改动尚未在模拟器上运行。
- iOS 原生页的 `WubiLookupRequestStateTests` 覆盖编辑后旧成功、旧失败回调失效，以及新请求能立即启动；这两项先在空实现上失败，再在修复后通过。iOS 17 模拟器 SDK 的 SwiftUI 类型检查和 XcodeGen 工程生成通过；此批改动尚未在原生 App 模拟器运行，本机剩余空间不足以完成 Rust 与 Xcode 全量构建。
- Windows 那一行（`platforms/windows/settings/main.cpp`）没有在本机构建：WinUI 设置窗口只能用 MSVC 编译，交给 CI 的 Windows 构建；它照搬同一文件里辅助码插件那一行的 `shell_row(..., nav::shell_links::input)`。
- `platforms/harmony/tests/run.sh`：`DictionaryMaintenancePolicy` 把 `wubi_codes` 当读操作（组字时允许、不要维护、不进队列）。
- `cargo test -p msime-host-api --lib -- dictionary::tests::wubi`：两个入口对同一请求给出相同回答，个人词库队列不建目录、用户数据目录不变；超长文字两个入口都拒绝。
- 后续（Android、iOS 原生查码页）：Android 在专用 AVD 的 API 35 和 API 28 上跑了新增的 `WubiLookupDeviceSmoke`：启用五笔、选 98 码表后「输入」页出现「五笔查码」，查「段舞」两个字的 86、98 编码与随包词库一致（段 `wdm wdmc` / `th thdc`，舞 `rlg rlgh` / `tgl tglg`，98 列标「（当前）」），「输入法」整段有一行，没有汉字时提示；API 35 上 `smoke.sh` 全套另外跑过（`CaretCandidatesDeviceSmoke` 在机器负载高时丢了第一个键，重跑通过，与本改动无关），API 28 上 `smoke.sh --core`。`WubiCodeLookupPolicySmoke` 在 `check-host.sh` 里钉住排法和文字。iOS 在 iOS 27 模拟器上跑了 `WubiCodeLookupTests`（App 进程的同一条调用读测试宿主里的随包词库，段、舞、输入法与 Android 相同）和 `OnboardingUITests.testWubiCodeLookupListsBothTables`（真 App：选 98 五笔、打开「五笔查码」、查「段舞」，读屏标签与上面一致）；`MSIMEClientUITests` 不在 CI 里跑，只在本机跑过这一条。
