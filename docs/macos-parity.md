# macOS 功能迁移对照

逐个源文件的去处清单在 [macos-feature-inventory.md](macos-feature-inventory.md)，来源 93 条测试断言的逐条核对在 [macos-assertion-audit.md](macos-assertion-audit.md)：那份是「一个不漏地列出来」，这份是「用什么方法比过、发现了什么」。

## 范围与固定基线

迁移的范围是 MSIME-Apple 的 macOS 完整功能，而不是设置页或能在某台机器上跑起来的子集。落地形态按四条原则：公共业务放共享层、公共管理界面放 Tauri；输入算法与组合状态归 C++ Engine；平台特性按 macOS 自身的机制适配，不照搬来源的实现形态；每一项的等价性由可重跑的比对而不是逐项人工确认来保证。

2026-09-20 本次对照使用以下不可变对象：

- 来源：`metasequoiaime/MSIME-Apple`，通过 `git ls-remote --symref origin HEAD` 确认默认分支 `develop`，远端 HEAD `663d7db619e45ebf3750db7c216438edd800f696`。实际读取的是本机检出 `63c51eddb5bc82c574ba7f8ca41db453e129db81`，它比默认分支多一个已提交改动（候选行宽适配：`CandidateRowFit.h`、`CandidatePanel.mm`、`CandidatePanelTests.mm`）。该检出 `git status --porcelain -- platforms/macos shared` 为空，未读取未提交内容；多出的那一项已由本仓库 #3029 覆盖。
- 目标：`metasequoiaime/msime` 的 `develop`，固定提交 `17ab45eb4234418faf2635fd6bf960d9c73f68f8`。

来源功能入口以该检出的 `platforms/macos/src/`（73 个文件）、`shared/apple-bridge/`、`shared/backend/`、`shared/backend-ui/` 交叉核对，并以 `PreferencesWindowController.mm`（3323 行）作为用户可见设置面的索引。索引只是入口，逐字段、逐动作的下钻在后面各节里逐条记录。

## 证据等级

下面《功能分组与目的地入口》一表里，每一项都标注了它的依据属于哪一档：

- **有调用链**：已找到目的地的真实调用代码。
- **有强制检查**：除调用链外，另有 CTest 或门内检查持续保证该项不回退。

比对过程中第三档是「明确缺口」——可由未消费配置、缺失运行时路径或实测结果直接证明的缺失。这一档现在是空的：每个被找出来的缺口都已修复并合入，下面按发现顺序逐条记录了是什么、怎么修的。

## 三次机械比对的结果

| 维度 | 方法 | 结果 |
| --- | --- | --- |
| 共享偏好 | 解析 `Preferences` 结构体全部字段，检查 macOS 宿主与共享运行时是否消费 | 82 项中 71 项有消费；其余 11 项必须在 `platforms/macos/tests/settings/preference_coverage.py` 的清单里写明平台为何不适用，清单双向校验 |
| 用户可见文案 | 抽取来源 macOS 全部源文件的中文字面量，去掉可访问性标签后逐条在目标中检索 | 623 条；未命中的均为措辞差异或已由 Tauri 页以不同表述覆盖，逐簇核实后无功能缺口 |
| 符号 | 抽取来源 ObjC 方法、C/C++ 函数、Swift 函数名，逐个在目标中按名与按文本检索 | 938 个；未命中项经逐簇核实后归为三类：目标以不同命名实现、按架构下沉到 Engine 或共享运行时、仅 iOS 使用 |

### 这三次比对看不见什么

上面三项都是**存在性**比对：某个符号、某条文案、某个偏好在两边都有。它们查不出「两边都有、但行为不同」的那一类，而这类恰恰是最影响实际使用的。按行为逐条核对后，确实找出两个：

- 组合中途单击 Shift，目标上屏的是高亮候选，来源上屏的是已输入的原始字母（#3179）。两边都有这个快捷键、都有对应偏好、符号也都在，只是做的事不一样——而这个手势存在的意义正是把词库里没有的词原样送出去。
- 以词定字与候选翻页可以被同时指到同一对键上，翻页先执行，于是显式绑定的以词定字在它自己的键上毫无反应（#3182）。两边都有这两个功能，来源用互斥开关避免了冲突，目标的原生 setter 也互相拒绝，但共享快照路径不做检查——而那正是 Tauri 设置页写入的路径。

有效的办法是**读来源注释里解释「为什么这样做」的段落**，再去目标里验证同一条理由是否成立。来源在这类非显然决策处几乎都写了理由，而理由是存在性比对抽不出来的信息。这条方法后来成了第四条比对轴，也是本仓库再遇到同类问题时该先用的那一条。

## 第四次比对：行为层（2026-09-20）

按上面那条方法做的一轮：读来源解释「为什么这样做」的注释，再回目标验证同一条理由是否成立。本轮覆盖与结论：

| 核对对象 | 结论 |
| --- | --- |
| 来源纯逻辑头（`InputModeRouting.h`、`InputControllerKeyRouting.h`、`WubiCommitPolicy.h`、`CandidateSelectionState.h`、`InputBehaviorPreferences.h`） | 找出 2 个缺口，已修；其余等价或目标更严 |
| 设置窗口侧栏 14 个分区 | 全部有 Tauri 归宿；「五笔」在目标是输入页内按方案条件显示的分区，不单列侧栏项 |
| 来源 macOS 全部源文件的中文字面量 | 643 条，精确未命中 204 条。滤掉以「卡片 / 行 / 页」结尾的可访问性标签后余 164 条，再滤掉有 5 字以上子串命中的余 151 条；从中挑出最像功能而非措辞的 5 条（主题模式、候选窗补充字体、离线优先、Whisper 模型、复制反馈报告）逐条直查，**全部已实现**，仅标签措辞不同 |
| 偏好键常量（来源 29 个 `k*Key`） | 按目标命名（snake_case / camelCase）逐条核对，全部有消费方 |
| 来源 74 个 macOS 源文件 | 按名未命中 12 个，逐个核实后均为命名差异或已下沉到共享层；`PersonalDictionaryStore` / `PersonalDictionaryView` 对应目标的共享词库 ABI + Tauri 词库页，且目标为超集（分页、按类过滤、查询、导入导出、失败重试） |

