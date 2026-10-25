# Agent Note: 跨平台候选窗的渲染边界与共享契约

Status: implemented

## Problem

一份跨平台候选窗方案要求"共享语义数据与主题协议 + 平台原生窗口/面板 + 平台原生文本绘制"，并建议先审计各宿主的真实渲染路径，再为缺失的语义补一套 `CandidateWindowView { session_id, generation, items[id], selected_id, … }` 契约和一张平台能力表。动手之前需要知道哪些已经存在：如果把已有的身份围栏再发明一遍，就会出现两套互相竞争的候选状态源，这正是方案自己禁止的。

## Decision

候选窗维持各宿主原生渲染，共享的只有语义、主题和能力声明，不新增一套候选视图 ABI。三块共享契约各有一个权威来源：

- **候选语义**：`crates/input-runtime/src/types.rs` 的 `View` 和 `Candidate`。每个候选的身份是 `CandidateId { session, generation, index }`，其中 `index` 是整张列表里的全局序号，不是页内格位。方案里的 `session_id` / `generation` / 稳定候选 ID 就是这个三元组，宿主必须原样回传，不能用展示文字或页内格位重建。
- **选择围栏**：`msime_client_select` 及同族调用由运行时校验（`runtime.rs` 的 `StaleCandidate`：会话或代次不符、序号不在当前页即拒绝）。任何重排都必须推进代次，这条不变量让"同代次、同序号"可以等价于"同一个候选"。宿主在调用前另外复核一次（macOS `MSIMECurrentCandidateIdentity`、Fcitx5 `FcitxState::select`、Windows `SessionController::request_selection`），复核只能比运行时更严，不能替代运行时。宿主自己合成的列表（Windows 的 Ctrl+Enter 译文页、IBus 的译文浮层）不经过运行时围栏，必须带上自己能区分新旧的身份。
- **主题与能力**：颜色以 `msime_client_resolve_theme` 的 `ResolvedTheme` 为准，圆角、内边距和字体来自偏好与皮肤包。宿主能画什么由 `HostCapabilities`（`crates/client-core/src/host_surface.rs`）静态声明；Linux 实际面板的限制在运行时通过 `candidate_panel_limit` 报告，设置页按它标注不生效的项。

简繁转换的展示与上屏一致性同样已由共享层保证：每个宿主都委托同一份 OpenCC 表（`msime::host_api::simplified_to_traditional`），是否转换由运行时在 view 和 commit 上下文里各写一次 `script_conversion`，`input-runtime` 的测试逐方案断言两者一致、日文混输行两边都关闭。

## 各宿主的渲染路径（2026-10-13 审计）

- **Windows**：TSF DLL 不画任何候选，只经管道发 Show/Move/Hide；候选卡由 Server 进程主线程的 `CandidateWindow` 用 Direct2D/DirectWrite 绘制，读 `CandidateMailbox` 的快照，画完回一张带 `render_serial` 的回执，按键最多等它 30 ms。候选没有任何 HTML/WebView2 路径，WebView2 只用于 Tauri 设置面板。窗口为 `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST`，`WM_MOUSEACTIVATE` 返回不激活；锚点来自 `ITfContextView::GetTextExt` 并按每显示器 DPI 换算。
- **macOS**：产品候选窗是 `InputController.mm` 里的 `MSIMECandidatePanel`（`NonactivatingPanel`、`becomesKeyOnlyIfNeeded`、加入所有 Space 和全屏辅助），按钮用 AppKit 文本测量；`CandidatePanel.mm` 只给测试用，不是产品路径。锚点来自 IMK 的 `attributesForCharacterIndex:lineHeightRectangle:`，按 `visibleFrame` 夹紧并在下方放不下时翻到上方。异步结果（释义、云、AI）都在主线程按 epoch、会话或代次校验后才落到界面。
- **Linux**：IBus 把候选填进 `IBusLookupTable`，Fcitx5 用自定义 `CandidateList`，像素、定位和指针都由桌面面板（ibus-ui-gtk3、GNOME Shell、Fcitx5 classicui、kimpanel）负责。MSIME 只把主题映射成 classicui 的 `theme.conf`（原子写入、内容不变不写、只接管默认或本项目主题、卸载按记录恢复），IBus 只写面板字体。候选窗从不由本项目另开顶层窗口绘制。

