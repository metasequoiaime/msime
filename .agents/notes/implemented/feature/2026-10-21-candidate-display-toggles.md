# Agent Note: macOS 候选显示补齐词性、单条释义与两类徽标

Status: implemented

## Problem

从旧版 Windows 正式版 `msime-windows@v0.11.1`（`dd018f79`）对照，macOS 把两项候选显示行为写死了：

- 候选释义总是画整条，词性标记（`n.`、`v.`）不可关，也不能只取第一条释义。共享偏好里没有对应字段，宿主把整条释义原样交给 `CandidateTranslation` 绘制。
- AI 联想候选的来源徽标写死 🤖，固定排位候选只靠蓝字辨认，没有角标、没有样式。Windows 出厂配置是 `candidate_fixed_badge = true`、`candidate_fixed_badge_style = "paperclip"`、`candidate_ai_badge_style = "robot"`，候选窗把徽标作为纯展示字段追加在候选文字后（`server/src/window/candidate_view_model.h`）。

这两项在本仓新架构 Windows 宿主里同样没有，属于 v0.11.1 遗留设置未搬；本次先在 macOS 补齐共同能力。

## Decision

新增五个共享偏好（`crates/client-core/src/preferences.rs`），出厂默认对齐 Windows v0.11.1 的 `installer/default_config/config.default.toml`：

| 键 | 默认 | 取值 |
|---|---|---|
| `candidate_translation_show_pos` | `true` | 是否显示释义开头的词性标记 |
| `candidate_translation_single_sense` | `false` | 是否只保留第一条释义 |
| `candidate_fixed_badge` | `true` | 固定排位候选是否加角标 |
| `candidate_fixed_badge_style` | `"paperclip"` | `paperclip` 📎 / `pushpin` 📌 / `dot` · |
| `candidate_ai_badge_style` | `"robot"` | `robot` 🤖 / `whale` 🐳 |

判定与选值抽成两个纯头文件，不依赖窗口，用不依赖图形会话的 CTest 验证：

- `platforms/macos/src/core/CandidateTranslationDisplay.h` 的 `format_translation_gloss_for_display` / `format_translation_for_display` 逐条移植 Windows 的 `FormatTranslationGlossForDisplay` 词性与义项规则，并按 macOS 的目标语言分行（`\n`）逐行套用。
- `platforms/macos/src/core/CandidateBadgePolicy.h` 的 `candidate_ai_badge` / `candidate_fixed_badge` 返回带前导空格的完整徽标串；AI 未知样式回退 robot，固定位未知样式不画角标。

宿主只改画法、不改上屏：释义在 `candidatePageGeometry:` 与 `candidateButtonFor:geometry:` 两处走 `CandidateDisplayTranslation`，`CandidateTranslation`（Ctrl+Enter 的释义页、Ctrl+数字的释义列）仍是未整理的原文；AI 徽标替换掉写死的 🤖，固定排位徽标追加在来源徽标之后，顺序与 Windows 的 `text + annotation + badge` 一致。

共享设置页只在 macOS 显示对应行：释义两行跟「候选词翻译」的释义来源开关联动，徽标三行放在外观页「窗口样式」分区。

两个切片分别落在 `09af8ebb4`（释义）与 `2e2f347ae`（徽标）。

### 旧笔记审计

- [候选释义分段与 Linux 原生画布配色](2026-10-20-candidate-translation-runs.md)：部分重叠。它把释义分段作为 `translation_runs` 给 Linux 画布消费；本篇做 macOS 的显示开关与徽标，不改共享分段格式。
- [新装默认关闭候选翻译](2026-10-15-new-install-candidate-translations-off.md)：无关。它定的是候选翻译总开关的默认值。
- [Windows 对齐 macOS 的遗留项](2026-10-10-windows-mac-parity-leftovers.md)：无关，只是同一类对照工作的早期记录。

## Alternatives considered