本轮修掉的两个行为缺口：

- **#3179** 组合中途单击 Shift：目标上屏高亮候选，来源上屏已输入的原始字母。两边都有快捷键、偏好与符号，做的事相反——而这个手势的用途正是把词库里没有的词原样送出去。
- **#3182** 以词定字与候选翻页可被指到同一对键上，翻页先执行，显式绑定的以词定字静默失效。来源用互斥开关规避，目标的原生 setter 也互相拒绝，但 `applySharedCandidatePreferences:` 不检查——而那是 Tauri 设置页写入的路径。

## 第五次比对：能力与产物（2026-09-20）

第四次按行为核对之后又补了一轮，针对的是「两边都有、但目标这边用户够不着」这一类。找出并修掉两个：

- **#3216** macOS 设置页隐藏了表情、颜文字、临时日语三个本地模式开关，理由写的是「预览包只发 msime-pinyin.db 和 msime-english.db」。但 `others.db` 与 `msime-japanese.dat` 自 `780a9381b`（2026-09-09）就在 `resources/desktop-dictionary.lock.json` 里，比那条过滤早十天，而 `tauri.macos.conf.json` 整目录打包已校验的资源集——三个能用的模式在设置里没有任何办法打开。同样受资源门控的「临时英文」一直显示着，这个不一致本身就说明前提错了。资源真缺时由 `apply_local_mode_resource_gates` 关掉该模式、触发键原样插入大写字母，比隐藏开关更好。
- **#3212** 本地 Whisper 模型只能手填绝对路径，而来源有 `browseVoiceModel:` 文件选择器。webview 的 file input 给的是内容不是路径，所以共享设置页答不了，改为向宿主要一个可选能力（`pickVoiceModelPath`），原生实现放在 `crates/host-macos/native/`，不引入新依赖；宿主不提供就不显示按钮，手填照旧。

本轮核过且确认等价或目标更强的：来源 12 个共享后端文件目标全有（另有 `BackendAiClient`）；四个原生视图（剪贴板、词库、账号、设置同步）文案差集为空；云词库备份视图逐字一致；输入法菜单条目集合一致；引擎选项写入面一致（来源的嵌套字段对应目标的扁平字段，自动纠错来源是一个总开关、目标拆成换位与邻键两项，覆盖引擎仅有的两个位）；`HostSurface` 各能力位 macOS 均已开启，唯一未开的 `number_row_selection` 来源没有该功能；`platforms/macos/tests/settings/preference_coverage.py` 的「不适用」清单双向校验、无陈旧项。

### 固定 Windows 来源的全拼纠错增量（2026-09-22）

上段“覆盖引擎仅有的两个位”后来被固定 Windows 来源的 Engine 提交推翻。目标锁定的共享 Engine 确实比早期来源锁更新，但 Windows 来源随后在 `3c2f3ae3`、`34c69fbe`、`c064cc64`、`49d0bf58`、`94abc08e`、`6673155c`、`3ee2ecdb` 上继续演进了全拼纠错：新增漏字/多字边、k-best 歧义切分、编辑代价分层、“纠错头 + 简拼尾”、ü 拼写别名标记、双拼置顶键修复以及热路径缓存。这说明“inventory 已登记”或“某一时点 Engine 更新”都不能替代逐提交行为核对。

真实锁定词库给出了可重跑的缺口证据：改动前 `gau` 只返回“噶”，没有进入 `gua/gai` 纠错；`hauzh` 只返回“哈”，没有形成 `hua'zh`。迁移后 `gau` 的便宜换位读音 `gua` 整组稳定领先昂贵邻键读音 `gai`，`hauzh` 可得到“华中”等候选。实现仍在 Engine 边界内：严格上下文 overlay 移植固定来源算法，大型纠错表由同一固定来源生成器从 Engine 自身合法音节表重建；patch 与生成器均进入 `engine-lock.json` 的内容摘要，冷 checkout 能得到相同源码。（2026-09-30 起 overlay、生成器与锁都已删除，同一套纠错规则在 `crates/engine/src/pinyin/autocorrect.rs` 里于首次使用时生成。）共享桥接的合成词库回归同时覆盖 `gau` 分层、`hauzh` 组合、`shng` 漏字、`sshang` 多字和 `nue` 别名轻标记；真实词库示例另行固定产品数据上的排序与候选。

### 固定 Windows 来源的中英混输默认值（2026-09-22）

固定对象 `467b9804` 的 `1cf27e2f` 已把出厂 `cn_en_mixed_input_min_chars` 从 2 改为 5。目标此前的审计读取过落后的可变分支，误写成“来源从未出现过 5”，又用门禁把共享默认和 Windows 模板一起钉在 2；macOS 原生设置却已经按 5 显示，因而空偏好下界面与共享运行时会分裂。现在共享偏好、Engine 默认选项、共享 React 与 Windows 模板统一为 5，macOS 原生设置无需再改；显式保存的 1–8 仍原样保留。`test-default-config-parity.py` 改为直接读取固定对象，以后不会再由可变 checkout 的历史描述覆盖真实基线。

