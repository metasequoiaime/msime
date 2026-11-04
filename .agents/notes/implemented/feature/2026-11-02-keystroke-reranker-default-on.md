# Agent Note: 整句联想默认「增强」，挂不挂逐键重排由每台设备自己量

Status: implemented

## Problem

逐键重排（「增强」，偏好 `sentence_association.neural_keyboard`）在 2026-10-29 只挂 runtime 那一层之后仍默认关（见[「增强」只挂 runtime 逐键重排器](2026-10-29-enhanced-runtime-reranker-only.md)），理由是 Pixel 3 XL 上随包 `model-v1` 让按键 p95 涨到 191 ms。ONNX 后端落地后（见[按键重排的 ONNX Runtime 后端](../architecture/2026-11-01-onnx-rerank-backend.md)），同一台手机用 6x256 整键 p95 9.3 ms，Mac、iPhone 17 用 8x448 约 4 ms，已经在逐键预算（整键 p95 ≤ 10 ms）之内。维护者决定「2.0 默认开」：用大模型还是小模型由每台设备自己决定，用户什么都不用设置。

只改默认值有三个缺口：

- **旧文档。** `Preferences` 每个字段都序列化，已发布的六个版本写下的文档（`crates/client-core/tests/fixtures/released-preferences/`）里都存着当时默认的 `"neural_keyboard": false`。`serde` 的默认值只对缺键生效，老用户升级后仍是「标准」，改动只对新装生效。
- **跑不动的设备。** 默认开以后每台设备都会挂重排器。没有 ONNX 模型的构建（HarmonyOS 没有 ONNX Runtime 产物，32 位安卓和 Win7 不在精简库的目标里，各桌面平台的 ONNX 接入还在各自的 PR 上）退回随包 `model-v1`，它在 Pixel 3 XL 上重排开销 p95 36 ms，默认开会让老手机快打落后、丢键，与 2026-10-07 `b8d1cb20c3` 是同一类问题。ONNX 的大小两档只在两者之间选，小模型本身从不检查。
- **桌面没有开关。** 共享 React 设置页和 Windows 原生设置页只有「桌面神经联想」（`neural_desktop`，停顿重排），桌面用户没法打开或关掉逐键重排（Windows PR #10673 上发现的）。

## Decision

### 默认值与一次性迁移

- `SentenceAssociationPreferences::neural_keyboard` 默认 `true`，缺键时按开读（`#[serde(default = "enabled_by_default")]`）。
- 迁移照[智能标点默认开](2026-10-16-smart-punctuation-default-on.md)的做法：`Preferences` 新增内部标记 `keyboard_reranker_migrated`（`#[serde(default, skip_serializing_if = "is_false")]`，`Default` 为真）。`PreferencesStore` 读入（`read_locked_keeping_unknown`）和修复损坏文档（`salvage_preferences`）时调用 `Preferences::settle_keyboard_reranker_default`：没有标记的文档，词网格开着就把 `neural_keyboard` 打开；词网格关着是用户在三档里选了「关闭」（三档选择器把两个开关一起写成关），不动。不论哪种都标上标记。只改读出来的那一份，不写盘、不动修订号，文件等下一次真正的保存才带上新值和标记。
- 标记只增不减：`save_locked` 里 `current` 带着它就给要写的偏好也标上，`settings_document::import_document` 取本机和文件标记的或，`salvage_preferences` 在源文档没有它时从垫底值里去掉。之后用户选回「标准」，保存的文档带着标记和 `false`，再读不会被改回「增强」。
- 账号同步不映射 `neural_keyboard`，不受影响。导入默认改变之前导出的设置文件时照文件的值来（可能是旧默认的「标准」），本机标记保留，与智能标点的处理相同。
- 读原始文档的原生代码跟着改缺省值：Android `ExpressionPage.sentenceLevel`、iOS `SentenceAssociationPreference.level`（以及 `ExpressionSettingsView` 的初值）、共享页的 `sentenceAssociationLevel` 与 `sentenceAssociationControlState`，缺键都按「增强」读。它们读的快照都经过 `PreferencesStore`，迁移后的值会写在里面，缺键的回退只在快照里没有这一项时起作用。

### 挂不挂由设备自己量

host-api 的 `ffi::keyboard_reranker` 在开关打开时决定挂哪一个、挂不挂：

