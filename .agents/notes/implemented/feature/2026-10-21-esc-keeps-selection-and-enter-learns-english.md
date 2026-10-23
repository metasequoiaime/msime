# Agent Note: Esc 保留已选词与回车学英文词改为可配

Status: implemented

## Problem

旧版 Windows 正式版（`metasequoiaime/msime-windows` v0.11.1）有两项输入行为设置，新架构里一个平台都没有：`platforms/windows`、`platforms/macos`、`platforms/linux`、`platforms/harmony`、`platforms/ios`、`platforms/android` 对这些键的检索都是 0 个文件。它们属于「v0.11.1 的遗留设置没有搬到新架构」，不是 macOS 独有的落后。

- **Esc 保留已选词**（`escape_keeps_selected_word`）：造词时第一次 Esc 只清掉还没选的拼音、保留已选的词，再按一次才整体取消（小狼毫的 `ClearNonConfirmedComposition`）。共享层 `crates/input-runtime/src/runtime.rs` 把 `Action::Command(Command::Cancel)` 写死成整体丢弃，已选词段跟着一起没。
- **回车学英文词**（`enter_learns_english_word`）：按回车上屏输入的英文时把这个单词写进英文词库。共享层 `crates/engine/src/session/input.rs` 在专用英文里无条件学词，`crates/engine/src/host/session.rs` 的回车策略也不看任何偏好。

## Decision

两项都提升为共享偏好，默认值对齐 Windows 正式版出厂配置，行为由共享 Rust 层决定，六个平台的共享输入链路一起受益。

**出厂默认值取自 v0.11.1 的 `installer/default_config/config.default.toml`：**

| 设置 | 出厂值 | 证据 |
|---|---|---|
| `escape_keeps_selected_word` | `false` | `installer/default_config/config.default.toml:331`；运行期回退 `server/src/config/ime_config.cpp:85`（全局初值）、`:494`（`.value_or(false)`） |
| `enter_learns_english_word` | `true` | `installer/default_config/config.default.toml:333`；运行期回退 `server/src/config/ime_config.cpp:87`（全局初值）、`:495`（`.value_or(true)`） |

**语义逐条照搬 v0.11.1 的判定，不重新发明。**

- Esc：`server/src/ipc/input_key_policy.h:251-254` 的 `ShouldEscapeKeepSelectedWord(option_enabled, raw_length, r_mode_active) = option_enabled && raw_length > 0 && !r_mode_active`。开着且还有没选的拼写、不在 R 模式（临时日文只能整段重置）时，`server/src/ipc/event_listener_keys.cpp:832` 走保留分支：清掉读音、候选列表收起，已选词单独留在组字里，选择历史保留（之后退格仍可撤回）。runtime 里词段本来就由 `phrase_prefix` 暂存、引擎的 `Cancel` 正好只清读音，所以保留分支只是不把词段冲给文档，判据与原函数逐项对应。
- 回车学词：`server/src/ipc/event_listener_keys.cpp:1132` 先用 `GetConfiguredEnterLearnsEnglishWord()` 做总开关，再按 `server/src/ipc/input_key_policy.h:137-141` 的 `ShouldLearnEnteredEnglishWord(dedicated_english, shift_letter_special_mode, chinese_scheme, all_complete_pinyin) = dedicated_english || shift_letter_special_mode || (chinese_scheme && !all_complete_pinyin)` 决定学不学。新架构里这条策略分在引擎内层（专用英文）和宿主 `Session` 的回车策略（本地模式、非完整音节拼音）两处，所以总开关同时接在两处；关掉后回车照常上屏，只是不写词库、也不报学词诊断。

**接线。** `Preferences::escape_keeps_selected_word`（默认 `false`，等于默认值时用 `skip_serializing_if = "is_false"` 不写进文档）与 `Preferences::enter_learns_english_word`（默认 `true`，等于默认值时用 `is_true` 不写进文档）落在 `crates/client-core/src/preferences.rs`。Esc 一项是宿主状态，经 `host-api` 的 `set_escape_keeps_selected_word` 与 FFI 会话接进 `crates/input-runtime/src/runtime.rs`；回车一项经 `EngineOptions::learn_english_on_enter` → `SessionOptions::learn_english_on_enter` 接进引擎内层与宿主策略。共享设置页 `packages/ui/src/settings/input-shared-settings-section.tsx` 在「选词与翻页」组加「Esc 保留已选词」（文案照 v0.11.1 设置页 `server/src/settings/native/native_page_input.cpp:1854`），在「候选与联想」组加「回车学英文词」（同文件 `:1857`）。