## 符号比对的重跑（2026-09-20，双方当前 HEAD）

前面那次符号比对做在固定提交上，而两边此后都前进过，所以在来源 `63c51ed`（比之前的固定检出多出候选行宽适配等改动）与目标当时的 `develop` 上重跑了一遍，方法与结论都记下来，便于下次复核而不是重新发明：

- 从来源 `platforms/macos/src/` 抽出 ObjC 方法、C/C++ 函数与 Swift 函数名共 **572 个**，逐个在目标的 `platforms/macos`、`shared`、`crates`、`packages/ui/src`、`apps/desktop/src` 全文检索。
- 按名未命中 **173 个**。自动消解 `Metasequoia*` → `MSIME*` 的改名与 camelCase → snake_case 之后，余 **164 个**。
- 这 164 个**逐个查出定义它的来源文件**，按文件归属如下（合计 164）：

| 来源文件 | 个数 | 去向 |
| --- | --- | --- |
| `PreferencesWindowController.mm/.h` | 78 | 原生设置窗口。目标的设置面在 Tauri，按指令不该有对应物 |
| `DictionaryInstaller.mm` | 18 | 词库安装。下沉到引擎与共享 Rust |
| `MetasequoiaInputController.mm` | 15 | 目标当时也有同名文件，**逐条核对见下**（那份是从未参与构建的直连适配器，2026-09-30 随 C++ Engine 删除；下面的对应物都在产品控制器 `platforms/macos/src/input/InputController.mm`） |
| `PersonalDictionaryView.mm`、`PersonalDictionaryStore.mm/.h` | 14 | 原生个人词库界面与存储。目标是 Tauri 词库页 + 共享词库 ABI，且为超集 |
| `InputBehaviorPreferences.h`、`CandidateAppearancePreferences.h`、`CandidatePageSize.h`、`LocalInputModePreferences.h`、`FloatingToolbarPreferences.h`、`CandidateTranslationLanguage.h` | 16 | 原生偏好读写器。目标的偏好在共享层，由 `platforms/macos/tests/settings/preference_coverage.py` 双向校验 |
| `FloatingToolbarPanel.mm` | 4 | **逐条核对见下** |
| `CandidatePanel.mm/.h`、`CandidateSkinPreviewView.mm`、`InputModeHUDPanel.h` | 7 | **逐条核对见下** |
| `TranslationClient.mm/.h`、`CandidateGlossClient.swift`、`BackendAccountBridge.swift` | 6 | 翻译与账号桥接。目标是 `CustomTranslationBatch`、`BackendCandidateGloss`、`BackendAccountBridge` |
| `InputModeRouting.h`、`InputControllerKeyRouting.h` | 3 | 已在行为层比对中逐条核过 |
| `Uninstaller.h` | 2 | 卸载器五条规则已逐条核过 |
| `VoiceInputService.mm` | 1 | `beginCapture` → 目标的语音采集 |

- 「目标也有同名文件」的那 **28 个**（控制器 15、悬浮工具栏 6、候选面板 7）逐条对照，全部有对应物：`MetasequoiaTogglePinnedWord` → `MSIMETogglePinnedCandidate`；`availableGlossColumnsPrimary` / `setArmedGlossColumn` → `_armedGlossColumn` 与其夹取；`commitGlossAtVisibleOffset` / `insertGlossForModifiedDigit` → `commitCandidateGlossColumn:` 及其 Option/Control 分支；`handleSolitaryShiftFlags` → `MSIMEModifierTap` 与 #3179 的原样上屏；`translationDictionary` → `candidate_glosses_with_user`；`LocalModeOptionsMatch` → 目标比的是整个 `Preferences` 值；`toolbarWidth` / `visibleButtonCount` / `applyItemVisibility` → `_preferredSize` 与 `_appliedComponentMask`；`MetasequoiaIsUsableCaretRect` → `MSIMEValidCaret`。
- 唯一没有同名对应物且**确实不该有**的是 `rightMouseDown`：目标把候选菜单挂在按钮的 `menu` 属性上，右键由 AppKit 自带的 `menuForEvent:` 接手，而不是覆写鼠标事件——用的是框架自身的契约。
- 其中不属于上述两类、最像功能的一组逐个核过并都已覆盖：Option/Control + 数字取释义（`CandidateGlossRequestForModifiers` → `commitCandidateGlossColumn:`）、待上屏列（`setArmedGlossColumn` → `_armedGlossColumn` 与其夹取）、候选置顶（`MetasequoiaTogglePinnedWord` → `MSIMETogglePinnedCandidate` 与 `MSIMECandidatePinCode`）、释义调度（`scheduleCandidateTranslations` → `synchronizeCandidateGloss` / `synchronizeAccountGloss` 与空闲延迟）、反馈诊断（`copyFeedbackReport` → Tauri 反馈页的「复制报告」）、学习数据清除（`ResetMetasequoiaLearnedData` → Tauri「清除学习数据」→ `resetLearnedData` 能力 → `reset_learned_data` → 引擎）。
- 单点对照另外确认：`ActionForSolitaryShift` 对应 #3179 之后的单击 Shift 行为，`NextArmedGlossColumn` 对应 `cycleArmedGlossColumnBackwards:`，`browseVoiceModel` 对应 #3212 新增的 `pickVoiceModelPath`，`MetasequoiaCandidateWantsOnlineGloss` 对应 #3156 的 `MSIMEOnlineGlossCandidates`。

## 结论：macOS 迁移已完成