| 情形 | 规则 |
| --- | --- |
| 编进了 `onnx-rerank`，大小两档都在 | 内存够就加载 8x448 自测一页重负载，≤ `LARGE_RERANK_MODEL_BUDGET_MS`（10 ms）用它；否则换 6x256，6x256 量同一页，≤ `KEYSTROKE_RERANK_LIMIT_MS`（15 ms）才挂，超过就什么都不挂 |
| iOS 可用内存不到 120 MiB | 直接用 6x256，同样过 15 ms 的上限 |
| 只装了一档（Android 只有 6x256 的资源包） | 量那一档，过 15 ms 的上限才挂 |
| ONNX 模型在、但挂得上的那一档超过上限 | 不挂，诊断写 `keyboard reranker over budget`，**不**退到随包 `model-v1`：它在同一台设备上只会更慢 |
| 没编进 `onnx-rerank`，或 ONNX 模型一个都读不了（HarmonyOS、32 位安卓、Win7、还没接 ONNX 的桌面构建） | 随包 `model-v1` 量同一页（`ffi::calibrate_bundled`），≤ 15 ms 才挂，否则不挂 |
| 版本不带键盘模型（五笔版等，`edition.features.neural_keyboard` 为假） | 不挂，设置页也不列「增强」 |

- **同一页、两条线。** 一页重负载是 `OnnxScorer::calibrate` 那一页：32 字上文、9 个 6 字候选、37 个新前缀，先编码上文只量按键那一步，预热一次取 3 次中位数。随包模型的 `calibrate_bundled` 用同样的形状和随包字表里都有的 64 个常用字，每次换一个不带前缀缓存的 `Reranker`。10 ms 那条线决定用大的还是小的（不变），15 ms 这条线决定挂不挂。
- **为什么是 15 ms。** 这一页是长句尾部那种按键，比平常重。2026-11-02 在同一台机器上量，它是真实运行时里重排开销 p95 的 1.4 到 1.7 倍：Pixel 3 XL（骁龙 845，Android 12）上 `model-v1` 一页 60–68 ms、重排开销 p95 36 ms；6x256 四线程一页 9.8–10.4 ms、重排开销 p95 6.9 ms（整键 9.3 ms）。所以「一页 ≤ 15 ms」大致对应「重排开销 p95 ≤ 10 ms」。直接拿 10 ms 卡这一页，Pixel 3 XL 的 6x256 会在线上来回跳，而它的整键 p95 本来就在预算之内。
- **每个模型在进程里只量一次。** ONNX 各档的结果按模型目录和 `lm_step.ort` 的长度、修改时间记在 `OnnxRerankers::calibrated`；量过超了的那一档之后不再加载。随包模型按路径记（`bundled_calibration`），同一路径的模型在进程里只加载一次。随包模型第一次就超过上限两倍时不再量下去，老手机上只多卡一次（Pixel 3 XL 上预热加一次约 250 ms）。
- **小模型和只装了一档时用 `calibrate`，不用 `calibrate_for_keystroke_budget`。** 后者会让打分器报 `scores_prediction_pool_within_keystroke_budget`，上屏后联想跟着改排；那条仍只交给过了 10 ms 换档自测的 8x448（见[上屏后联想按逐键打分器排序](2026-11-02-next-word-prediction-rerank.md)），这次不改。
- **这条门对用户选的「增强」也生效。** 默认开以后分不出「用户自己选的增强」和「默认的增强」，而且门要挡的正是这台设备上会让打字落后、丢键的那种按键；用户在这台设备上选「增强」得到的就是它跑得动的那一档，跑不动时与「标准」相同。诊断日志的 `keyboard reranker ...` 行说明挂了哪一个、量出的一页毫秒数和上限。
- 量到的设备与结论：Mac（M 系列）`model-v1` 一页 6.2–6.4 ms，挂；Pixel 3 XL `model-v1` 一页约 60 ms，不挂；Pixel 3 XL 6x256 约 10 ms，挂；Mac、iPhone 17 的 8x448 过 10 ms 换档线（4–7、3.8–4.8 ms），挂大的；iPhone XR 8x448 约 22 ms，换小的。

### 桌面设置

- 共享 React 设置页（macOS、Linux）在「输入 → 候选与联想」的「桌面神经联想」之前加一行「整句联想」，说明「更准确，但更耗电」，选项「标准」「增强」，与 iOS、HarmonyOS 手机同名同文案，只写 `neural_keyboard`（`KeyboardRerankRow`，读法 `keyboardRerankLevel`）。没有「关闭」：手机上的「关闭」在桌面是「更多选项」里的「本地整句联想」。不带键盘模型的版本不列这一行。
- Windows 原生设置窗口（WinUI，`build_sentence_level_settings`）在同一位置加同一行，自动化 id `sentence-association-level`；Win7 原生设置窗口在「本地整句联想」下面加同名下拉框（控件 `sentence_association_level`）。两处都按新的版本宏 `MSIME_EDITION_NEURAL_KEYBOARD`（`edition_windows.py` 从 `features.neural_keyboard` 生成）决定列不列。
- 三个桌面实现都不随「本地整句联想」变灰：host-api 的 `keyboard_reranker_enabled` 不看词网格，词网格关着时重排器照样给单词候选排序，变灰却仍然生效的控件会误导。
- 手机端（iOS、Android、HarmonyOS）本来就有三档选择，只改缺省显示。

