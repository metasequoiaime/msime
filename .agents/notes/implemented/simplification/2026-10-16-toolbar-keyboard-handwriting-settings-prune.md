# Agent Note: 精简悬浮工具栏、屏幕键盘和手写输入三页的设置

Status: implemented

## Problem

macOS 设置项精简调研逐项核对了「悬浮工具栏」「屏幕键盘」「手写输入」三页，结论是这三页的问题不在开关多，而在几处控件说的和宿主做的对不上：

- 悬浮工具栏的按钮清单里有一行锁定勾选的「中英文切换（始终显示）」，下面又有一项「英文输入模式」。两行说的是同一个中/英按钮，而这个按钮由 `floating_toolbar.english_mode` 控制、可以关掉：macOS `FloatingToolbarPanel.mm` 的 `applySizingPreferences:` 注释写着每个按钮都能关，Windows 的 `FloatingToolbarSettings.language`、Linux 的 `ClientEngine.cpp` / `FcitxEngine.cpp` 和 HarmonyOS 的 `KeyboardSession.ets` 也都读它。「英文输入模式」这个名字还和 ⌃⇧E 的独立英文模式撞名。
- 「工具栏缩放」和「图标尺寸」是两个下拉，宿主把两者相乘才得到按钮和图标的大小（macOS `FloatingToolbarPanel.mm` 的 `(fontSize + padding) * scale`，Windows `FloatingToolbarWindow.cpp` 的 `font_size_ * unit`），控件说明里自己写着「再乘以上方缩放」。
- 屏幕键盘的「键盘高度」（`touch_keyboard_height_adjustment`）只在 Windows 上起作用：`panel_window.rs` 的 `windows_panel_height` 带 `#[cfg(target_os = "windows")]`，macOS 和 Linux 的 Tauri 键盘窗口高度固定，macOS 原生回退面板写死 400pt。设置页的预览却会跟着滑块变高。
- 按键间距、行间距、在预览上拖动调间距和「恢复默认」是触屏键盘的微调，macOS 上只有 Tauri 键盘面板（`desktop-keyboard.tsx`）读这两个值，默认的 60 已经是滑块上限。「恢复默认」的说明和确认框在桌面上还提到页面根本不显示的「工具栏按钮」。
- 砍完之后，macOS 上「屏幕键盘」和「手写输入」两页各自只剩一个「打开」按钮加说明。

## Decision

### 悬浮工具栏

- 共享页去掉锁定的「中英文切换（始终显示）」一行，`english_mode` 那一项改名为「中英文切换」，和 Windows 原生设置页同名。键不变，所有宿主一起生效。`floatingToolbarButtonOptions` 不再返回伪项 `mode_switch`，`settings-style.ts` 里只为它存在的 `toolbarRequiredLabel` 一并删除。
- 「工具栏缩放」和「图标尺寸」合成一个「工具栏大小」下拉，五档 75%、100%、125%、150%、175%（`floatingToolbarSizes`）。每一档同时写 `scale_percent` 和 `font_size`：前四档是对应的缩放加默认的 24 号图标，175% 是 150% 缩放加 28 号图标（150 × 28 = 175 × 24），和以前两个下拉都拉满时一样大。宿主画间距、logo 和分隔线只乘缩放，所以从 150% 到 175% 只有按钮和图标变大、间距停在 150%；控件说明因此只写「按钮和图标一起放大或缩小」。
- 五档用到的值都在每个读取方已经接受的集合里：缩放只用 75 / 100 / 125 / 150，图标只用 16–28。共享文档里不出现任何宿主不认得的新值，所以 macOS 原生回退窗口、Windows 宿主和原生设置页、Linux 的模式徽标、HarmonyOS 面板都不用改。
- 存下的缩放和图标凑不成任何一档时（在 Windows 原生设置页或旧版本里单独调过），下拉显示一个禁用的「自定义（缩放 X%，图标 Y）」选项，打开页面不改写存储；用户选了某一档才写入。
- Linux 页首说明里的「缩放和图标尺寸不适用」改成「工具栏大小不适用」，`docs/windows-parity.md` 的工具栏对照段同步。

### 屏幕键盘：两个能力位

`client-core` 的 `HostCapabilities` 新增两项，TS 类型里是可选字段，缺省按提供处理（没有宿主的网页预览和旧宿主照旧显示）：

- `touch_keyboard_height`：宿主真的按 `touch_keyboard_height_adjustment` 改变键盘高度。Windows、Android、iOS、HarmonyOS 为真，macOS、Linux 为假。设置页据此隐藏「键盘高度」，预览按默认高度画。
- `touch_keyboard_spacing`：宿主的键盘按两个间距值画。只有 macOS 为假。设置页据此隐藏两个间距滑块、拖动调间距和「恢复默认」，预览按默认间距画；高度和间距都不给时「尺寸」组整组不画。

