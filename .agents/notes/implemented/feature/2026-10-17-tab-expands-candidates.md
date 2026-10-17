# Agent Note: Tab 打开展开候选

Status: implemented

## Problem

#8429 反馈展开多行多列候选时必须先按方向键，实体键盘上不如微信输入法顺手。iOS 全尺寸 iPad 键盘已经把共享 `navigation.tab` 用作打开展开候选；Android 的 Tab 只翻页，macOS 的 Tab 也只翻页，两个宿主的展开面板因而没有同一条快捷入口。

## Decision

在已有展开能力的宿主里，未按 Shift 的 Tab 先尝试打开完整候选面板；面板不适用时保留原来的翻页路径。Android 的 `CandidateNavigationPolicy` 只负责判断 Tab 是否有资格尝试打开，`MSIMEInputService` 仍由 `openCandidatePanel()` 检查候选页数、九键方案和当前会话，失败后回到普通导航。Shift+Tab 始终向前翻页，打开后的面板继续使用各宿主已有的 Tab 滚动或导航处理。

macOS 在候选窗可见、`expand_candidates` 已开启且网格确实可展开时，让 Tab 进入 `openExpandedCandidates`；释义列预选仍先于展开处理，偏好关闭、竖排、单页和 Shift+Tab 都保留原行为。网格打开后 Tab 仍由 `handleExpandedCandidateEvent:` 一次滚过可见行。

## Alternatives considered

- **把 Tab 永久改成展开，取消翻页**：最强的理由是入口最直接；不选它，因为单页、竖排或展开偏好关闭时没有面板可打开，且候选列表的翻页快捷键仍是已有用户习惯。只有成功打开面板时才消费 Tab，失败即回退到原翻页。
- **新增一份独立的“展开快捷键”偏好**：最强的理由是可以在 Tab 和方向键之间逐项选择；不选它，因为 iOS 已把 `navigation.tab` 定义为全尺寸键盘的展开入口，macOS 的 Tab/PageUp 由宿主固定接管，新增字段会让设置页、同步文档和各宿主产生第二套绑定。这里复用现有 Tab 导航开关和 `expand_candidates` 能力开关。
- **只改 macOS**：最强的理由是 #8429 的方向键描述直接对应桌面网格；不选它，因为 Android 同样有完整候选面板和外接键盘路径，复用同一个“先尝试展开、失败再翻页”的路由可以让两个宿主与 iPad 的行为一致。

## Consequences

- Android 外接键盘在候选有多页时按 Tab 直接进入展开候选；没有可展开面板时不会吞掉原本的翻页或应用行为，九键三栏面板也沿用已有资格判断。
- macOS 开启「展开候选」后，Tab 不再先翻到下一页，而是进入网格；网格内 Tab 仍滚动，Shift+Tab 仍翻页。释义列快捷键和候选身份、代次契约不变。
- 共享设置文案和 Android/macOS README 明确了新入口。Android 的纯策略冒烟测试覆盖未按 Shift、Shift+Tab 和已打开面板三个边界；macOS `shortcut` 集成测试覆盖 Tab 打开网格且不发 Engine 翻页命令。
- Windows、Linux 和 HarmonyOS 的展开入口未改动；它们没有与本次路由相同的桌面展开能力或现成 Tab 展开契约，后续接入应单独补宿主级路由和验收。
