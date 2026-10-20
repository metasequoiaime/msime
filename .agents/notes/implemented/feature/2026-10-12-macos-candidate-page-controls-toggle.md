# Agent Note: macOS 紧凑候选窗：页码和翻页按钮开关与卡片内边距

Status: implemented

## Problem

#7316 的用户想要更紧凑的 macOS 候选窗，提了两件事。

第一件：logo 和窗内拼音都关掉以后，只要候选多于一页，卡片顶上还是留着一行 26pt 的首行（`MSIMECandidateHeaderHeight`），里面只有右侧的「1 / 3」和 ‹ ›，没有任何设置能关掉它。截图里看着浪费的空间主要就是这一行。共享偏好里已经有 `show_candidate_page_number`（默认 true），共享设置页的「显示页码」开关也有，但 `HostCapabilities::candidate_page_number` 只对 Linux 为真，macOS 宿主不读它。

第二件：卡片边缘和候选之间的距离太大，希望能自己调。macOS 上这段距离是皮肤 token `pad`（原生 6pt，皮肤包的 `padding` 可以改），四边都用它（`InputController.mm` 的 `inset = MAX(2 * scale, pad)`），用户没有设置能动它。

## Decision

### 页码和翻页按钮

macOS 复用 `show_candidate_page_number`，不新增偏好：

- `host_surface.rs` 对 Linux 和 macOS 声明 `candidate_page_number`，共享设置页「候选窗口」→「布局」组随之出现这个开关。macOS 上行名是「显示页码和翻页按钮」，Linux 仍是「显示页码」（`candidate-page-number-section.tsx` 按 `platform` 选文案）。
- macOS 的说明写「关闭后候选窗不再显示「1 / 3」和 ‹ ›，仍可用键盘翻页；鼠标滚轮翻页默认关闭，需要时在「输入 › 选词与翻页」里打开。」原生设置窗口里同一句话指向「按键 › 候选翻页与选字」。说明不能许诺滚轮翻页照常：滚轮翻页由 `navigation.mouse_wheel` 控制，默认关（`preferences.rs` 的 `NavigationPreferences::default`），新装用户关掉这个开关后 ‹ › 就是鼠标翻页的唯一入口，没了它鼠标就翻不了页。
- `AppearancePreferences` 加 `showsCandidatePageNumber`，写法同 `showsAppLogo`：共享值存 `_sharedShowsCandidatePageNumber`，本机值存 `MSIMEClientCandidatePageNumber`，两者都没有时为 YES。原生设置窗口候选窗卡片里加同名开关，「恢复默认值」覆盖它。
- 输入法保存偏好时总把当前值写进 `show_candidate_page_number`，和 `show_app_logo` 一样。输入法每次载入文档都先填 `_sharedShowsCandidatePageNumber`，所以写回去的就是文档里的值；「恢复默认值」清掉本机值和共享值后，写回的是默认的显示，恢复才落到文档里。
- `InputController.mm` 的 `renderCandidates` 用 `pageControls = paging && showsCandidatePageNumber` 决定画不画「1 / 3」和 ‹ ›、首行高度和宽度、以及 `candidatePageGeometry` 为箭头留的 76pt 最小宽度。`paging` 本身不变，面板的 `hasPreviousPage` / `hasNextPage` 仍按页码设置，滚轮（打开时）和键盘翻页不受开关影响。`updateKeymapPanel` 算双拼键位图让开的高度时用同一条规则。
- MCP `update_preferences` 和 `msime config set` 也能读写这个字段（`crates/mcp-server/src/preferences.rs`），和 `show_app_logo` 一样。

logo、拼音、页码和翻页按钮都关掉时，首行高度为 0，卡片只剩候选行。

### 卡片内边距

新增共享偏好 `candidate_padding: Option<u8>`，写法照 `candidate_corner_radius`：