macOS 上间距是真的固定了：`DesktopKeyboard` 读宿主能力（客户端带的 `host`，或面板窗口自己调 `host_capabilities`），`touch_keyboard_spacing` 为 false 时把默认的 60 / 70 交给 `KeyboardPanel`，不再读存下的值。生产里的面板窗口拿不到客户端带的 `host`，要等 `host_capabilities` 返回；在那之前一律先按默认间距画，否则偏好先到时 macOS 上会先画出存下的旧间距再跳回默认。Windows 和 Linux 在能力返回后照旧按存下的值画，和偏好还没读到时先画默认值是同一种过渡。没有做数据迁移：存下的值留在文档里，macOS 只是不再读它；macOS 不做设置同步（桌面端只有 Windows 接了），这两个值在同步表里也只以 `platform.android.*` 导出。

「恢复默认」的说明和确认框改成只列这一页实际给出的几项（`touchKeyboardResetSubjects`）：例如 Windows 是「高度、间距和顶部语音入口」，Linux 是「间距和顶部语音入口」，HarmonyOS 手机是「高度和间距」，手机上有工具栏时才有「工具栏按钮」。分组把同一句话经 `onReset(subjects)` 交给 `useTouchKeyboardSettingsReset`，说明和确认框只有一个来源。删除哪些字段不变。

macOS 的屏幕键盘页没有任何偏好之后，页脚不再显示全局的「恢复默认设置」，免得让人以为它只恢复这一页。

### 屏幕键盘与手写合成一页

- 只在 macOS 和 Linux 桌面上合并：`settingsPageProjections` 的 `handwritingOnScreenKeyboardPage` 为真且本版本提供手写时，`handwriting` 不进 `availablePages`，`screen-keyboard` 的标题改为「屏幕键盘与手写」，屏幕键盘页在键盘各组之后画手写那一组（`HandwritingSettingsContent`，和单独的「手写输入」页是同一个组件）。
- 旧 id 仍然能打开：投影返回 `pageAliases`（`handwriting → screen-keyboard`），`useSettingsNavigation` 在 `selectPage`、宿主路由和启动页上都先查它，`settings:handwriting` 和 `initialPage="handwriting"` 打开合并后的页，而不是落到「输入」。
- `settings-page-registry.ts` 的 `pages` 和 `settingsNavGroups` 不变，`SettingsNavigation.h` 也不变。Windows 的共享设置页仍分成两页，与原生设置窗口的侧栏一致；触屏宿主的手写页另有内容，不并；不提供手写的版本没有可并的页，屏幕键盘页保留原名。

### 原生回退窗口和检查脚本

- macOS 原生回退设置窗口没有屏幕键盘的高度和间距控件，没有要删的行；它的工具栏缩放、字号两个弹出菜单和 `sharedPreferencesByMerging:` 写回的值都落在五档用到的集合里，不会把合并后的选择改掉，所以不改。
- `platforms/macos/tests/settings/preference_coverage.py` 里三个触屏几何键原本就列为 macOS 不适用，理由改成写明能力位和面板固定默认值；`scripts/test-reference-config-coverage.py` 的 `floating_toolbar.scale_percent` / `font_size` 仍由「工具栏大小」读写，不需要豁免。

## Alternatives considered

- **新增 175 这个缩放值，控件只写 `scale_percent`、把 `font_size` 重置成 24**：存储模型最简单，一个数就是大小。不用它，是因为每个读取方都把缩放限在 75–150：Windows `FloatingToolbarSettings::valid()` 会整份拒绝工具栏设置，PreviewConfig 只接受 0.75–1.5（`CandidateAppearance.h` 因此把超范围的缩放直接丢掉）；macOS `FloatingToolbarPanel.mm` 和 `AppearancePreferences.mm` 的 `ValidToolbarScale` 会回落到 100，而 `sharedPreferencesByMerging:` 每次保存外观都会把 100 写回去；Linux 的 `ModeBadgeStyle.h` 也只认四个值。要支持它就得改 Windows 宿主，超出这次范围，而 Mac 上选的 175 一旦同步到 Windows，就会让那边的工具栏设置整份失效。
- **升级时把 `font_size` 一次性折算进 `scale_percent`（调研报告的建议）**：之后只剩一个值。不用它，是因为这是所有宿主上的数据迁移，Windows 原生页还在分开调两项，折算后它显示的值会变；这次的约定是不做数据迁移，Windows 对齐另行跟进。
- **保留锁定的「中英文切换」行，只给 `english_mode` 改名**：最小改动。不用它，是因为锁定行说的「始终显示」在每个宿主上都不成立，留着它就还是一个按钮两种说法。
- **macOS 上保留间距滑块**：最强的理由是间距本身是可达性需求，键距太挤的用户想放宽。不用它，是因为默认 60 已经是上限，滑块只能把键挤得更密；这是给手指设计的微调，鼠标操作的面板用不着。
- **只隐藏间距控件，存下的值继续生效**：改动最小。不用它，是因为[撤掉数字键盘设置的先例](2026-10-11-remove-number-keypad-settings.md)明确否决了「藏起行、已存的值继续生效」：改过的人从此改不回来。
- **给 macOS 和 Linux 的 Tauri 面板接上高度调整，让「键盘高度」生效**：控件就有了用处。不用它，是因为这是新功能而不是精简；哪个宿主接上了，就在 `host_surface.rs` 把它的 `touch_keyboard_height` 打开。
- **Windows 也合并两页，同步改 `SettingsNavigation.h`**：三个桌面一致。不用它，是因为那个头文件不只是一份列表：`main.cpp` 按页 id 画「手写输入」页，`settings_navigation.cpp` 断言正好 18 页，`page_offered` 按版本收掉 `hand` 页；Windows 的屏幕键盘页也还有高度和间距，不是只剩一个按钮。这要改 Windows 原生设置页，留给 Windows 对齐时一起做。
- **把两个「打开」按钮挪到首页**：桌面设置没有首页（`hasHomePage` 只在触屏宿主上为真），没有地方放。

