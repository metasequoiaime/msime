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
- 五档用到的值都在每个读取方已经接受的集合里：缩放只用 75 / 100 / 125 / 150，图标只用 16–28。共享文档里不出现任何宿主不认得的新值，所以 macOS 原生回退窗口、Windows 宿主和原生设置页、Linux 的模式徽标、HarmonyOS 面板都不用改（Linux 的模式徽标在各平台跟进时改成了固定大小，不再读这两个值）。
- 存下的缩放和图标凑不成任何一档时（以前在 Windows 原生设置页或旧版本里单独调过），下拉显示一个禁用的「自定义（缩放 X%，图标 Y）」选项，打开页面不改写存储；用户选了某一档才写入。
- Linux 页首说明里的「缩放和图标尺寸不适用」先改成了「工具栏大小不适用」，`docs/windows-parity.md` 的工具栏对照段同步；各平台跟进时发现 Fcitx5 的中英文切换提示其实还在读工具栏大小，改的是宿主而不是这句话（见「各平台跟进」）。

### 屏幕键盘：两个能力位

`client-core` 的 `HostCapabilities` 新增两项，TS 类型里是可选字段，缺省按提供处理（没有宿主的网页预览和旧宿主照旧显示）：

- `touch_keyboard_height`：宿主真的按 `touch_keyboard_height_adjustment` 改变键盘高度。Windows、Android、iOS、HarmonyOS 为真，macOS、Linux 为假。设置页据此隐藏「键盘高度」，预览按默认高度画。
- `touch_keyboard_spacing`：宿主的键盘按两个间距值画。只有 macOS 为假。设置页据此隐藏两个间距滑块、拖动调间距和「恢复默认」，预览按默认间距画；高度和间距都不给时「尺寸」组整组不画。

macOS 上间距是真的固定了：`DesktopKeyboard` 读宿主能力（客户端带的 `host`，或面板窗口自己调 `host_capabilities`），`touch_keyboard_spacing` 为 false 时把默认的 60 / 70 交给 `KeyboardPanel`，不再读存下的值。生产里的面板窗口拿不到客户端带的 `host`，要等 `host_capabilities` 返回；在那之前一律先按默认间距画，否则偏好先到时 macOS 上会先画出存下的旧间距再跳回默认。Windows 和 Linux 在能力返回后照旧按存下的值画，和偏好还没读到时先画默认值是同一种过渡。没有做数据迁移：存下的值留在文档里，macOS 只是不再读它；macOS 不做设置同步（桌面端只有 Windows 接了），这两个值在同步表里也只以 `platform.android.*` 导出。

「恢复默认」的说明和确认框改成只列这一页实际给出的几项（`touchKeyboardResetSubjects`）：例如 Windows 是「高度、间距和顶部语音入口」，Linux 是「间距和顶部语音入口」，HarmonyOS 手机是「高度和间距」，手机上有工具栏时才有「工具栏按钮」。分组把同一句话经 `onReset(subjects)` 交给 `useTouchKeyboardSettingsReset`，说明和确认框只有一个来源。删除哪些字段不变。

macOS 的屏幕键盘页没有任何偏好之后，页脚不再显示全局的「恢复默认设置」，免得让人以为它只恢复这一页。

### 屏幕键盘与手写合成一页

内容顺序部分由[键盘与手写设置拆成顶部标签](../feature/2026-10-19-keyboard-handwriting-top-tabs.md)替代：合并入口、能力位与路由别名继续保留，页内内容通过「键盘」「手写」标签分别显示。

- 在桌面宿主上合并（最初只有 macOS 和 Linux，各平台跟进时加上 Windows 和 HarmonyOS 2-in-1）：`settingsPageProjections` 的 `handwritingOnScreenKeyboardPage` 为真、不是触屏宿主且本版本提供手写时，`handwriting` 不进 `availablePages`，`screen-keyboard` 的标题改为「键盘手写」，屏幕键盘页在键盘各组之后画手写那一组（`HandwritingSettingsContent`，和单独的「手写输入」页是同一个组件）。
- 旧 id 仍然能打开：投影返回 `pageAliases`（`handwriting → screen-keyboard`），`useSettingsNavigation` 在 `selectPage`、宿主路由和启动页上都先查它，`settings:handwriting` 和 `initialPage="handwriting"` 打开合并后的页，而不是落到「输入」。
- 页面 id 与路由不变，导航顺序由 `settingsNavGroups` 统一定义。最初 `SettingsNavigation.h` 也不变、Windows 两页分开，各平台跟进时 Windows 原生窗口同样并成一页（见下）。触屏宿主的手写页另有内容，不并；不提供手写的版本没有可并的页，屏幕键盘页保留原名。