- 单位是点（Windows 上将来是 DIP），核心接受 0–16，超出时 `validate` 返回 `InvalidCandidateWindowStyle`。缺省为 `None`，表示跟随皮肤包和宿主自己的值；`skip_serializing_if = "Option::is_none"`，没设过的文档不出现这个字段。
- 优先级：用户值先于皮肤包的 `padding`，皮肤包先于宿主常量（macOS 4pt）。用户值在「整体大小」缩放之前替换 `pad`，所以缩放照样把它放大（`CandidateSkin.cpp` 的 `StyledCandidateSkin`）。
- 只有 macOS 声明 `HostCapabilities::candidate_padding`。共享设置页「窗口样式」组在圆角下面加「内边距」一行：滑块 2–16pt，旁边「跟随皮肤」按钮把值交还皮肤（写成缺省）；跟随皮肤时滑块停在 macOS 原生的 4pt。滑块从 2 起，因为 macOS 画卡片时至少留 2pt（上面的 `inset`），0 和 1 画出来和 2 一样。这个下限后来改为边框宽度，滑块改为 0–16，见[内边距低于 2pt](../bug-fix/2026-10-15-macos-candidate-padding-below-2pt.md)。
- `AppearancePreferences` 加 `candidatePadding`（`NSNumber`，nil 表示跟随皮肤），本机值存 `MSIMEClientCandidatePadding`，回到跟随皮肤时存空串，与圆角相同；共享值存 `_sharedCandidatePadding`，文档里是 null 或缺省时为 `NSNull`。原生设置窗口候选窗卡片里加「内边距」滑块和「跟随皮肤」按钮，「恢复默认值」覆盖它。保存时总写 `candidate_padding`，跟随皮肤时写 null，`MSIMEMergePreferenceSnapshot` 才会清掉旧值。
- 设置页预览：`candidateWindowStyle` 在设了值时给出 `--msime-candidate-user-pad`，`styles.css` 里卡片的 `--msime-candidate-pad-x/y` 先取它，再取宿主的 `--msime-host-pad`。
- MCP `update_preferences` 读写 `candidate_padding`，null 交还皮肤。

## 为什么箭头跟页码一起隐藏

Linux 的开关只去掉辅助文本里的当前页/总页数，那里没有箭头。macOS 的首行由 logo、拼音和翻页控件三样东西撑开，只藏「1 / 3」的话 ‹ › 还在，首行照样占 26pt，用户要的紧凑一点也没有得到。箭头是鼠标翻页唯一一个默认就有的入口，所以开关说明要告诉用户去哪里打开滚轮翻页。

## Alternatives considered

- **新增 macOS 专用的 `show_candidate_page_arrows`** — 页码和箭头可以分开控制。但分开后，只关其中一个都不能让首行消失，第二个开关只多一项设置；而且新顶层字段在 `deny_unknown_fields` 下会让旧版设置应用拒读偏好文档（见 [logo 开关](2026-10-08-macos-app-logo-toggle.md) 的代价一节），复用现有字段没有这个问题。
- **只隐藏「1 / 3」，保留箭头** — 和 Linux 的语义完全一致。但首行仍然存在，解决不了 #7316。
- **关掉页码时顺便打开滚轮翻页** — 能保住鼠标翻页，但一个开关悄悄改另一个偏好，用户关掉滚轮翻页的理由（触控板误翻）也会被覆盖。改成在说明里指路。
- **内边距做成「紧凑 / 标准」两档** — 少一个滑块，但 #7316 要的是「自己编辑」间距，两档定不准每个皮肤合适的值；圆角已经是滑块加「跟随皮肤」，同一组里保持一种控件。
- **内边距分成水平和垂直两个值** — 皮肤 token 只有一个 `pad`，四边共用；拆开要改皮肤包格式和三处宿主，用户的截图里四边一样大，一个值够用。
- **这次就让 Windows、Linux、鸿蒙也读 `candidate_padding`** — Windows 的边距在 `candidate_card_metrics`，Fcitx5 主题和鸿蒙卡片各有自己的边距，每个都要单独接、单独验证；issue 报的是 macOS 0.52.0，并说 Windows 版已经够紧凑。先只给 macOS 声明能力，其他宿主不显示这一行。