- **停顿时重解的宿主上，跑不进逐键预算不等于不挂。** 宿主在 HostOptions 写了 `pause_rerank`（鸿蒙的键盘）时，ONNX 模型一页超过 4 ms 就延后到停顿、分片重解，加载上限放宽到 60 ms，诊断写 `keyboard reranker at pause`；其他宿主照本篇的规则。见[跑不进逐键预算的鸿蒙设备，把模型延后到停顿](2026-11-04-harmony-pause-rerank.md)。

## Alternatives considered

- **`Option<bool>`，`None` 跟随产品默认**（`chaizi: Option<bool>` 的做法）。最强的理由是三态能区分「没动过」和「选了标准」，以后再翻默认值也不用迁移。没有采用：旧文档里已经写着 `false`，`Option` 解决不了这些文档，迁移标记照样要加；改成 `Option` 还要动所有读这个字段的原生代码、TS 类型和测试，换来的只是以后的方便。
- **只改默认值，不迁移旧文档。** 最强的理由是「已存的值从不重新解释」，迁移会打开一部分在默认改变前有意从「增强」退回「标准」的用户。没有采用：旧文档里的 `false` 绝大多数是从没动过的默认值（桌面从来没有这个开关，手机上「增强」一直是可选项），不迁移的话这次改动只对新装生效；有意选「标准」的人再选一次就永久生效。
- **迁移时不看词网格，一律打开。** 更简单。没有采用：词网格关着只可能是用户在三档里选了「关闭」（或桌面上关了「本地整句联想」），那是明确不要整句联想的信号，打开逐键重排违背它。
- **用户显式选「增强」时绕过预算门。** 最强的理由是尊重用户的选择，老手机上想要模型的人也能用上。没有采用：默认开以后分不出显式和默认；而门挡的是会让打字落后、丢键的设备，正是 2026-10 撤回「增强」默认挂载的那类问题。
- **直接拿 10 ms 卡这一页。** 与换档线同一个数，规则更整齐。没有采用：Pixel 3 XL 的 6x256 一页 9.8–10.4 ms，会在线上来回跳，而它的整键 p95 9.3 ms 本来就在预算之内。
- **按平台或机型名单决定挂不挂（例如 HarmonyOS、32 位安卓一律不挂随包模型）。** 不用在会话建立时量。没有采用：名单要人维护，HarmonyOS 手机的快慢差很多，Mac 上的随包模型明明跑得动；自测量的就是要决定的那件事，每个进程只量一次。
- **ONNX 小模型超过上限时退到随包模型再量一次。** 代码更少（不用区分「超了」和「没有」）。没有采用：随包模型在每台量过的设备上都比 6x256 慢，退过去只是多加载一个模型、多卡一次再得出同样的结论。

## Consequences

- **收益**：新装用户和没动过这个选项的老用户默认用上逐键重排，Mac、iPhone 17 用 8x448，iPhone XR、Pixel 3 XL 用 6x256，没有 ONNX 模型的 Mac 构建用 `model-v1`；跑不动的设备不挂，不会因为默认开而打字落后。桌面三个设置实现都能选「标准」「增强」。
- **代价**：
  - 默认改变之前有意从「增强」退回「标准」的手机用户，升级后会被打开一次，需要再选一次「标准」。
  - 会话第一次挂重排器时多量一页：ONNX 小模型几十毫秒；随包模型在 Mac 上约 50 ms，在 Pixel 3 XL 上预热加一次约 250 ms，然后得出「不挂」。都只发生一次（每个进程、每个模型文件）。
  - 跑不动的设备上「增强」与「标准」一样，设置页不提示这一点，只有诊断日志说明。
  - 15 ms 这条线是用一台老安卓、一台 Mac 的两个模型推出来的比例；量化或模型换代后要重跑 `calibrate`、`rerank_latency` 并更新 `KEYSTROKE_RERANK_LIMIT_MS` 旁的数字。
  - 迁移标记 `keyboard_reranker_migrated` 用掉了；以后再翻这个默认值要另立标记。
- **跟进**：Android 的 ONNX 接入（`feat/android-onnx-reranker`，还没合）把 6x256 做成资源包，原计划在设置里选成「增强」时下载。默认开以后大多数用户不会去选，那边要改成「增强」开着（包括默认）而资源包没装时提示或自动下载，否则 Android 用户实际挂的是随包模型或什么都不挂。各桌面平台的 ONNX 接入（#10657、#10666、#10673）合入后，桌面默认挂的从 `model-v1` 换成 8x448。

