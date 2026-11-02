# Agent Note: 两套原生设置页补齐共享偏好新增开关

Status: implemented

## Problem

parity 批次的各个 feature 分支给共享偏好加了字段并在共享 React 设置页提供了开关，但 macOS 原生 AppKit 设置窗口（`AppearancePreferences.mm`）和 Windows 原生 WinUI3 设置窗口（`settings/main.cpp`）没有对应控件，原生入口的用户看不到也改不了这些偏好。

## Decision

两套原生窗口各按自己的既有模式补六个开关，只写共享文档里的契约键，不重复定义 client-core 字段——字段与引擎行为由各 feature 分支在 parity 批次里提供（`local_modes.symbol` 来自 feat/symbol-mode，`wubi_z_wildcard` / `wubi_fifth_code_top_commit` 来自 feat/wubi-input-switches，`ai_assistant.english_candidates` 来自 feat/issue-9015-english-ai-candidates；`sentence_association.neural_desktop` / `show_next_on_duplicate` 已在 client-core）。`PreferencesStore::save_locked` 经 `restore_unknown_fields` 在保存往返中原样放回本版本不认识的键，所以原生窗口先落地写出的键不会丢，批次合齐后立即生效。

macOS 侧：

- `local_modes.symbol`（「符号快捷输入（S 模式）」）进 `LocalModeControls()` 颜文字之后，缺键读作开，随「Shift+字母进入快捷模式」总开关一起禁用；探针、合并、推送全部沿用表驱动，不用另写。
- `wubi_z_wildcard`（缺省关）、`wubi_fifth_code_top_commit`（缺省开）照 `wubi_auto_commit_unique` 的平铺键模式：defaults 键 + `_shared*` ivar + `SharedOverrideProperties` + `SettingProbes` + `applySharedInputPreferences:` 读值 + `sharedPreferencesByMerging:` 总是写当前生效值，放在输入方案页五笔卡片末尾两行。
- `sentence_association.neural_desktop`、`show_next_on_duplicate`（均缺省关）用同样的平铺键模式，放在词库与数据页「云端与智能候选」组；合并时只写这两项，`word_lattice`、`neural_keyboard` 和将来的字段随传入文档原样保留。恢复默认值后靠 `_sharedInputPreferencesApplied` 条件把缺省写回文档，避免旧值下次载入复活（与 `shuangpin_keymap_hint` 同一处理）。
- `ai_assistant.english_candidates`（「英文模式也请求 AI 候选」，缺省关）放进独立的「AI 联想设置」窗口（`MSIMEAISettingsWindow`），勾选框加进表单并在 `MSIMEAISettingsMerge` 白名单（七字段改八字段）登记；这是 macOS 上 ai_assistant 的唯一原生入口，对应共享页 AI 页「联想」组。

Windows 侧：本次没有做，上游当时在 `platforms/windows/settings/main.cpp` 里重写云候选与 AI 两页的结构，Windows 的这几行按新结构另开一支加。五笔两项由 #10696 补上：报告人在 Windows 上关掉「五笔四码唯一候选自动上屏」后，第五码顶字照样上屏，以为开关失效，而 Windows 两个原生窗口都没有能解释并改变它的那一行。

- WinUI 窗口在 `if (scheme == L"wubi")` 分组的「五笔快捷选词」之后、「五笔四码唯一候选自动上屏」之前加 `wubi_z_wildcard`（缺省关）、`wubi_fifth_code_top_commit`（缺省开）两个 `bool_row`，顺序、标题、说明与共享页 `WubiSection` 相同。
- Win7 表单（`win7/SchemeForm.h`）在「候选显示剩余编码」之后加两个复选框（`scheme_wubi_z_wildcard`、`scheme_wubi_top_commit`，插在 `scheme_cantonese_method` 之前，`command` 的 ID 区间不用改），`valid`、`refresh`、`enable`、`BN_CLICKED` 四处与兄弟开关同表绑定；复选框下面照抄共享页的两句说明，因为单看标题猜不出边界（通配会停用 `z` 反查，顶字关掉只在混输开启时生效）。其余五笔复选框维持只有标题。
- `scripts/test-windows-wubi-settings-copy.py`（contracts job 自动发现）逐句比较共享页的开关与 WinUI 的 `bool_row`（顺序、标题、说明、偏好键、出厂缺省），并核对 Win7 每个开关都有同标题的复选框、四处绑定同一个键。文案三处各写一份，与 `test-windows-autocorrect-settings-copy.py` 守全拼纠错组的做法相同。
- Server 不用改：两个键早已经共享 host-api 下发（`lib.rs` 把 `wubi_z_wildcard` 交给 Engine 的五笔选项，`set_wubi_fifth_code_top_commit` 交给 runtime）。