### 桌面菜单名称与顺序

本节的四字侧栏命名和 `desktopPageTitle` 投影由[按功能语义命名](2026-10-19-settings-navigation-semantic-titles.md)取代；下列文字保留旧决定，页面顺序与页内标签不属于这次命名调整。

桌面菜单统一采用四字名称：输入设置、词库管理、按键设置、标点翻译、语音输入、键盘手写、主题外观、候选窗口、快捷工具、剪贴管理、智能辅助、扩展插件、打字统计、账号管理、维护诊断、关于水杉。不提供手写的版本仍叫「屏幕键盘」。共享桌面侧栏连续平铺页面，不显示分类标题。先排输入行为和其他输入方式，再排外观、工具、账号与支持；词库紧接输入，统计放在工具末尾，避免几个功能相邻的页面被外观打断。

共享页的桌面名称走 `desktopPageTitle` 投影，不改 `pages` 的注册名称、路由 id 与 `mobilePageTitle` 的触屏名称。`settingsNavGroups` 继续作为导航顺序的唯一来源，触屏设置列表也按输入方式先于外观排列。Windows 原生窗口同步菜单名称、顺序、组索引和 PowerShell 验收清单。搜索同时匹配显示名称与注册名称，改成「智能辅助」「快捷工具」「按键设置」后仍可用「AI」「工具栏」「快捷键」查找。

参考窗口覆盖测试分别固定菜单名称与内容 `fieldset` 的无障碍名称：四字菜单名不改变页面内的分组名，按页面 id 选择输入页子标签，避免显示名称变化后跳过准备步骤。皮肤跨页测试点击「主题外观」，触屏返回栈测试继续点击手机的「输入」入口。鸿蒙手机的辅助码选择用面板行展示已选值，导入回归核对行上新码表的名称与保存内容；桌面仍核对原生选项。保留参考小节和选项列表的完整断言，修正导航入口不删用例。

桌面页内的顶部标签也统一为四字短名，并沿用各页原有的使用顺序：输入页为「输入方案 / 中英模式 / 选词翻页 / 候选联想 / 输出设置 / 高级选项」，表达页为「标点设置 / 候选释义 / 翻译服务」，词库页为「查找词库 / 管理词库 / 云端词库」，关于页为「版本更新 / 帮助反馈 / 系统信息 / 许可隐私」，维护页为「维护诊断 / AI助手」。主题页已有「本地皮肤 / 社区皮肤」两项，不改触屏宿主专用的「内置 / 我的 / 社区」和社区图库分类名称。页面 id、路由和面板内部标题不随标签文案变化。

### 原生回退窗口和检查脚本

- macOS 原生回退设置窗口没有屏幕键盘的高度和间距控件，没有要删的行；它的工具栏缩放、字号两个弹出菜单和 `sharedPreferencesByMerging:` 写回的值都落在五档用到的集合里，不会把合并后的选择改掉，所以不改。
- `platforms/macos/tests/settings/preference_coverage.py` 里三个触屏几何键原本就列为 macOS 不适用，理由改成写明能力位和面板固定默认值；`scripts/test-reference-config-coverage.py` 的 `floating_toolbar.scale_percent` / `font_size` 仍由「工具栏大小」读写，不需要豁免。

## 各平台跟进

macOS 这一轮之后，Windows、Linux 和 HarmonyOS 逐项对照同样的理由：控件在那个平台上存在、macOS 的理由在那里也成立才跟，触屏宿主需要的设置留着，同步到云端的值不硬砍。

**Windows**