依据是五条各自独立、各自可重跑的比对轴**全部跑完**，每条找到的缺口都已修复并合入，没有剩余的已知未处理项：

| 轴 | 覆盖面 | 规模 | 产出 |
| --- | --- | ---: | --- |
| 参考窗口截图 | 设置界面的结构与文案 | 13 页 | #3247 侧边栏分组与帮助页、#3258 快捷键页 |
| 控制项标识 | 设置窗摆出来的每一个控件 | 23 | 无缺口 |
| 运行时偏好键 | 输入路径上真正读的偏好 | 27 | 无缺口 |
| 测试断言 | 来源用测试钉住的行为 | 93 | #3280 恢复默认设置、#3298 双拼方案菜单 |
| 符号 | 来源写了但没写测试的行为 | 518 | 无缺口 |

文件层另有一份全量清单：来源 `platforms/macos/src` 与 `shared` 的 111 个源文件逐个列出去处，见 [macos-feature-inventory.md](macos-feature-inventory.md)；断言逐条的状态见 [macos-assertion-audit.md](macos-assertion-audit.md)。两份都附了可重跑的抽取命令，表可以核对而不是只能相信。

**已知分歧共三处，都是刻意的，都有理由：**

1. **悬浮工具栏齿轮**可关闭（参考不可关）。目标的工具栏组件更多，把「设置」一并交给用户是一致的；关掉也不困人，输入菜单里仍有「水杉输入法设置…」。手写与语音按钮现在默认关（连同 Emoji 与屏幕键盘，见 `docs/windows-parity.md` 的《工具栏可选按钮改为默认关》），所以整条工具栏可以只剩下兼作拖动柄的 logo——齿轮关掉之后能回到设置的路径只有输入菜单那一条。
2. **标点模型**：参考是两个互斥的钉住标志加一个跟随态，目标是一个状态加独立的智能标点开关。目标这侧更能表达。
3. **翻译 provider 模型**：参考是单选，目标是三家各自独立配置。目标是超集。

另有一处方向相反的差集：屏幕键盘、手写识别板、AI 辅助与 AI 对话、社区资源与皮肤、打字统计、悬浮工具栏皮肤编辑、双拼键位提示面板、输入模式 HUD——目标有而来源没有。

**这个结论覆盖的是什么：** 五条轴覆盖文件、符号、偏好键、用户可见文案与来源测试断言这五层，判据是「所有能机械重跑的比对方法都已用尽且无剩余缺口」。它不声称逐字节等价——来源既没写测试、也不体现在文件或符号层的那一类手感差异（某个延迟、某个动画的观感）只能在实际使用中发现。遇到这类问题按单个缺口处理即可，不需要重开一轮比对。

## 两条跨平台取舍的归位

曾经挂着的两条跨平台取舍，各自归位如下：

- **语音整理的请求预算**：是 macOS 缺口，已修（#3224）。目标此前沿用 Windows 的 3 秒总预算，而来源实测 3 秒下整理「永远来不及返回」并使用 30 秒；配合 `HTTPVoiceRequest.mm` 吞掉失败的写法，结果是转写已经发给服务商、清理后的答案每次都被丢弃、界面毫无提示。现在预算是带默认值的参数，默认仍是 Windows 的 3 秒，只有 macOS 显式传 30 秒——Windows 行为一字未动。代价也一并写在提交里：整理与转写在同一条路径上，慢的服务会推迟文字上屏本身，这与来源的取舍相同，且好过「发出去再把回答扔掉」。
- **离线释义的查询顺序**：**不是 macOS 迁移缺口**。`msime_client_candidate_gloss_request` 是所有宿主共用的 C ABI，`candidate_glosses_with_user` 的顺序在 macOS、Windows、Linux、HarmonyOS 上完全一致，macOS 并不落后于本产品的任何宿主。与来源的差异是整个产品层面的一个刻意选择：引擎把顺序设计成构造参数（custom_translations → 随包 → 联网缓存），来源直接用四参数构造，而目标另开 `translation-glosses.db` 先查，并由 `crates/engine/src/host/tests.rs` 的 `unsafe_learned_glosses_fall_back_to_packaged_values`（原在已删除的 `crates/engine-bridge/src/tests.rs`） 明确断言「learned 优先于随包」。

  此处此前记过一条「用户手写的释义会被自动学来的盖过、需要引擎侧改动」——**那是错的**，实测推翻：不改一行桥接代码，用户写的条目就已经排在最前。路径值得写下来：`translation-glosses.db` 与设置页写的 `custom_translations.txt` 同在用户目录下，而 `EnglishDictionary` 在没有显式 translations 路径时会读取数据库旁边的 sidecar，于是 learned 那个对象本身就带着用户手写的条目，`query_*_gloss` 又先查 custom。所以优先级是「两个文件恰好同目录」带来的涌现性质：挪动其中任何一个，或给 learned 传一个显式 translations 路径，都会把用户的释义静默降到最后。`crates/engine/src/host/tests.rs` 的 `hand_written_glosses_outrank_learned_and_packaged_ones`（原在已删除的 `crates/engine-bridge/src/tests.rs`） 现在固定住了这一条。

另有一条记录需要收紧。#3182 把「以词定字占用的键不参与翻页」写成宿主侧要处理的一个可达状态，实际不是：`crates/client-core/src/preferences.rs` 的 `validate()` 在 `word_character.enabled` 与对应翻页键同时为真时返回 `ConflictingKeyBindings`，而保存（`preferences.validate()?`）和读取（`snapshot.preferences.validate()?`）两条路径都会调用它——带着这个组合的偏好文件根本加载不进来，设置页也存不下去，`key_conflict` 就是它在界面上的那句提示。所以宿主里那段排除是防御，不是在修一个用户能走到的状态；两个布尔项看着独立，共享层已经把互斥钉死了。

