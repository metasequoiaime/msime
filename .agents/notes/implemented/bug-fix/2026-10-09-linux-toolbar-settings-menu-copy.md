# Agent Note: Linux 设置页把工具栏写成输入法菜单，不再呈现成悬浮窗

Status: implemented

## Problem

#6324：Linux 用户在设置里开着「悬浮工具栏」，桌面上却什么也看不到。

Linux 两个前端都不画悬浮工具栏窗口，这是既有设计（`platforms/linux/README.md` 里「不创建脱离输入上下文的伪悬浮窗口」）。`floating_toolbar` 偏好在 IBus 下是面板属性菜单里的「工具栏」子菜单（`ClientEngine.cpp` 的 `toolbar_property`），在 Fcitx5 下是托盘状态区菜单里的同名子菜单（`FcitxEngine.cpp` 的 `refreshToolbar`），后者还要求桌面提供托盘宿主。

共享设置页却把它呈现成悬浮窗：页首画一条悬浮条预览，开关叫「在桌面显示悬浮工具栏」，唯一的说明藏在页底「尺寸」组里。用户据此期待一个浮动条，看不到就以为坏了。

还有一种更糟的情形：IBus 跑在 GNOME Shell 下时，`publish_mode` 走 `candidate_panel_is_gnome_shell()` 分支，只注册输入模式和「设置」两项就返回，`toolbar_property` 从不发布。那里没有任何工具栏入口，旧说明里「上方的按钮选择和显示开关仍然生效」是错的。

## Decision

- `floating-toolbar-page.tsx` 用 `linuxPlatform` 分支（局部名 `menuToolbar`）：Linux 下页首不画 `SkinToolbarPreview`，换成 `FloatingToolbarPlatformNotice`；开关标题是「在输入法菜单显示工具栏」，描述写明入口：IBus 在面板属性菜单，Fcitx5 在托盘状态区菜单，需要桌面提供托盘；按钮组描述改说「工具栏」子菜单；「尺寸」组在 Linux 下整组不出现（缩放、图标尺寸本来就不显示，「颜色与明暗」链接本来就对 Linux 隐藏）。
- `FloatingToolbarPlatformNotice` 改成 Linux 专用文案，新增 `gnomeShell` 参数。页面在 `host.candidate_panel_limit === "gnome_shell"` 时传真。这个值只有 IBus 宿主会写（`publish_candidate_panel_status`），判据和 `publish_mode` 的 GNOME 分支是同一个 `candidate_panel_is_gnome_shell()`，所以页面说「不生效」的时候，宿主确实没发布工具栏。
- GNOME 下开关和按钮组的描述也跟着换，不再指向「工具栏」子菜单：开关说「当前 GNOME 桌面的输入源菜单没有「工具栏」子菜单，这个开关在这里不生效」，按钮组说「勾选工具栏要列出的按钮，当前 GNOME 桌面不生效」。页面只算一次 `gnomeShell`，页首说明和这两处描述用同一个值，免得页首说没有子菜单、下面又教人去子菜单里找。开关标题仍是「在输入法菜单显示工具栏」，它是这个偏好的名字，换到别的桌面就生效。
- 宿主还没写出状态文件时（例如用户还没在 IBus 下激活过输入法），页面不知道是不是 GNOME，通用说明的末尾仍注明「GNOME 桌面下的 IBus 没有这个子菜单」，不让 GNOME 用户只看到一句不成立的话。
- `host_surface.rs` 的能力位不动。Windows、macOS、Harmony 的页面（预览、标题、描述、「尺寸」组）不变，`host-capabilities.test.tsx` 按三家逐一钉住。

## Alternatives considered

- **按 `!showToolbarAppearance` 分支而不是 `linuxPlatform`**：今天只有 Linux 报 `floating_toolbar_appearance: false`，两个判据结果相同，而且能力位比平台名更符合「按能力渲染」的约定。没有采用：新文案点名 IBus、Fcitx5 和 GNOME，是 Linux 宿主的事实，不是「没有外观设置」这个能力的推论；将来某个宿主不报外观但画的是悬浮窗，按能力位分支会把 Linux 文案套到它头上。
- **在 GNOME 下隐藏整页**：GNOME 用户不会再看到一页不生效的设置。没有采用：IBus 和 Fcitx5 读的是同一份用户偏好，同一个人换到别的桌面或改用 Fcitx5 后这些开关立刻生效，隐藏后他在 GNOME 会话里就没法预先调；而且状态文件只反映当前会话，隐藏会让页面随会话忽隐忽现。如实写「当前桌面不生效」已经回答了 #6324 那位用户的疑问。
- **Linux 真正画一个悬浮工具栏窗口（复用 `platforms/linux/src/overlay/` 的 X11/Wayland 自绘面）**：能让设置页与 Windows 一致。工作量是 L/XL 级，Wayland 上窗口定位和常驻受合成器限制（GNOME 不支持 layer-shell），并且推翻 README 里的既有设计边界，不在这次范围内。
- **把 Linux 下的导航页名也改成「工具栏」**：更彻底，但页名同时是导航按钮、`aria-label` 和多处测试的锚点，牵动面比文案大；页首说明已经第一句就写「Linux 不显示悬浮工具栏窗口」。留给以后单独决定。

## Consequences

收益：Linux 用户打开这一页，第一眼看到的是「没有悬浮窗、工具栏在输入法菜单里」以及具体入口；GNOME + IBus 用户被明确告知这页在当前桌面不生效，而不是被告知「仍然生效」。

代价：

- `FloatingToolbarPlatformNotice` 从「菜单型宿主」的通用说明变成了 Linux 专用文案。假设中「不是 Linux、又不报外观能力」的宿主，「尺寸」组里原先的通用说明不再出现，那里只剩「颜色与明暗」链接；现实中没有这样的宿主。
- GNOME 判定依赖 IBus 宿主写的 `candidate-panel.json`。Fcitx5 跑在 GNOME 下不会报 `gnome_shell`，那时页面给通用说明；Fcitx5 的入口取决于托盘，描述里已写明需要桌面提供托盘。

验证：`apps/desktop` 下 `vitest run tests/settings tests/input/host-capabilities.test.tsx`（Linux 页首说明在「显示」组之前、无预览、新标题与描述、无「尺寸」组；GNOME 文案不含「仍然生效」，开关和按钮组描述不提子菜单入口；Windows/macOS/Harmony 保持预览、旧标题和「显示 / 按钮 / 尺寸」三组），以及 `pnpm run typecheck`。

尚未验证：没有在真实的 Linux 桌面（IBus/GNOME、IBus/KDE、Fcitx5）里打开设置窗口看这页。