## Historical audit

按 `neural_keyboard`、`增强`、`calibrate`、`默认` 检索活跃笔记：

- [「增强」只挂 runtime 逐键重排器](2026-10-29-enhanced-runtime-reranker-only.md)：部分被取代。「增强的缺省值仍是关」由本篇接管，其余（只挂 runtime 层、引擎层固定关、设置文案）仍然成立；已在那篇注明。
- [按键重排的 ONNX Runtime 后端](../architecture/2026-11-01-onnx-rerank-backend.md)：事实更新。大小两档之外多了挂不挂的上限、小模型也自测、诊断多一个 `over budget` 类别，已就地改。
- [上屏后联想按逐键打分器排序](2026-11-02-next-word-prediction-rerank.md)：事实更新。只装一档时现在也量一页，但量的是上限、不是换档自测，仍不排上屏后联想，已就地改。
- [九宫格的窄裁决点](2026-10-29-nine-key-model-contests.md)：部分重叠。它的「`增强` 关着（移动端缺省）时行为不变」和「升级前留下的手动第 1 槽当成自动的最近选择」两条，在「增强」默认开以后对老用户生效；它写下的重访信号「真机上『增强』默认打开时再看裁决点的 margin」由此触发，留给那篇的后续。已在那篇注明。
- [智能标点默认开](2026-10-16-smart-punctuation-default-on.md)：同一种迁移的先例，本篇照它的标记做法。
- [iOS 键盘的内存](../bug-fix/2026-11-02-ios-keyboard-reranker-memory.md)：无关。内存不够时用小模型的规则不变，小模型多量一页不改它的内存结论。
- 2026-10-27 两篇九键笔记里的「移动端默认 `neural_keyboard: false`」是当时测量条件的记录，不改。

## Verification

- `msime-client-core`（Studio，macOS）747 项全部通过，其中新加的 `keyboard_reranker_defaults_on_when_the_key_is_absent`、`keyboard_reranker_settle_turns_on_standard_but_keeps_off`（「标准」打开、「关闭」不动、带标记的 `false` 保留）、`released_documents_read_as_enhanced`（六个已发布版本的文档读成「增强」、文件不变）、`keyboard_reranker_migration_runs_once_and_an_explicit_standard_sticks`（旧文档读入时打开、写入方漏掉标记照样落盘、选回「标准」后再读仍是「标准」、恢复默认回到「增强」）、`keyboard_reranker_recovery_migrates_a_legacy_document`，以及 `settings_document::tests::an_old_file_keeps_its_sentence_level_and_this_machine_s_marker`。
- `msime-host-api` 1116 项全部通过；`--features onnx-rerank`（Studio 用 `fetch_onnxruntime.py` 取的 `aarch64-apple-darwin` 精简库）下 `keyboard_reranker_prefers_the_onnx_model_when_one_is_installed` 覆盖：只装大的时量上限不换档、超了返回 `OverLimit` 而不是 `Unavailable`；两档都装时大模型过不了换档线、小模型超上限时什么都不挂，量过的结论留着不再加载，上限放宽后按量过的数挂上小的；内存不够只能用小的时也过上限。`the_bundled_keyboard_reranker_is_attached_only_within_the_keystroke_limit` 覆盖随包模型在上限之内挂、0 ms 上限不挂、没有文件不挂；`keyboard_reranker_follows_the_keyboard_model_switch` 改成默认挂、选「标准」摘下、再选「增强」挂上。
- 一页耗时的真机数字来自 `crates/onnx-rerank` 的 `calibrate` 示例（交叉编译到 `aarch64-linux-android`，`adb` 推到 Pixel 3 XL 上跑）和一个与 `calibrate_bundled` 同页的一次性程序（未入库），Mac 是这台开发机。
- `apps/desktop` vitest 全部 885 个文件通过，新加 `sentence-keyboard-rerank-row.test.tsx`（桌面行只列标准和增强、缺键显示增强、选标准只写 `neural_keyboard`、五笔版不列、macOS 和 Linux 输入页都在「候选与联想」里列出它），`expression-harmony` 和 `sentence-association-preferences` 改成默认「增强」。
- iOS `SentenceAssociationSettingsTests` 改成缺键读「增强」、显式 `false` 读「标准」。Windows 的 `scripts/test-windows-input-habits-settings.py` 检查 WinUI 行的位置、版本门、键和文案；Win7 的 `windows7_settings.cpp` 加了下拉框的读写用例。WinUI、Win7 的 C++ 和 iOS、Android 的原生改动本机没有编译，由 CI 编译；没有在真实输入法里装机验证。
