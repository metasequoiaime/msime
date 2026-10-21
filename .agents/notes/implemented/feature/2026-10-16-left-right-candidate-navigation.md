# Agent Note: 左右键在输入串两端移动候选

Status: implemented

## Problem

用户要的「左右方向键也能选候选」在现状里没有位置：四个方向键只有 ↑/↓ 由 `navigation.arrows` 管，是移动候选高亮；←/→ 是编辑输入串的光标键，全拼和双拼里还先承担整句改字（`msime_client.h` 的 `MSIME_CONVERSION_LEFT`/`MSIME_CONVERSION_RIGHT`）。速查页把这两件事分开列：「移动候选项 ↑ / ↓」和「编辑输入串 ← / → / Backspace」。

新开关要在保留现有手感的前提下让左右键也能选词，判断点因此落在「光标已经停在要移动的那一端」。这一步同时要看引擎的光标位置和 runtime 的高亮，而这两个状态分属两层：高亮是 runtime 的状态，引擎不知道哪个候选被高亮。五个宿主（Windows TSF、macOS、Linux、Android 外接键盘、HarmonyOS 外接键盘）都各自把左右键发成光标命令，各判一遍就是五份同一规则的实现。

## Decision

> 默认关闭的历史决定已由 [2026-10-20-issue-8847-left-right-default.md](../bug-fix/2026-10-20-issue-8847-left-right-default.md) 取代；下面记录的是开关初次引入时的实现边界。
>
> 下面的「编辑优先」语义已由[方向键按候选窗朝向和首选分工](../bug-fix/2026-10-21-left-right-by-orientation-and-first-candidate.md)部分取代：横排里首选上编辑、其余候选上左右键只移动候选，竖排时左右键只编辑；判断位置和不接管的边界仍以本篇为准。

新偏好 `navigation.left_right`（`NavigationPreferences`），默认关闭、可选字段：`#[serde(default, skip_serializing_if = "is_false")]`，与 `mouse_wheel` 一样不在设置页的默认对象里补写，旧文档里没有这个键就是关闭。

语义是**编辑优先**：不带修饰键的 ←/→ 先照旧编辑输入串；光标已经停在要移动的那一端、引擎动不了它时，这一次按键改成移动候选高亮（← 上一个、→ 下一个）。到端点以后不再编辑光标，这是本次的取舍。

判断放在 `input-runtime`（`Runtime::caret_edge_candidate_shortcut`，在 `dispatch_action` 顶部与 `candidate_shortcut` 并列），不放宿主：

- 宿主按键路由一行没改，只把左右键照旧发成 `MSIME_MOVE_LEFT`/`MSIME_MOVE_RIGHT`（整句方案是 `MSIME_CONVERSION_LEFT`/`MSIME_CONVERSION_RIGHT`）。
- 偏好由 `host-api` 在偏好生效时推给 runtime（`Runtime::set_left_right_navigation`，两个调用点：`msime_client_create` 与 `PreferencesState::update`），与 `backspace_reverts_selection` 同一条路：宿主状态，改完立刻生效，不重建 Engine。
- 换成的动作就是 ↑/↓ 的那两个动作（`Action::PreviousCandidate`/`NextCandidate`），所以候选项扩展、隐藏行、代次、翻页换页与 ↑/↓ 完全一致。

三条不接管的边界：

- 当前页没有候选（`visible_len() == 0`）时不接管：左右键照旧交给引擎，于是仍被吞掉，不会变成应用的按键落进文档（应用里的 ←/→ 会移动文档光标）。
- 整句改字期间不接管：改字时左右键移的是改字的光标，它不在输入串里（`caret_position` 停在末尾），按输入串端点判会把改字导航整个劫持成移候选。
- 已经打开的候选列表（韩文汉字、注音）不接管：那张列表本来就有自己的方向键规则，各宿主也先一步把四个方向键都归给高亮，runtime 不重复接管。

本地模式（`/`、网址模式）的光标起点在前缀字母之后，这里按 0 判左端：那一个位置上左右键仍然什么都不做，宁可漏一次也不把死键变成移候选。

界面三处：共享设置页的「翻页方式」加一项「左 / 右（移动候选项）」（桌面表与 HarmonyOS 手机的「外接键盘翻页键」表都加，手机用 `← / →` 符号），速查页在「移动候选项」下加一行「移动候选项（输入串两端）← / →」，Windows 原生设置页的「翻页方式」加同名的第 6 项。iOS 不动：外接实体键盘的按键不会交给第三方键盘，它的设置页也没有翻页键分组。Android 的设置页没有翻页键分组，行为跟着共享 runtime 自动生效。

## Historical audit

按 `navigation`、`arrows`、`mouse_wheel`、翻页和各宿主的方向键消费检索活跃笔记：