宿主使用的输入源标识（`app.msime.inputmethod.MetasequoiaIME`）在系统里已登记，`platforms/macos/scripts/install.sh` 安装后中文模式注册并启用，可以直接从输入菜单选中使用。读注册结果有两个坑：替换 bundle 之后会有分钟级的一段时间查不到、之后自行恢复，按那段时间里的读数下结论会错；另外某个模式已经是 disabled 时，进程启用不了它，这既不是安装失败也不是重新登录能解决的事。判据与测量过程见下面的《输入源注册的读数怎么看》和《模式启用不了不是安装失败》两节。

## 功能分组与目的地入口

以下路径均相对目标仓库根目录，只写文件名的短路径相对 `platforms/macos/src/`；“来源入口”相对固定的来源检出。

| 功能组 | 来源入口 | 目的地证据 | 结论 |
| --- | --- | --- | --- |
| IMK 事件路由、候选面板、输入源注册 | `MetasequoiaInputController.mm`、`CandidatePanel.mm`、`InputSourceRegistration.mm`、`InputControllerKeyRouting.h` | `platforms/macos/src/input/InputController.mm`、`candidate/CandidatePanel.mm`、`input/InputSourceRegistration.mm`、`input/InputControllerPhysicalKeys.h` | 有调用链；目标另行处理来源未覆盖的小键盘数字与标点物理键 |
| 候选翻页、以词定字、方向键导航 | `MetasequoiaCandidateKeyOptions`、`ClassifyConfiguredControllerKey` | 共享 `NavigationPreferences`、`WordCharacterPreferences`；宿主逐项消费 `minus_equal`/`comma_period`/`brackets`/`tab`/`page_up_down`/`arrows`/`mouse_wheel` | 有调用链；来源的互斥开关在目标是各自独立的布尔项，互斥由 `Preferences::validate()` 保证 |
| 设置界面 | `PreferencesWindowController.mm` | 主编辑器为 Tauri `packages/ui/src/index.tsx`；原生回退 `platforms/macos/src/settings/AppearancePreferences.mm`、`platforms/macos/src/voice/VoiceProviderSettings.mm` | 有强制检查（`preference-coverage`、`settings-route-coverage`、`voice-provider-settings-keys`）；按「公共 UI 放 Tauri」重构形态，非逐窗复刻 |
| 语音输入 | `VoiceInputService.mm`、`VoiceSettings.mm`（云端 + 本地 Whisper） | `platforms/macos/src/voice/`：豆包流式、HTTP 批量（OpenAI、Groq、SiliconFlow、EveryAPI、Mistral）、macOS 系统识别、本地 Whisper（#3014）；`shared/voice/VoiceProviders.cpp` 提供 `recognize_local_asr` | 有强制检查（`bundle-contents` 校验可执行文件确实链接了本地识别器，`http-voice-controller` 校验 provider 路由与会话代次） |
| 候选释义与翻译 | `TranslationClient.mm`、`CandidateGlossClient.swift`、第二语言、Option/Control 取列上屏 | `platforms/macos/src/core/CustomTranslationBatch.mm`、`platforms/macos/src/cloud/TranslationCache.mm`、`commitCandidateGlossColumn:`、共享 `translation_secondary_language` | 有调用链；目标另有腾讯、NiuTrans、账号释义、离线优先与 macOS 26 系统离线翻译补位 |
| 智能标点 | `PairedPunctuation.h`、重复标点转中文 | 共享 `punctuation::route` 消费 `direct_digit`/`direct_letter`（#3075）；空格回转 ASCII（#3081） | 有强制检查（`smart-punctuation-space`） |
| 词库与用户词条 | `DictionaryInstaller.mm`、`PersonalDictionaryStore.mm`、`PersonalDictionaryView.mm` | `dictionary/DictionaryInstaller.mm`、`dictionary/DictionaryWindowController.mm`；词条增删改查与导入导出在 Tauri 词库页；导出文件由 Tauri `save_export` 写进「下载」文件夹，重名时按浏览器的 `name (2).txt` 规则另起，页面提示写入的完整路径（WKWebView 在没有下载处理器时取消下载链接，来源 WebView2 的下载在这里由宿主完成） | 有调用链；`shared/export_file.rs` 单测覆盖文件名校验与重名规则 |
| 学习数据清除 | `ResetMetasequoiaLearnedData` 及其标记/恢复协议 | `crates/host-api/src/dictionary.rs` 的 `Operation::Reset`，经 `DictionaryAccess::try_maintenance` 加锁后交 `msime_engine_bridge::reset_learned_data` | 有调用链；按「输入算法与词库归 Engine」下沉，宿主不再自建标记恢复协议 |
| 软件更新 | `UpdateController.mm`（Sparkle 2.9.6） | 共享 About 页检查本仓库发行版；`core/UpdateController.mm` 仅在应用 bundle 配置 `SUFeedURL` 时启动 Sparkle，无 feed 的原生降级会说明限制并经用户确认打开固定的官方发布页；非应用进程不显示更新 UI | 有强制检查（`update-controller` 覆盖三种路由、确认、取消与打开失败） |
| 卸载 | `Uninstaller.mm` | `crates/host-macos/native/uninstaller.mm`，`shared-uninstaller` CTest | 有强制检查 |
| 输入菜单图标、本地化、TCC 权限 | `MetasequoiaIMEMenuIcon.tiff`、`render_menu_icon.swift`、`Info.plist` 用途字符串 | `platforms/macos/resources/MSIMEClientInputMethodMenuIcon.{svg,tiff}` 与三个模式带角标的 `MSIMEClientInputMethodMenuIcon{Chinese,Japanese,English}.tiff`、`platforms/macos/scripts/render_menu_icon.swift`（#3021）；语音识别用途字符串及其本地化（#3015、#3052）；输入源名称的本地化键与 bundle id 配对 | 有强制检查（`info-plist-icons`、`info-plist-usage`、`info-plist-names`、`bundle-contents`） |
| 账号、云剪贴板、云词库、快照、社区 | `shared/backend/*.swift` | `shared/backend/` 为来源的超集（另有 `BackendAiClient.swift`），并带 Swift 测试 | 有调用链 |