`local_modes.symbol`、`ai_assistant.english_candidates` 在 Windows 原生窗口的情况不在 #10696 的范围内。

## Alternatives considered

- **Windows 把五笔组改成从共享页的表生成**：文案只写一份，不会再漏。没有采用：WinUI 窗口的每一行都直接写在 `main.cpp` 里，Win7 表单是 Win32 控件，两边都没有读共享表的机制；为五笔一组先造这套机制，比照全拼纠错组用比对脚本守住文案代价大得多。
- **Win7 表单只加复选框、不写说明**：与已有的混输、剩余编码、四码唯一三个复选框一致。没有采用：#10696 正是因为用户不知道顶字关掉只在混输开启时生效才误报，标题「第五码将首选上屏」说不出这一点。

- **在本分支把四个缺失字段一并加进 client-core（serde 字段 + 缺省值）**：字段很小且能让开关在本基线上立即被运行时读取。没有采用：那四个字段连同引擎行为、React 控件是各自 feature 分支的完整交付，这里再写一份同样定义会在 parity 批次合并时产生逐字节相同的重复定义（或无意义的冲突）；「一个 issue 一个 PR」也要求字段归字段的 PR。未知键经 `restore_unknown_fields` 保存不丢，UI 先行是安全的。
- **sentence_association 用字典键（像 `MSIMEClientLocalModes`）而不是两个平铺键**：字典键能天然保留 `word_lattice` 等兄弟字段。没有采用：平铺键与 `autocorrect_transposition`/`autocorrect_neighbor`（同属嵌套对象的平铺先例）一致，代码更少；兄弟字段靠「合并进传入文档字典」保留，同样安全。
- **macOS 的英文 AI 候选开关放主窗口「云端与智能候选」组**：可以走 probe/restore 体系。没有采用：`ai_assistant` 的全部字段归 `MSIMEAISettingsWindow` 管、经 `MSIMESavePreferenceEdits` 直接写文档，开关放主窗口会造出第二条写 `ai_assistant` 的路径。

## Consequences

- 收益：原生窗口与共享 React 页在这六个偏好上达到 parity；「恢复默认值」和各分区「这一节是否被改过」的判定自动覆盖新键（探针表驱动）。macOS 的 LocalModesKey 探针、`applySharedLocalModes:`、`sharedPreferencesByMerging:` 因表驱动对新模式零改动生效。
- 代价：本基线上 `local_modes.symbol`、`wubi_z_wildcard`、`wubi_fifth_code_top_commit`、`ai_assistant.english_candidates` 四个键在 client-core 尚无字段，开关写出的值在 feature 分支合入前是惰性的（保存往返不丢、不报错）。Windows 原生侧的五笔两项由 #10696 补上，`tests/ui/windows7_settings.cpp` 点这两个复选框，核对读出与写回；WinUI 的两行由文案比对脚本守住，UI Automation 验收没有新增。
- 验证：`AISettingsSnapshotTest` 对 `english_candidates` 的合并断言在加入白名单前 abort、加入后通过（本机 clang 直编验证）；`LocalModePreferencesTest` 的快捷模式按钮计数 10 → 11 并覆盖 `symbol` 行（窗口类测试需图形会话，本机未跑，供 CI 验证）；`AppearancePreferences.mm` 过 `clang++ -fsyntax-only`。