- 原生「悬浮工具栏」页的「尺寸」组把「工具栏缩放」「图标尺寸」两个下拉换成一个「工具栏大小」（`platforms/windows/settings/main.cpp` 的 `toolbar_size_control`），五档和匹配规则放在新的 `FloatingToolbarSizeModel.h`，与 `floatingToolbarSizes` / `floatingToolbarSize` 逐档相同，由 `tests/ui/floating_toolbar_size_model.cpp` 核对。选一档在同一次编辑里写两个值并重画页面；存下的组合凑不成一档时，最前面是一项灰着的「自定义（缩放 X%，图标 Y）」并选着它，不改写存储（用的是 `select_control` 已有的不可选选项）。175% 仍写成 150% 加 28，`FloatingToolbarSettings::valid()` 不用改。原生页不再能调出不成档的组合，共享页上的「自定义」只剩旧数据会触发。
- 原生「屏幕键盘」页的「恢复默认」说明和确认框改成「高度、间距和顶部语音入口」，与共享页给 Windows 的 `touchKeyboardResetSubjects` 一致；删掉的五个键（含 `touch_toolbar`）不变。
- 「手写输入」并进「屏幕键盘」页：Windows 的手写页只有一个打开面板的按钮，macOS 的理由成立。`SettingsNavigation.h` 去掉 `hand` 页（18 页变 17 页），`handwriting` 路由改指 `osk`；`page_offered` 只用来收掉 `hand` 页，一并删掉，改由 `offered_page_for_route` 在不提供手写的版本里把 `handwriting` 路由送到默认页，对应共享页只在有手写时才有 `handwriting` 别名。`build_osk_page` 在「工具栏」和「外观」之间画 `build_hand_page` 那一组，页名在提供手写的版本里是「键盘手写」。共享页的 `handwritingOnScreenKeyboardPage` 同时对 Windows 打开，两边侧栏一致。`tests/ui/settings_navigation.cpp`、`tests/tools/settings_ui.ps1` 和 README 同步。
- 不跟：键盘高度和两种间距都留着。macOS 去掉间距的决定性理由是 Mac 没有触屏，而 Windows 有 Surface 这类二合一触屏设备，面板本身写着「使用鼠标或触控方式」；Tauri 面板在 Windows 上按存下的高度定窗口高度（`panel_window.rs` 的 `windows_panel_height`），`touch_keyboard_spacing` 为真时也按存下的间距画，所以两个能力位都不改。`desktop-keyboard.tsx` 在能力返回前先画默认几何的改动是共享代码，Windows 面板已经带上。

**Linux**

- 「工具栏大小」不显示（`floating_toolbar_appearance` 为假，页面不画「尺寸」组）；高度已经隐藏、间距保留（Linux 有触屏笔记本和平板，面板也实时应用两种间距）；页面合并在 macOS 那一轮已经上线。
- 发现的问题：页首说明写「工具栏大小不适用」，但 Fcitx5 宿主的中英文切换提示（`platforms/linux/src/overlay/ModeBadgeStyle.h`）原来由 `FcitxEngine.cpp` 经 `mode_badge_metrics` 读 `floating_toolbar.scale_percent` 和 `font_size` 定大小，页面上又没有能改它的控件，从 Windows 或 macOS 导入的设置会悄悄改变徽章大小。按「砍掉的设置由宿主改用固定默认值」的约定修在宿主上：徽章改用固定的 `ModeBadgeMetrics{}`（24 号、100%，也就是绝大多数 Linux 用户一直看到的大小），`mode_badge_metrics` 删掉，`tests/runtime/mode_badge_style.cpp` 改成核对固定大小。这和 macOS 2026-10-14 的做法一致（[macOS 中英文切换提示使用紧凑尺寸](../bug-fix/2026-10-14-macos-compact-input-mode-hud.md)：提示有自己的尺寸，工具栏大小不再影响它）。页首说明保持「工具栏大小不适用」，现在它在 Fcitx5 上也是实话。没有顺带把 Linux 徽章缩成 macOS 的 18pt / 32pt 紧凑尺寸，那是外观改动，不是精简。IBus 不画这个徽章，GNOME Shell 那句说明不变。没有数据迁移，存下的工具栏值留在文档里给其他平台用。

**HarmonyOS**

- 手机：全部保留。它是触屏输入法，高度和间距都由 ArkTS 键盘读取（`KeyboardSession.ets`），并以 `platform.harmony.*` 同步到云端，不能硬砍；手机的手写页有识别等待时间、笔迹颜色和粗细，不并进屏幕键盘页（投影本身排除触屏宿主）；手机没有悬浮工具栏页。
- 2-in-1：「工具栏大小」由共享页直接带上，`FloatingToolbarLayout.ts` 和 `InputModeHud` 接受每一档（150 加 28 画成 175%）。手写页在 2-in-1 上只是一段说明，而且说明本身就让人从屏幕键盘的方案选择器切到手写，所以同样并进「键盘手写」，`handwritingOnScreenKeyboardPage` 对 HarmonyOS 打开即可。两个触屏几何能力位在 2-in-1 上照旧为真，键盘仍按它们画。

## Alternatives considered

- **直接把注册名称全部改成四个字**：一份名称覆盖所有地方，代码最少。不用它，是因为手机底部标签和列表已有适合窄屏的短名，桌面整理菜单不应改变手机术语；桌面投影也能保留原名供搜索。
- **为每个平台各维护一份菜单顺序**：可以完全保留手机现有排序。不用它，是因为输入方式靠近输入设置对两种布局都适用，重复清单还会让共享桌面与 Windows 的导航继续漂移。