## 目标具备而来源没有的部分

屏幕键盘、手写识别板、AI 辅助与 AI 对话、社区资源与皮肤、打字统计、悬浮工具栏皮肤编辑、双拼键位提示面板、输入模式 HUD。这些不属于本次迁移范围，此处只说明两侧差集不是单向的。

## 第六次比对：设置界面（2026-09-20，按参考窗口的截图）

前五次比对走的是代码与产物，看不见界面本身长什么样。这一次的输入是参考窗口十三个页面的截图，比对的是结构而不是像素。三条差异落地，其余页面目标是超集，不动：

- **侧边栏分组**（#3247）。参考按四组排列——输入类、外观类、数据类、帮助类，组间留空白不画线。目标此前是一条平铺列表。macOS 现在按参考分组，本客户端有而参考没有的页面（社区、AI 辅助、打字统计等）单独成组保留在帮助组之前，没有为了对齐截图把功能删掉。其他平台仍是原来的平铺列表。
- **帮助页改成词条式**（#3247）。参考回答的是三个问题——怎么打字、候选旁边的英文是什么、为什么输入菜单里没有——每条左边术语右边说明，而不是整段散文。照此重写，顺带补上此前没有的数字键 1–9、翻页、Option+Shift+H、离线优先、需要账号、两种语言、词库没有更新。
- **切换提示与全半角快捷键**（#3258）。参考把「切换中英文时显示提示」放在快捷键页紧挨 Shift 那一行，目标放在输入页，已挪过去且输入页不再重复。Option+Shift+H 在目标宿主里从上线起无条件占用、没有交还给应用的办法；新增 `keybindings.toggle_fullwidth_option_shift_h`（默认开，行为不变）并只门控这一个组合键——`IsFullWidthInputToggle` 同时匹配的 Ctrl+Shift+Space 是 Windows 宿主也保留的那个，设置页没有提它，照旧生效。macOS 上那几行的键名也按 Mac 键盘改写为 Control / Option。

还有一处差异当时留了下来：反馈页附带的诊断信息，参考写的是系统版本与输入方案序号，目标写的是平台与 User-Agent，因为给出真实的 macOS 版本号需要宿主再开一个能力。这项已在 `1ae180f90` 补上——`apps/desktop/src-tauri/src/lib.rs` 的 `macos_product_version()` 填入 `os_version`，反馈页优先报它。

## 控制项与运行时键的两次机械比对（2026-09-20）

第六次比对看的是截图，只覆盖参考窗口画出来的东西。补两条能机械跑、不依赖截图的：

- **控制项标识**。参考 `PreferencesWindowController.mm` 里所有 `.identifier = @"…"` 共 23 个，逐个在目标的共享偏好与设置页里找对应项，全部有着落。命名从 camelCase 换成 snake_case，对应关系是：`edgeSelection`→`word_character.enabled`、`perApplicationMode`→`ime_mode_scope`、`outputScript`→`traditional_chinese_output`、`mixedEnglish`→`mixed_input.english`、`defaultEnglish`→`default_ime_mode`、`repeatPunctuation`→`smart_punctuation_repeat`，其余同名。
- **运行时偏好键**。参考在输入路径上读的 `MetasequoiaInput{Flag,Integer,String}(@"…")` 共 27 个，同样逐个有对应。只有 `alwaysChinesePunctuation` / `alwaysEnglishPunctuation` 不是一对一：参考用两个互斥的「钉住」标志加一个跟随状态，目标用 `chinese_punctuation` 这一个状态加独立的 `smart_punctuation` 开关。目标这一侧更能表达（参考没有「钉成英文标点同时开智能标点」这种组合），不是缺口。

两条都没有找到缺失项，这是第六次比对之外对「无已知缺口」这句话的又一次独立交叉检验。

## 第三条比对轴：来源的测试清单（2026-09-20）

控制项标识和运行时偏好键这两条比对，看的都是「参考的设置窗口摆出了什么」。还有一层它们都看不见：参考在测试里钉住的行为契约。把 `platforms/macos/tests` 下 93 条 `require(...)` 的断言说明逐条读过来，对着目标找对应实现，这是找出真缺口最有效的一条轴——前两条都没有发现的东西，它发现了一个。逐条结果见 [macos-assertion-audit.md](macos-assertion-audit.md)。

