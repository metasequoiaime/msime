# Agent Note: macOS 候选窗与悬浮工具栏的水杉 logo 开关

Status: implemented

## Problem

macOS 候选窗首行和悬浮工具栏左端总是画着水杉 logo，用户没有办法关掉。希望提供一个开关，并且新装默认隐藏。已经在用的人升级后不应该被替他改掉外观。

## Decision

共享偏好 `Preferences::show_app_logo`（`crates/client-core/src/preferences.rs`）同时控制这两处 logo。默认值按 `cloud_candidates` 的写法：

- `Default` 为 `false`：新装、写第一份偏好文档、以及「恢复默认设置」时都隐藏 logo。
- serde 缺省为 `true`：已有文档里没有这个字段时读成显示，升级用户看到的和原来一样。

`HostCapabilities::app_logo` 只在 macOS 为真，开关只在 macOS 的设置里出现：共享设置页「候选窗口」→「布局」组的「显示水杉 logo」，以及原生设置窗口候选窗卡片里的同名开关。Windows、Linux 和鸿蒙的候选窗照常画 logo，不读这个偏好。

macOS 宿主的行为：

- 候选窗（`InputController.mm`）不画 logo 时，拼音顶到左边；既没有拼音又只有一页时，首行不占高度。
- 悬浮工具栏（`FloatingToolbarPanel.mm`）的 logo 同时是拖动把手。不画 logo 时，这个视图收窄成 10pt 宽的握把，画两列三行小圆点，颜色同分隔线；拖动和手形光标照旧，工具栏总宽度少 24pt。
- 原生设置页的两个预览（`CandidateSkinPreviewView.mm`）和共享设置页的候选预览（`skin-candidate-preview.tsx` 的 `brand` 参数）跟着开关走。

`msime config set show_app_logo=…` 和 MCP 的 `update_preferences` 也能改这个开关（`crates/mcp-server/src/preferences.rs`）。

## Alternatives considered

- **候选窗和工具栏各给一个开关** — 能分别控制。但用户要的是一个开关，两处 logo 在视觉上是同一件事，拆成两个只多一项设置。
- **字段放进 `floating_toolbar` 里** — `FloatingToolbarPreferences` 没有 `deny_unknown_fields`，旧版本读到不认识的字段不会拒绝整份文档。但它管候选窗的 logo，放在工具栏下面名不副实；而且旧版设置应用保存时会丢掉这个字段，下次又读回 serde 缺省值，开关会被悄悄打开。
- **隐藏 logo 后工具栏不留握把，靠背景拖动** — 实现最简单。但按钮之间只剩几个点宽的空隙，所有按钮都关掉时工具栏几乎没有宽度，实际上拖不动。Windows 的工具栏也另有一条拖动条。

## Consequences

- **收益**：新装默认是更简洁的候选窗和工具栏，老用户升级不变。一个偏好、一个开关，能从设置页、原生设置窗口和命令行改。
- **代价与已知上限**：
  - `show_app_logo` 是顶层字段，`Preferences` 带 `deny_unknown_fields`，所以早于这个字段的设置应用或 `msime-mcp` 会拒读写过这个字段的偏好文档。正式发布里输入法和设置应用在同一个包里，版本一致；开发机上单独换了输入法时，设置应用要一起更新。
  - 其他平台还没有这个开关。要做时，给 `HostCapabilities::app_logo` 加平台，并让那个平台的候选窗按偏好画 logo。

## Verification

- `preferences/tests.rs` 的 `app_logo_starts_hidden_but_an_upgraded_document_keeps_it` 覆盖新装、恢复默认、旧文档缺字段和保存后读回四种情况；`host_surface/tests.rs` 断言只有 macOS 声明 `app_logo`。
- `apps/desktop/tests/settings/settings.test.tsx` 的两条 `app logo` 用例：非 macOS 宿主不显示开关；macOS 上开关默认关，点开后保存 `show_app_logo: true`。
- macOS CTest：`floating-toolbar-panel` 断言隐藏 logo 时握把宽 21pt（150% 缩放）、仍可拖动、分隔线紧随其后、总宽度少 24pt；`skin-preview` 在带 logo 和不带 logo 两种布局下都检查工具栏预览的分隔线位置与颜色。