- **新增 175 这个缩放值，控件只写 `scale_percent`、把 `font_size` 重置成 24**：存储模型最简单，一个数就是大小。不用它，是因为每个读取方都把缩放限在 75–150：Windows `FloatingToolbarSettings::valid()` 会整份拒绝工具栏设置，PreviewConfig 只接受 0.75–1.5（`CandidateAppearance.h` 因此把超范围的缩放直接丢掉）；macOS `FloatingToolbarPanel.mm` 和 `AppearancePreferences.mm` 的 `ValidToolbarScale` 会回落到 100，而 `sharedPreferencesByMerging:` 每次保存外观都会把 100 写回去；Linux 的 `ModeBadgeStyle.h` 当时也只认四个值（各平台跟进后它不再读缩放）。要支持它就得改 Windows 宿主，超出这次范围，而 Mac 上选的 175 一旦同步到 Windows，就会让那边的工具栏设置整份失效。
- **升级时把 `font_size` 一次性折算进 `scale_percent`（调研报告的建议）**：之后只剩一个值。不用它，是因为这是所有宿主上的数据迁移，Windows 原生页还在分开调两项，折算后它显示的值会变；这次的约定是不做数据迁移，Windows 对齐另行跟进。
- **保留锁定的「中英文切换」行，只给 `english_mode` 改名**：最小改动。不用它，是因为锁定行说的「始终显示」在每个宿主上都不成立，留着它就还是一个按钮两种说法。
- **macOS 上保留间距滑块**：最强的理由是间距本身是可达性需求，键距太挤的用户想放宽。不用它，是因为默认 60 已经是上限，滑块只能把键挤得更密；这是给手指设计的微调，鼠标操作的面板用不着。
- **只隐藏间距控件，存下的值继续生效**：改动最小。不用它，是因为[撤掉数字键盘设置的先例](2026-10-11-remove-number-keypad-settings.md)明确否决了「藏起行、已存的值继续生效」：改过的人从此改不回来。
- **给 macOS 和 Linux 的 Tauri 面板接上高度调整，让「键盘高度」生效**：控件就有了用处。不用它，是因为这是新功能而不是精简；哪个宿主接上了，就在 `host_surface.rs` 把它的 `touch_keyboard_height` 打开。
- **Windows 保持两页，只在 macOS 和 Linux 上合并**：这是 macOS 那一轮的做法，理由是 `SettingsNavigation.h` 不只是一份列表（`main.cpp` 按页 id 画「手写输入」页，`settings_navigation.cpp` 断言页数，`page_offered` 按版本收掉 `hand` 页），要改 Windows 原生设置页。各平台跟进时做了这部分改动，Windows 的手写页同样只剩一个按钮，所以不再保留两页；Windows 屏幕键盘页还有高度和间距，并不影响把一个按钮并进来。
- **Linux 只改页首说明，写明 Fcitx5 的切换提示沿用存着的工具栏大小**：各平台跟进最初就是这样做的，不动宿主。不用它，是因为说明等于告诉用户有一个看不到也改不了的设置在起作用，违反「砍掉的设置由宿主改用固定默认值」的约定，而 macOS 2026-10-14 已经让切换提示脱离工具栏大小，有现成的先例。
- **在 Fcitx5 上把「工具栏大小」改名成「切换提示大小」显示出来**：徽章大小就有了入口。不用它，是因为这是给 Linux 加控件，和这次精简的方向相反；短暂、鼠标穿透的提示也用不着可调大小，macOS 同样没有给它控件。
- **把两个「打开」按钮挪到首页**：桌面设置没有首页（`hasHomePage` 只在触屏宿主上为真），没有地方放。
- **只调整分段控件的宽度、不改标签文案**：可以避免测试和辅助文本跟着改。不用它，是因为同一页里两字、三字、五字标签混排，用户扫读时仍然不齐；顶部标签的四字短名不改变面板内容或路由，改动边界清楚。

## Consequences