- **「The settings footer restore button was not found.」→ 真缺口，已补（#3280）。** 参考每一页底部都有「恢复默认设置」，目标只有「保存设置」。补的时候有一处必须反着做：参考清的那串偏好键里没有翻译和语音服务，因为在那边密钥不在这份文档里；在这边密钥就在文档里，所以 `Preferences::restored_to_defaults` 是从 `Default` 出发把服务配置搬回来，而且 endpoint / provider / model / 本地模型路径跟着密钥一起搬——留一个密钥指着默认 endpoint 比两个都留或都清更糟。Rust 侧的测试不逐字段列密钥，而是把恢复后的文档序列化出来找哨兵串，以后加了新密钥字段却忘了搬会被它挡住。
- **输入菜单**当时逐项对齐过 Apple 来源（中文输入 / 英文输入 / 简体输出 / 繁体输出 / 表情与符号… / 检查更新… / 水杉输入法设置… / 开始或结束语音输入 / 语音输入设置…），之后按 MSIME-Windows 的托盘菜单重排：现在是 中文输入 / 英文输入 / 英文候选模式（⌃⇧E）/ 繁体输出 / 全角字符 / 中文标点 / 显示译文 / 输入方案（当前方案）▸ / 主题（当前主题）▸ / 悬浮工具栏 / 水杉表情面板… / 云剪贴板… / 水杉屏幕键盘… / 手写输入… / 开始/结束语音输入 / 水杉输入法设置… / 关于水杉输入法…，其中与 Windows 托盘菜单的七项（悬浮工具栏、表情/符号面板、手写识别板、屏幕键盘、语音输入、设置、关于）一一对应，检查更新与语音设置收进设置窗与悬浮工具栏。现状以 `platforms/macos/tests/input/ShortcutTest.mm` 对 `-[MSIMEInputController menu]` 的断言为准。
- **「New apps must use the default input mode.」** 已实现：`platforms/macos/src/settings/AppearancePreferences.mm` 里没有记忆的应用回落到 `defaultImeMode`。
- 其余关于卸载、更新控制器、候选面板、五笔自动上屏、学习数据清除确认的断言，逐条都有对应实现。

## 一处刻意的分歧：悬浮工具栏的齿轮

参考的 `MetasequoiaFloatingToolbarItemKeys()` 只有四项——中英文切换、中西文标点、全角半角、简繁输出——齿轮不在其中，并且有一条测试专门钉住「四个开关全关，齿轮还在」。目标把齿轮也做成了可开关的一项，还多出表情与屏幕键盘两项。

这里不跟。目标的工具栏组件本来就比参考多，把「设置」一并交给用户控制是一致的；关掉齿轮也不会把人困住——输入菜单里的「水杉输入法设置…」照样能开设置窗口。手写和语音不再恒常存在：它们与 Emoji、屏幕键盘一起改成了默认关（见 `docs/windows-parity.md` 的《工具栏可选按钮改为默认关》），每个按钮都能关掉，工具栏可以只剩兼作拖动柄的 logo（`FloatingToolbarPanel.mm` 在整行为空时隐藏分隔线、保留 logo，面板因此仍可拖动）。记在这里是因为它确实是一处已知的、刻意的行为差异，不该被上面那句「无已知缺口」盖过去。

## 输入源注册的读数怎么看（2026-09-20 实测）

安装不需要重新登录，原因是 #3270 之后宿主继承了系统已登记的 `app.msime.inputmethod.MetasequoiaIME`。判据本身仍然成立：**一个在本次登录会话开始时不在输入源列表里的 bundle identifier，无论 bundle 内容如何都进不去**；换句话说，更新同一 identifier 可以原地生效，换一个新 identifier 则要等下次登录。

实测：从 `e215ba7be` 构建、`platforms/macos/scripts/install.sh` 安装之后，`platforms/macos/scripts/check_input_source.swift` 报

```
app.msime.inputmethod.MetasequoiaIME.Hans: enabled
app.msime.inputmethod.MetasequoiaIME: enabled
```

连续 15 次查询全部命中。要注意刚替换 bundle、刚调用 `--register-input-source` 之后有一小段时间查询会时有时无——同一条命令隔几秒跑，会先报 not in the registry 再报 enabled。所以 `install.sh` 只查一次就下结论是不稳的，两个方向的误判都可能出现；判断安装结果时应多查几次再看。

**查询结果会在替换 bundle 之后消失一段时间，再自己回来——按这段时间里的读数下结论是错的，我错了两次。** 同一天的经过：装好之后第一次查报 enabled，几分钟后查报查不到，再过一阵连查 15 次全部命中；之后在 worktree 里裸跑了一次 `cmake --build` 又删掉构建目录，查是 0/12，于是写下了「被顶掉了，只有重新登录能恢复」——这条是错的，再过一段时间之后，不做任何操作、没有重新登录，连查 15 次又全部命中，直接枚举 TIS 也能看到两条都 enabled、`.Hans` 可选。

所以读注册结果的规矩是这三条：

- 继承一个本次会话开始时就在列表里的标识（#3270）是它能进列表的前提。
- `TISCreateInputSourceList` 在 bundle 被替换或重新注册之后会有相当长一段时间（分钟级）按 bundle id 查不到任何东西，之后自行恢复，不需要重新登录。
- 因此**紧接安装之后的任何读数都不能用**——单次命中可能是上一份 bundle 的残留，单次查不到也可能只是还没回来。要判断就隔开一段时间再连查若干次，并且用直接枚举（`TISCreateInputSourceList(nil, true)` 再按 `kTISPropertyInputSourceID` 匹配）交叉对一下。

`install.sh` 在注册前会把指向其他路径的竞争记录 `lsregister -u` 掉，这一步裸跑 `cmake --build` 没有；构建产物确实会以同一个 identifier 被 LaunchServices 登记，所以构建完顺手 `install.sh` 重装是稳妥的做法。但「裸构建会把已装的那份顶掉、且只有重新登录能恢复」这个更强的说法没有证据支持。

## 模式启用不了不是安装失败（2026-09-27 实测，macOS 27.0 / 26A428）