## Consequences

- **收益**：macOS 侧栏少一项；悬浮工具栏页少两行（锁定行和第二个尺寸下拉）；macOS 屏幕键盘页从四个尺寸控件、一个拖动预览和一个重置按钮变成只剩打开按钮和静态预览；Linux 少一个不起作用的高度滑块；「恢复默认」的文案在每个宿主上都只说看得到的东西。
- **代价**：
  - 在 macOS 上调过间距的用户，升级后键盘面板回到默认间距，而且无处可改；存下的值仍在文档里，没人读。
  - 在 Windows 原生页里把缩放和图标调成不成档的组合，共享页上会看到「自定义」，只能换成某一档、不能在这里微调。
  - 共享页按宿主改了一页的标题，这是桌面端第一次按平台改页标题，走的是投影而不是注册表；Windows 的共享设置页（用户实际用原生窗口）和 macOS、Linux 的侧栏不再相同。
  - Windows 原生屏幕键盘页（`platforms/windows/settings/main.cpp` 的「恢复默认」行和 `reset_screen_keyboard` 的确认框）仍写着「高度、间距、顶部语音入口和工具栏按钮」，和共享页给 Windows 的「高度、间距和顶部语音入口」不一致。这次不改 Windows 原生页，Windows 对齐时让它跟随 `touchKeyboardResetSubjects`。
  - `host_surface.rs` 多两个能力位。HarmonyOS 的设置投影不覆盖它们，手机和 2in1 都按 `for_platform(Harmony)` 为真。
- 按钮选项模型这篇[悬浮工具栏页面共用按钮与尺寸选项模型](2026-10-16-floating-toolbar-options.md)和[触屏键盘几何偏好](2026-10-14-touch-keyboard-geometry-preferences.md)的事实已就地更新并链回这里；[Windows 原生屏幕键盘页](../feature/2026-10-11-windows-settings-screen-keyboard.md)描述的 Windows 共享页四组布局不变。

## Verification

- `cargo test -p msime-client-core` 全部通过，其中新增 `touch_keyboard_geometry_is_offered_where_the_keyboard_applies_it` 逐平台核对两个能力位；`cargo clippy -p msime-client-core --all-targets -- -D warnings` 通过（经 `rbuild` 在编译机上跑，本机盘剩余低于 50 GB）。
- `pnpm --filter @msime/desktop typecheck`、`pnpm --filter @msime/desktop test`、`pnpm lint`、`pnpm format:check` 通过。新增或改写的用例覆盖：按钮清单没有锁定项、五档的取值和最大尺寸、凑不成一档时显示「自定义」且不改写、macOS 屏幕键盘页没有尺寸组且预览画默认几何、Linux 只去掉高度、Windows 两页分开且控件齐全、`handwriting` 旧 id 经路由和启动页打开合并页、`DesktopKeyboard` 在 macOS 上不理存下的间距（含面板窗口经 `host_capabilities` 取能力、偏好先到的情形）、「恢复默认」说明和确认框同一句话。
- `scripts/test-appearance-preview.py`（本地 Playwright 回归，不在 CI 里）原来逐个组合拨两个下拉，改为逐档拨「工具栏大小」并核对图标宽度；这次没有在本机跑它。
- `platforms/macos/tests/settings/preference_coverage.py` 与 `scripts/test-reference-config-coverage.py` 通过。
- 没有在本机编译 macOS 或 Windows 原生宿主（这次没有改原生代码），也没有启动应用或面板看实际效果。