## 性能基线

审计时没有采到三平台的实机数据，下面记录的是现有的测量入口和预算，供下一次采基线时直接使用：

- macOS 诊断日志的 `candidate-frame show … build_ms=` 记录每次排版耗时（`InputController.mm`），打开诊断日志即可采样。
- Windows 没有候选绘制计时；可用的上限是按键等候选回执的 30 ms（`CandidateRenderSync.h` 的 `candidate_render_wait_max_ms`）；回执未到时屏障先叫醒候选窗立即绘制，见 [渲染屏障叫醒候选窗](../bug-fix/2026-10-25-windows-candidate-render-wake.md)。DirectWrite 文本格式有 128 项 LRU，文本布局不缓存，每次刷新和每帧闪光都重新测量。空闲时 Server 主循环每 50 ms 醒一次，候选卡本身没有常驻帧循环。
- Linux 面板由桌面绘制，本项目可测的只有填表耗时；Fcitx5 每个聚焦上下文有一个 250 ms 的轮询，每次读运行时选项和 `classicui.conf`，内容变化时才写主题。

## Alternatives considered

- **按方案示例新增 `CandidateWindowView` ABI**：能把 `selected_id`、`page_count`、`direction` 等收拢成一份视图结构。但会话、代次和候选身份已经在 `View` 里，并且有运行时围栏和测试守着；再加一套会让宿主面对两个状态源，旧快照覆盖新会话的问题反而更难排查。不采用，缺的字段直接加到现有 `View` 或能力表。
- **统一的 Web/Tauri 候选窗**：一套代码三处复用，动效和皮肤最容易一致。代价是在 TSF、InputMethodKit 和 Wayland 面板之外再造窗口，焦点、定位和延迟都要重新证明，Wayland 下普通顶层窗口拿不到全局坐标。不采用。
- **Linux 用 Cairo/Pango 自绘浮窗**：可以和 Windows、macOS 像素一致。Wayland 普通客户端不能把窗口放到文本光标旁，GNOME 和 KDE 上会出现双候选窗或抢焦点。只有在合成器的输入法 popup 角色或 Fcitx5 UI 插件路径被证明可用时才重新评估。

## Consequences

收益：候选身份、选择围栏、简繁一致性和主题解析只有一个权威来源，宿主各自只负责窗口、定位和文字绘制；审计找到的缺陷都落在宿主自合成列表和宿主侧复核上，修复不需要改 ABI。

代价：能力表是按平台的静态声明，Linux 的两个入口（IBus、Fcitx5）共享一组标志，真实面板差异只能靠运行时 `candidate_panel_limit` 补充；三平台的实机性能基线仍待采集，目前只能用上面的埋点和预算做相对比较。宿主每新增一个自合成列表，都必须自己带上可区分新旧的身份，这条规则没有编译期检查，只能靠评审和各宿主的过期点击测试。

相关记录：[Windows 候选窗与 Mac 对齐](../feature/2026-10-10-windows-candidate-window-mac-parity.md)、[Windows Server 窗口的 UI 自动化](../feature/2026-10-10-windows-server-window-ui-automation.md)、[Fcitx5 系统主题保留品牌样式](../feature/2026-10-09-fcitx5-system-theme-keeps-brand-style.md)。

macOS 系统主题的背景材质见[候选卡片液态玻璃](../feature/2026-10-19-macos-candidate-liquid-glass.md)，候选身份、按钮归属与原生定位边界保持不变。

Fcitx5 的静态 canvas 面板通过 [classicui 九宫格映射](../feature/2026-10-19-linux-fcitx-canvas-panel.md)接入，候选窗口、定位与指针继续归桌面；该通路经[共享图片解码](../feature/2026-10-20-linux-canvas-image-formats.md)显示 PNG／APNG 默认图与 GIF／WebP 首帧，不代表动画或其他面板完成验收。

X11 canvas 的窗口所有权由[原生 X11 画布](2026-10-20-linux-native-canvas-x11.md)部分取代：有效 canvas 包、X11 输入上下文和水杉持有的 classicui 主题使用原生候选窗，挂件与独立行距由该路径绘制；其余上下文继续使用本篇的桌面面板路径。静态图片不代表接通动画，Wayland 与其他 UI 的缺口仍保留。