上面那段的读数规矩管的是「查不查得到」，还有一种结果与它无关：查得到、已启用、能打字，但 bundle 的另一个模式是 disabled。这台机器上 `install.sh` 装完稳定报出三条——`.Hans` enabled、总源 enabled（不可选中）、`.Roman` disabled，隔 10 秒连查四次不变，所以不是上面那个分钟级窗口。

`.Roman` 加不回来，而且这一条不在本项目的控制范围内：类型为键盘输入模式（`TISTypeKeyboardInputMode`）的输入源无法由进程启用，`TISEnableInputSource` 返回 noErr 而模式仍是 disabled。三组对照把原因钉死在「输入源类型」上，而不是签名、bundle、plist 或那条登录会话限制：

- 本输入法的 `.Roman`：`TISEnableInputSource` 返回 0，再读仍 disabled。
- 苹果自己的 `com.apple.inputmethod.SCIM.Shuangpin`（父输入法已启用、同胞模式 `ITABC` 正在使用，形状与我们完全一致）：返回 0，再读仍 disabled。
- 同一会话里的键盘布局 `com.apple.keylayout.US`：返回 0，再读变成 enabled，`TISDisableInputSource` 也能还原。测完已还原为 disabled。

`--register-input-source` 因此会在这种机器上退 0 而模式没启用——系统不报错，所以退出码不能当判据。`check_input_source.swift` 为此退 2（可用但有源未启用），与退 1（不在注册表里，或没有任何可选中的源是 enabled 的）分开；`install.sh` 按退出码选提示，不再对这种情况打「注销重新登录」——那条提示对它不成立。英文模式缺失时输入法的行为见 `platforms/macos/README.md` 的《输入源菜单与输入模式》：控制器不请求这个不可选的模式，菜单栏保持「中」，中英切换照常工作。

bundle 自身是否装配正确由 `bundle-contents` 持续检查，不依赖安装：图标是否真的暂存进 `Resources`、菜单图标是否带 16×16 与 32×32 两页、每条用途字符串是否在每种已暂存语言中都有、可执行文件是否链接了本地识别器。

## 这条判据是怎么测出来的

更早的时候这一项记为「需要签名产品包」，是错的。实测（见 `platforms/macos/README.md`）：把同机注册正常的那份输入法复制一份、只换 bundle identifier、用同一张 Developer ID 证书重签后注册，失败方式完全一样——`TISRegisterInputSource` 返回 noErr 而 `TISCreateInputSourceList` 查不到。真正的判据是**该 identifier 在本次登录会话开始时是否已在输入源列表里**：当时唯一能被列出的那份，其安装时间早于本次会话的开始；重启 `imklaunchagent`、`lsregister` 重新登记与重扫用户域都无效。所以那不是装配缺陷，签名产品包也绕不过去——换一个新标识仍然进不去。#3270 走的是这条判据的另一半：继承那个已登记的标识。

## 基线状态

macOS 的 `ctest --test-dir <build>` 是门禁而不是一份总在红的名单：`scripts/known-failures.txt` 里没有任何 macOS 条目，任何一条失败就是失败。清单曾经挂着 `local-mode-preferences`、`text-client`、`shortcut` 三条，一并记为「引擎答案与测试断言不一致」，逐条查下来没有一条是引擎：分别是测试没有提供被资源门控的可选词库、测试断言的恢复路径在组合期守卫加入后已不可达、以及一长串因套件提前中止而从未执行过的陈旧断言。

## 不回退的保证

以下检查在 `ctest --test-dir <build>` 中运行，新增偏好、新增 TCC 调用、图标改名或本地识别器被关掉都会直接失败，而不是等到安装后才发现：

- `preference-coverage`：共享偏好要么被 macOS 宿主消费，要么在清单中写明不适用的理由。当前是 82 项中 71 项有消费、11 项写明理由，清单双向校验，所以既不会漏掉新偏好，也不会留下陈旧的豁免。
- `settings-route-coverage`：设置页的页面清单在 TypeScript 里，路由词表在 Rust 里，中间没有东西连着；这一项把两边对起来，少一个分类的页面等于没有宿主能打开它。
- `bundle-contents`：产物 bundle 的图标、本地化与本地识别器链接。
- `info-plist-icons`、`info-plist-usage`：模板 plist 的图标引用与 TCC 用途字符串（含每种语言的本地化）。
- `info-plist-names`：输入源标识、输入菜单里显示的名字，以及源码里按标识办事的那些地方。三者分在 `Info.plist.in`、每种语言各一份的 `InfoPlist.strings` 和 Rust/Objective-C 源码里，中间没有任何东西连着，所以改 bundle id 会留下一组键名仍然合法、却挂在已不存在的标识上的本地化（菜单于是把标识本身画出来），以及一批仍按旧标识校验安装包、启动进程、读写偏好域的字面量。检查三向：plist 声明的每个标识在每种语言里都有名字，`.strings` 里每个标识形状的键都对应一个 plist 仍在声明的标识，源码里每个输入法标识形状的字面量也都是 plist 当前声明的那个。
- `voice-provider-settings-keys`：原生语音窗口的每个文本字段都对应一个运行时读取的默认键。
- `entitlements-guard`：签名用的权限文件不含受限权限（`com.apple.developer.*` 整族都需要 provisioning profile 背书，Developer ID 签名给不了，AMFI 会在 exec 时拒绝启动，表现为输入法从输入菜单里消失而签名本身校验正常），且 `install.sh` 拿到这样一份文件时会在动任何东西之前拒签。对应来源侧 #465 在发布打包脚本里的同名拦截。
