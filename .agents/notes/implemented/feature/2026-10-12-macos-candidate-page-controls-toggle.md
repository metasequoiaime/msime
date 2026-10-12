# Agent Note: macOS 候选窗的页码和翻页按钮开关

Status: implemented

## Problem

#7316 的用户想要更紧凑的 macOS 候选窗。logo 和窗内拼音都关掉以后，只要候选多于一页，卡片顶上还是留着一行 26pt 的首行（`MSIMECandidateHeaderHeight`），里面只有右侧的「1 / 3」和 ‹ ›，没有任何设置能关掉它。截图里看着浪费的空间主要就是这一行。

共享偏好里已经有 `show_candidate_page_number`（默认 true），共享设置页的「显示页码」开关也有，但 `HostCapabilities::candidate_page_number` 只对 Linux 为真，macOS 宿主不读它。

## Decision

macOS 复用 `show_candidate_page_number`，不新增偏好：

- `host_surface.rs` 对 Linux 和 macOS 声明 `candidate_page_number`，共享设置页「候选窗口」→「布局」组随之出现这个开关。macOS 上行名是「显示页码和翻页按钮」，说明写「关闭后候选窗不再显示「1 / 3」和 ‹ ›，键盘、滚轮翻页照常。」，Linux 仍是「显示页码」（`candidate-page-number-section.tsx` 按 `platform` 选文案）。
- `AppearancePreferences` 加 `showsCandidatePageNumber`，写法同 `showsAppLogo`：共享值存 `_sharedShowsCandidatePageNumber`，本机值存 `MSIMEClientCandidatePageNumber`，两者都没有时为 YES。原生设置窗口候选窗卡片里加同名开关，「恢复默认值」覆盖它。
- 输入法保存偏好时总把当前值写进 `show_candidate_page_number`，和 `show_app_logo` 一样。输入法每次载入文档都先填 `_sharedShowsCandidatePageNumber`，所以写回去的就是文档里的值；「恢复默认值」清掉本机值和共享值后，写回的是默认的显示，恢复才落到文档里。
- `InputController.mm` 的 `renderCandidates` 用 `pageControls = paging && showsCandidatePageNumber` 决定画不画「1 / 3」和 ‹ ›、首行高度和宽度、以及 `candidatePageGeometry` 为箭头留的 76pt 最小宽度。`paging` 本身不变，面板的 `hasPreviousPage` / `hasNextPage` 仍按页码设置，滚轮和键盘翻页不受开关影响。`updateKeymapPanel` 算双拼键位图让开的高度时用同一条规则。
- MCP `update_preferences` 和 `msime config set` 也能读写这个字段（`crates/mcp-server/src/preferences.rs`），和 `show_app_logo` 一样。

logo、拼音、页码和翻页按钮都关掉时，首行高度为 0，卡片只剩候选行。

## 为什么箭头跟页码一起隐藏

Linux 的开关只去掉辅助文本里的当前页/总页数，那里没有箭头。macOS 的首行由 logo、拼音和翻页控件三样东西撑开，只藏「1 / 3」的话 ‹ › 还在，首行照样占 26pt，用户要的紧凑一点也没有得到。箭头是鼠标翻页的唯一入口，但滚轮翻页（`navigation.mouse_wheel`）和键盘翻页都还在，关掉开关的人是主动选择了更少的界面元素。

## Alternatives considered

- **新增 macOS 专用的 `show_candidate_page_arrows`** — 页码和箭头可以分开控制。但分开后，只关其中一个都不能让首行消失，第二个开关只多一项设置；而且新顶层字段在 `deny_unknown_fields` 下会让旧版设置应用拒读偏好文档（见 [logo 开关](2026-10-08-macos-app-logo-toggle.md) 的代价一节），复用现有字段没有这个问题。
- **只隐藏「1 / 3」，保留箭头** — 和 Linux 的语义完全一致。但首行仍然存在，解决不了 #7316。
- **同时把卡片内边距做成偏好** — #7316 的第二个请求。它要定共享字段的范围、和皮肤包 `corner_radius_dip` 一样的优先级、哪些宿主声明能力，还牵涉皮肤包格式，需要产品决定，这次不做。

## Consequences

- **收益**：macOS 用户能把多页时的首行去掉，卡片只剩候选行。没有新增偏好字段，偏好文档格式不变，Linux 和 macOS 共用一个开关。
- **代价**：
  - 同一个开关在 Linux 和 macOS 上管的范围不同（macOS 多了箭头），靠两个平台各自的行名说明。
  - 关掉后鼠标只能用滚轮翻页；滚轮翻页开关默认关，所以这类用户要自己打开它，或用键盘翻页。
  - Windows 候选窗首行也画「1 / 3」和 ‹ ›（[Windows 候选窗](2026-10-10-windows-candidate-window-mac-parity.md) 的 `pager_visible`），但没有声明这个能力，要做时给 `HostCapabilities::candidate_page_number` 加 Windows，并让 `candidate_card_metrics` 读这个偏好。
  - 内边距仍是皮肤的 `pad`（macOS 6pt），不能由用户调。

## Verification

- `crates/client-core/tests/candidate_page_number.rs` 的 `linux_and_macos_expose_the_page_number_control` 断言只有 Linux 和 macOS 声明能力。
- `crates/mcp-server/src/preferences.rs` 的 `the_page_number_switch_is_read_and_written` 断言缺省为开、能关掉、写进偏好库。
- `apps/desktop/tests/settings/settings.test.tsx`：macOS 宿主上行名是「显示页码和翻页按钮」，点掉后保存 `show_candidate_page_number: false`；「basic to advanced」按平台期待行名。
- macOS CTest `shortcut`（需要图形会话，在本机跑）：`TestCandidatePageNumberPreference` 覆盖缺省值、非布尔共享值被忽略、共享值优先、本机值持久化、「恢复默认值」后写回显示；渲染部分断言关掉开关后三页的卡片没有 `candidate-page-indicator` 和 tag -1/-2 的按钮，与只有一页时一样高，面板仍可往下一页滚。`preference-coverage` 去掉了 macOS 不读 `show_candidate_page_number` 的豁免。