### 各平台的现状

- **macOS**：Esc 此前写死整体丢弃（`crates/input-runtime/src/runtime.rs`），回车学词此前写死开启。现在两项都可配；默认值（Esc 关、回车学开）与旧行为一致，升级不改变手感。
- **Windows 新架构**：`platforms/windows` 里两项都是 0，v0.11.1 的遗留设置没有搬过来。新宿主与 macOS 共用同一份 runtime/engine，因此这两项随共享偏好一起补齐；Windows 使用原生 WinUI 设置页，本 PR 没有为它增加设置控件。
- **Linux**：此前没有这两项，行为来自共享 runtime/engine。现在设置页（Tauri 设置应用）显示两行，值由共享偏好驱动。
- **HarmonyOS**：同上，设置页经 ArkWeb 加载的共享 rawfile 显示两行；宿主不额外读这两项，行为仍在共享层。

- **iOS 与 Android**：共享 runtime/engine 具有相同行为；两者使用原生设置页，本 PR 没有增加设置控件。共享 React 设置页只在 macOS、Linux 和 HarmonyOS 显示这两行。

## Alternatives considered

- **各平台各存一份本地开关，不提升为共享偏好。** 好处是不用碰共享文档格式和设置页。但这两项是输入行为、不是外观，六个平台本来就共用 runtime/engine；各存一份会让同一台机器上的共享设置页与本地开关互相矛盾，也重演 Windows v0.11.1 遗留设置搬不过来的老路。
- **Esc 默认开（小狼毫风格）。** 更接近 Rime 用户的直觉。但 Windows 正式版出厂是关，且 macOS 旧行为是整体取消；默认开会让现有 macOS 用户的 Esc 手感在升级后改变，所以默认关、由用户显式打开。
- **回车学词只在引擎内层加开关。** 改动面最小，只碰 `SessionOptions`。但产品路径经宿主 `Session` 的回车策略，非专用英文（本地模式、非完整音节拼音）那一半仍在宿主层学词，开关会显得时灵时不灵；所以内层和宿主策略一起接总开关。

## Consequences

- **收益**：v0.11.1 的两项遗留设置以共同能力的形式回到新架构，六个平台共享层一次补齐；默认值与旧行为一致，升级零行为变化。判定逻辑照搬 Windows 的谓词，没有引入新的产品决定。
- **代价与已知上限**：共享偏好新增两个键，`Preferences` 文档和 MCP/账号同步的镜像表暂时没有收录它们（按最小方案只接产品路径与设置页）；需要跨设备同步时再补 `crates/mcp-server/src/preferences.rs` 与 `crates/client-services/src/account/settings_sync.rs`。本机只在单元测试层面验证了 runtime/engine/设置页，没有在六个平台的真机上交互验证。

## Verification

审查新增真实 Engine 会话回归，覆盖 Esc 后再次取消、退格恢复选词历史、分段删除、回车和空格上屏；英文学习开关扩为专用英文、未完成拼音、临时英文、临时日语与完整拼音边界。实际运行结果随验证补齐。

- 先失败后通过：`cargo test -p msime-input-runtime escape_keeps_the_selected_word_when_the_preference_is_on`（关掉保留分支时在 `phrase_prefix` 断言失败，恢复后通过）；`cargo test -p msime-engine enter_learns_english_word_follows_the_preference`（未接总开关时关掉的用例在词库计数 `1 != 0` 失败，接上后通过）。
- `cargo test -p msime-engine -p msime-client-core -p msime-input-runtime` 全绿；`cargo fmt --all --check`；改动 crate 的 `cargo clippy --all-targets --locked -- -D warnings`。
- 设置页：`pnpm --filter @msime/desktop test`（新增 `tests/settings/escape-keeps-selection.test.tsx`、`tests/settings/enter-learns-english.test.tsx`）与 `pnpm format:check`。

相关笔记：[Windows 组字细节对齐 macOS](2026-10-10-windows-input-compose-mac-parity.md)（方向相反：那次是 Windows 补 macOS 已有的能力）。