- **在共享 Rust 层做释义整理** — 分段规则已经在 `client-core::translation` 有一段（`translation_runs`），放那里各宿主都能用。但 Windows 与 Linux 目前都不消费这两个开关，把尚未搬的设置先塞进共享视图会扩大线上格式；macOS 只需要显示用的字符串，因此在宿主侧用纯函数做，等新架构 Windows 要搬时再上移。
- **固定排位继续只用蓝字** — 不加角标就不用改绘制路径，蓝字在 macOS 皮肤上已经能区分。但 Windows 出厂默认开着角标，且用户可在三种字形间选，缺少这两项就是可见的功能缺口。
- **让 AI 徽标跟随固定徽标样式** — 两个徽标共用一条展示路径，合成一个开关可以少两行设置。但来源徽标（🤖/🐳）和状态徽标（📎/📌/·）语义不同，Windows 也分两张配置表，合并会让用户无法只改其中一项。

## Consequences

- **收益**：macOS 用户能关掉词性、只看第一条释义，并给固定位和 AI 候选选角标样式；缺键的旧文档读成与 Windows 相同的出厂结果。
- **代价与已知上限**：Windows 的 `FormatTranslationGlossForDisplay` 还有 `first_synonym`（离线词典条目取首个逗号前的同义词），macOS 的候选快照不带「来自词典」的标记，宿主传 `false`，所以打开「只显示单条释义」时词典释义仍保留同一义项里的英文同义词。词性标签表与义项分隔符是移植的一份拷贝，Windows 改表时要一起改。
- 新架构 Windows 宿主还没有消费这五个键，共享设置页因此只在 macOS 渲染这些行。

## Verification

- `platforms/macos/tests/core/CandidateTranslationDisplayTest.cpp`、`platforms/macos/tests/core/CandidateBadgePolicyTest.cpp`：纯逻辑 CTest，先失败后通过。
- `crates/client-core/src/preferences/tests.rs` 的 `candidate_translation_display_defaults_follow_windows_factory_config` 与 `candidate_badge_defaults_follow_windows_factory_config` 钉住出厂默认、旧文档回退和显式选择往返。
- `apps/desktop/tests/settings/candidate-settings-layout.test.tsx` 钉住外观页的三行与保存 patch。
- `pnpm --filter @msime/desktop test`（784 文件 4849 用例）、`cargo test -p msime-client-core`、`cargo clippy -p msime-client-core --all-targets --locked -- -D warnings`、`pnpm format:check` 全绿；`shortcut-test` 目标编过，确认 `InputController.mm` 的宿主改动可编译。

审查整合：旧出厂徽标回归使用的 `CandidateDisplay` 包装仅定义在 `ShortcutTest.mm`，继续调用宿主的 `CandidateDisplayWithBadges` 与共享纯函数表。生产宿主传实际偏好，不保留没有调用方的出厂包装，也不依靠 `unused` 压制警告。整合作者后续修复时移除生产侧重复定义，避免控制器被测试直接包含后同一编译单元重定义。

快捷键测试的主控制器夹具通过直接设置 `appearance` 隔离系统输入源，因此绕过 `ensureAppearance` 的默认缓存初始化。夹具经 `applySharedToolbarPreferences` 明确加载五个出厂显示偏好，随后以真实候选窗口覆盖机器人／鲸鱼、回形针／图钉／圆点及固定徽标关闭的连续切换，同时验证候选身份、固定状态和原始视图不变。生产入口仍由 `ensureAppearance` 初始化；不以空串掩盖缺失的夹具状态。

整合作者的空徽标防护：格式化边界将 `nil` 视为缓存尚未提供，回退到共享纯函数表的出厂徽标；显式空字符串仍表示不画，不能被出厂回退覆盖。正文缺失按空串处理。这条边界防护不替代夹具初始化，回归分别验证 `nil`、显式空串、缺失正文与真实偏好切换。