## Consequences

- **收益**：macOS 用户能把多页时的首行去掉，并把卡片边缘到候选的距离从 6pt 调到最小 2pt，卡片只剩紧贴边缘的候选行，#7316 的两件事都有了设置。
- **代价**：
  - 同一个 `show_candidate_page_number` 在 Linux 和 macOS 上管的范围不同（macOS 多了箭头），靠两个平台各自的行名说明。
  - 关掉页码和翻页按钮后鼠标只能用滚轮翻页，而滚轮翻页默认关，用户要按说明自己打开它。
  - `candidate_padding` 是新顶层字段，`Preferences` 带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读设过内边距的文档；没设过时字段不写入，不受影响。正式发布里输入法和设置应用同包，版本一致。
  - Windows 候选窗首行也画「1 / 3」和 ‹ ›（[Windows 候选窗](2026-10-10-windows-candidate-window-mac-parity.md) 的 `pager_visible`），已经读 `show_candidate_page_number`（[Windows 缺省几何对齐 macOS](2026-10-20-windows-candidate-mac-default-geometry.md)），但还没有声明 `candidate_page_number`，原生设置窗口也还没有这一行；内边距同样只有 macOS 读。

## Verification

- `crates/client-core/tests/candidate_page_number.rs` 的 `linux_and_macos_expose_the_page_number_control` 断言只有 Linux 和 macOS 声明能力；`host_surface/tests.rs` 的 `only_macos_offers_the_candidate_padding` 断言只有 macOS 声明 `candidate_padding`。
- `preferences/tests.rs` 的 `candidate_padding_is_absent_by_default_and_bounded`：缺省不写入文档，0、2、16 往返不变并能存进偏好库，17 被拒。
- `crates/mcp-server/src/preferences.rs` 的 `the_page_number_switch_is_read_and_written` 和 `the_candidate_padding_is_set_and_returned_to_the_skin`：开关能关掉；内边距能设、null 交还皮肤、17 被拒。
- `apps/desktop/tests/settings/settings.test.tsx`：macOS 宿主上行名是「显示页码和翻页按钮」，说明指向滚轮翻页的设置，点掉后保存 `show_candidate_page_number: false`；「basic to advanced」按平台期待行名。
- `apps/desktop/tests/settings/candidate-window-style-section.test.tsx`：「内边距」排在圆角之后，跟随皮肤时滑块停在 `CandidateSkin.h` 的原生 `pad`，范围 2–16，拖动写 `candidate_padding`，「跟随皮肤」写回缺省；只有 macOS 的能力打开这一行；预览样式带 `--msime-candidate-user-pad`。`candidate-preview-native-tokens.test.ts` 和 `external-skins.test.tsx` 核对样式表里用户值先于 `--msime-host-pad`。
- macOS CTest：`candidate-skin` 断言用户内边距替换皮肤的 `pad`、不动圆角、被缩放放大、超出 16 截到 16；`skin-preview` 覆盖原生设置窗口的滑块范围、文档值的取舍（非整数、越界、字符串、布尔不接受）、本机持久化、跟随皮肤写 null、setter 拒收 17；`shortcut`（需要图形会话，在本机跑）断言内边距 2pt 时卡片高度少 `2 × (pad − 2)`、候选行左移 `pad − 2`、宽度不增加，交还皮肤后恢复原尺寸，另有 `TestCandidatePageNumberPreference` 覆盖页码开关。`preference-coverage` 去掉了 macOS 不读 `show_candidate_page_number` 的豁免，`candidate_padding` 由宿主读取，不需要豁免。