- [翻页与以词定字共用偏好转换](../../implemented/simplification/2026-10-15-candidate-paging-preferences.md)：部分重叠，本篇只往那张选项表加一项、往速查加一行，互斥、补丁和默认对象规则仍由它持有；它的「桌面七项 / 手机六项」已就地改成八项 / 七项。
- [macOS 候选控件](../../implemented/feature/2026-10-12-macos-candidate-page-controls-toggle.md)：不重叠，滚轮与页码开关不动。

## Alternatives considered

- **每个宿主各判一次端点。** 宿主手里就有视图，改动看起来最小，也不用碰共享层；但「能不能编辑」的规则（本地模式的前缀字母、整句改字、打开的列表、隐藏行、光标的字节偏移）要在 C++、Objective-C++、Java、ArkTS 里各写一遍，五份一起漂移，而本机只有 Rust 那份跑得起测试。macOS 现在那段「首选的左键编辑光标」就是这个做法的简化版，它已经在用 `highlighted == 0` 近似左端。
- **新增一对共享命令（`MSIME_MOVE_LEFT_OR_PREVIOUS_CANDIDATE`）。** 宿主只换一个命令号，同样是一处实现；但要扩 C ABI 的命令表和它的中文文档，而语义完全能用现有光标命令加 runtime 判断表达，多出来的两个常量只是把「偏好已经推给 runtime」这件事藏进命令号里。
- **候选优先：有高亮就左右移候选，光标编辑让位。** 语义最直白，离「开启后可以用左右选词」的字面意思最近；但全拼和双拼里 ←/→ 是整句改字的光标键，接管它会让改字只剩 Home/End 和鼠标可用，用户选了编辑优先。
- **把高亮下沉进引擎，让 `edit_at_caret` 到端点时自己移高亮。** 判断会落在唯一一处已经有全部端点规则的地方；但高亮是 runtime 的展示状态，下沉等于把展示状态搬进算法层，与「引擎只管组词、runtime 管编排」的边界相反。
- **只落偏好与界面，宿主后续再接。** 设置里能勾、勾了不生效，比不做更糟。

## Consequences

收益是把一条跨层规则留在唯一能同时看见两层的图层里：一处实现覆盖五个宿主的共享会话路径（Windows、macOS、Linux、Android 外接键盘、HarmonyOS 外接键盘都把自己的左右键发成同一条光标命令），五份宿主的按键路由一行没动，移动候选的语义与 ↑/↓ 共用同一段代码，候选扩展、隐藏行和翻页换页不会各写一套。网页引擎不在其中：它的 JS 按键映射（`packages/web-engine/src/keys.js`）组字时本来就把左右键直接包成移动高亮，不经过这些光标命令。

代价是 `Command::MoveLeft` 在开关打开时不再只有字面意思：宿主不能再用它保证「只编辑、不选词」，所以整句改字和打开的列表必须显式排除，这三条边界（含「当前页没有候选时不接管」）各有用例守着。

默认关闭，字段缺省即关闭且 false 不落盘，旧文档、参考配置和已有用户行为一字不变；打开后左右键在端点由「什么都不做」变成移高亮，这是本次唯一的行为变化。

验证：`cargo test -p msime-input-runtime` 280 项全绿（含新增的 `left_right_edits_the_input_string_before_it_moves_the_highlight`）；`cargo test -p msime-host-api left_right` 里新增的 `left_right_navigation_preference_reaches_real_sessions_and_live_updates` 用真 Engine 加合成词库走通「新建会话时读一次」和「运行中改偏好」两条路；`cargo test -p msime-client-core` 只多出一个与本改动无关的既有失败（`host_surface::tests::candidate_panel_limit_reads_only_what_the_host_reported` 要求 `/run/user/1000` 是绝对路径，在 Windows 上必然为假，CI 在 Linux 上跑它）；`cargo fmt --all -- --check` 干净，clippy 只剩两处既有告警（`host-windows`、`host-api` 剪贴板里的 `Result<_, ()>`）。

前端：`apps/desktop` 的全量 vitest 除 4 个按源码逐字比对、因本工作区是 CRLF 检出而失败的既有用例外全绿（同样的断言在仓库 blob 的 LF 文本上成立）；整批并行时 `cloud-clipboard-relay` 偶发失败一次，单跑 9 项通过，与本次改动无关；`tsc --noEmit`、`apps/harmony` 类型检查、`vite build` 与 `vpr lint` 通过；「选词与翻页」那条集成用例另加一步勾选新项，核对它默认关闭、勾上后 `navigation.left_right` 落进保存的偏好。

未验证（本机没有对应工具链）：Windows 原生设置页（WinUI 3，无 MSVC）与 TSF 那次重画（无 MinGW/vcpkg）只做了人工核对，等 CI 的 Windows 交叉构建和 Windows runner 的设置工程编译；macOS 的 ctest 与真机行为（左右键真的移高亮、整句改字里不被接管）只有代码核对。