- **收益**：macOS 侧栏少一项；悬浮工具栏页少两行（锁定行和第二个尺寸下拉）；macOS 屏幕键盘页从四个尺寸控件、一个拖动预览和一个重置按钮变成只剩打开按钮和静态预览；Linux 少一个不起作用的高度滑块；「恢复默认」的文案在每个宿主上都只说看得到的东西；桌面页内标签统一成四字短名，分段控件在不同页面更容易扫读；触屏专用标签继续保留设计稿用词。
- **代价**：
  - 在 macOS 上调过间距的用户，升级后键盘面板回到默认间距，而且无处可改；存下的值仍在文档里，没人读。
  - 以前在 Windows 原生页或旧版本里把缩放和图标调成不成档的组合，共享页和 Windows 原生页上都会看到「自定义」，只能换成某一档、不能再微调。
  - 共享页按宿主改了一页的标题，这是桌面端第一次按平台改页标题，走的是投影而不是注册表。各平台跟进后四个桌面宿主（含 HarmonyOS 2-in-1）和 Windows 原生窗口的侧栏重新一致。
  - Windows 原生屏幕键盘页「恢复默认」的说明和确认框原来还写着「工具栏按钮」，各平台跟进时已改成跟随 `touchKeyboardResetSubjects`。
  - Linux Fcitx5 上以前借用（例如从其他平台导入的）工具栏大小放大或缩小了中英文切换提示的用户，升级后提示回到固定的默认大小。
  - `host_surface.rs` 多两个能力位。HarmonyOS 的设置投影不覆盖它们，手机和 2in1 都按 `for_platform(Harmony)` 为真。
- 按钮选项模型这篇[悬浮工具栏页面共用按钮与尺寸选项模型](2026-10-16-floating-toolbar-options.md)和[触屏键盘几何偏好](2026-10-14-touch-keyboard-geometry-preferences.md)的事实已就地更新并链回这里；[Windows 原生屏幕键盘页](../feature/2026-10-11-windows-settings-screen-keyboard.md)描述的 Windows 共享页四组布局不变。

## Verification

- `cargo test -p msime-client-core` 全部通过，其中新增 `touch_keyboard_geometry_is_offered_where_the_keyboard_applies_it` 逐平台核对两个能力位；`cargo clippy -p msime-client-core --all-targets -- -D warnings` 通过（经 `rbuild` 在编译机上跑，本机盘剩余低于 50 GB）。
- `pnpm --filter @msime/desktop typecheck`、`pnpm --filter @msime/desktop test`、`pnpm lint`、`pnpm format:check` 通过。新增或改写的用例覆盖：按钮清单没有锁定项、五档的取值和最大尺寸、凑不成一档时显示「自定义」且不改写、macOS 屏幕键盘页没有尺寸组且预览画默认几何、Linux 只去掉高度、Windows 两页分开且控件齐全、`handwriting` 旧 id 经路由和启动页打开合并页、`DesktopKeyboard` 在 macOS 上不理存下的间距（含面板窗口经 `host_capabilities` 取能力、偏好先到的情形）、「恢复默认」说明和确认框同一句话。
- `scripts/test-appearance-preview.py`（本地 Playwright 回归，不在 CI 里）原来逐个组合拨两个下拉，改为逐档拨「工具栏大小」并核对图标宽度；这次没有在本机跑它。
- `platforms/macos/tests/settings/preference_coverage.py` 与 `scripts/test-reference-config-coverage.py` 通过。
- 没有在本机编译 macOS 或 Windows 原生宿主（这次没有改原生代码），也没有启动应用或面板看实际效果。
- 各平台跟进：`FloatingToolbarSizeModel.h` 的测试和 `tests/ui/settings_navigation.cpp` 在本机用 clang 按 C++17 和 C++20 加 `-Wall -Wextra -Werror` 编译运行通过；`main.cpp` 的改动没有在本机编译（WinUI 只能在 CI 的 Windows 机器上编译），`settings_ui.ps1` 新加的 UIA 断言也要等 CI 跑。共享页的用例改成 Windows 和 HarmonyOS 2-in-1 都落在「键盘手写」，参考窗口的小节表在 Windows 上同样按合并后的页查找，Linux 的 `tests/runtime/mode_badge_style.cpp` 在本机用 clang 按 C++17 加 `-Wall -Wextra -Werror` 编译运行通过，`FcitxEngine.cpp` 的改动只能交给 CI 的 Linux 构建。

- 菜单名称与顺序：Chromium 合成宿主在浅色、深色以及 1000×780、800×600 下核对 16 项完整导航，名称均为四个字、等高且无横向溢出；「AI」「工具栏」「快捷键」「屏幕键盘」搜索与 Enter 跳转、Escape 清空、无结果提示均通过。Windows 导航模型在本机以 C++20、`MSIME_EDITION_FULL` 与 `-Wall -Wextra -Werror` 编译运行通过，PowerShell 验收顺序检查通过。WinUI 窗口需由 Windows CI 构建，本机没有安装这次菜单改动。

共享表单中的手写合并挂载判断与无偏好页面的恢复默认策略由[偏好设置表单组件](2026-10-19-settings-preferences-form.md)原样持有；平台投影及各页面实现的归属不变。
