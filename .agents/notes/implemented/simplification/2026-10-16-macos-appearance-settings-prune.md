# Agent Note: macOS 主题页与候选窗口页精简

Status: implemented

## Problem

macOS 设置里「主题」和「候选窗口」两页的控件太多，而且大多是平铺的微调项。「主题」页的「高级」组有八个下拉框，每个给一个界面（设置窗口、屏幕键盘、候选窗口、悬浮工具栏、菜单、表情面板、手写识别板、语音弹出条）单独定明暗；「自定义主题」组的七个取色器一直展开。「候选窗口」页分布局、字体与大小、窗口样式、预编辑四个标签，一共二十多行。其中有几处是同一个决定拆成了两个控件：「候选字体」预设和「主字体」写的是同一个 `candidate_font_family`；「系统字体列表」整行只是字体框的读取状态和刷新按钮；「窗口样式」标签收起不透明度、圆角和内边距之后，只剩一行跳到侧栏已有的「主题」页的链接。

调研和核查记录在这次精简的研究报告里，逐项结论见下面的 Decision 和 Alternatives considered。

## 后续决定

[候选窗口顶部功能标签](../feature/2026-10-18-candidate-window-top-tabs.md)取代下文共享桌面候选页的一列与「更多选项」排版决定，页面重新使用 `candidateTabs` 模型。下文保留当时的理由；宿主能力、配色、手机布局及 Windows 原生设置的决定不受这次排版调整影响。

## Decision

### 按宿主能力位固定的值

这些改动改变行为，所以按宿主能力位收口，不在页面里判断平台名。最初只在 macOS 上做，随后 Windows、Linux 和 HarmonyOS 跟进（见下面的「各平台跟进」），现在只有 Android 和 iOS 照旧读取。宿主停止读取存下的值，改用固定的默认值；不做数据迁移，文档里原有的值留着。这几项都不在云同步表里（`crates/client-core/src/account/settings_sync.rs` 只同步 `theme`、`global_theme` 和自定义主题的底与皮肤槽位），不会出现「别的设备改了、Mac 上看不见却生效」的情况。

- `HostCapabilities::surface_themes_follow`（macOS、Windows、Linux、HarmonyOS 为真）：设置窗口、屏幕键盘、悬浮工具栏与中英文提示、菜单、表情面板、手写识别板、语音弹出条一律跟随颜色模式，不读 `settings_theme`、`screen_keyboard_theme`、`toolbar_theme`、`menu_theme`、`emoji_theme`、`handwriting_theme`、`voice_theme`。
  - 共享设置页：主题页不画「高级」组，「候选窗口主题」一行放进「明暗」组，排在「颜色模式」下面；`settingsThemePreferences` 在这个宿主上把设置窗口的明暗固定为 `follow`，工具栏和屏幕键盘的预览也只看颜色模式。
  - Tauri 面板：`useHostCapabilities` 取宿主能力，`panelSurfaceTheme` 在这个宿主上丢掉面板自己的那一项。面板窗口的客户端不带宿主能力，要异步问外壳；问到之前（`undefined`）也只按颜色模式画，免得 macOS 面板先按已经砍掉的设置闪一下。确定没有宿主（浏览器预览、外壳没答上来）记为 `null`，照旧套用面板自己的那一项。
  - 其他页面的指向：「悬浮工具栏」页的「颜色与明暗」和「屏幕键盘」页的「屏幕键盘外观」两行在这个宿主上改说「跟随「主题」页的颜色模式和主题」，不再指向不存在的工具栏一行或「高级」组；「颜色模式」的说明改成「设置窗口和各界面的明暗模式；候选窗口可在下面单独设置」。
  - 原生宿主：`MetasequoiaFloatingToolbarPanel`、`MSIMEInputModeHUDPanel`、`MSIMEScreenKeyboardPanel`、`MSIMEVoiceWaveOverlay`、`ApplyMetasequoiaMenuTheme`、`MacEmojiAppearance`、`MacHandwritingAppearance` 只读 `theme`。固定明暗的内置主题以前靠改写 `toolbar_theme` / `menu_theme` 钉住工具栏和菜单，现在 `InputController` 钉住的是交给它们的 `theme`，效果相同。
  - 原生回退设置窗口：去掉「悬浮工具栏主题」一行和 `toolbarTheme` 属性，`sharedPreferencesByMerging:` 不再写回 `toolbar_theme`，本机存下的 `MSIMEClientToolbarTheme` 不再读。
- `HostCapabilities::candidate_number_color`（新增）和 `candidate_border_color`（只有 Android、iOS 为真）：设置页不给「候选编号颜色」和「候选边框色」取色器。macOS 宿主把这两个槽位留空交给解析器：选了文字颜色时编号色由它按 `PICKED_NUMBER_ALPHA` 推出，否则取主题的值；边框取主题和皮肤的发丝线。原生回退窗口去掉这两个取色器，合并时不再写它们，`MetasequoiaStoredCustomTheme` 也不再读本机存的旧值。皮肤包里的 `number` 和 `border` 照常生效。设置页的三处预览（候选窗口页预览、主题页的自定义主题卡片、悬浮工具栏预览）读页面模型的 `previewCustomTheme`：`hostDrawnCustomTheme` 按这两个能力位从草稿的自定义主题里去掉宿主不读的槽位，所以 Mac 上存着旧编号色或边框色的文档，预览画的也是宿主实际画的颜色；这份副本只给预览，不写回草稿，文档里的值原样留给其他宿主。

### 所有宿主共享的排版改动

- 主题页的取色器收进「自定义配色」折叠区，默认收起。悬停色留在里面：在 `system` 底上不会推出悬停色（`theme.rs`），深色的自定义表面只能靠它看清悬停。`MoreOptions` 加了可选的 `title`，默认仍叫「更多选项」，只有里面是同一类具体东西时才写出它是什么。
- 「候选窗口」页去掉四个标签，改成一列：预览之后是「布局与字体」组，主流程只有排列方式、每页数量、候选字体和字号；其余各行收进同一个「更多选项」，顺序是展开候选（及展开键）、页码与翻页按钮、显示 logo、跟随光标、英文字体、预编辑字号、整体大小、不透明度、圆角、内边距、候选窗口预编辑、双拼预编辑、行内预编辑。「皮肤、颜色与明暗」跳转行去掉。
- 「候选字体」预设后面加「自定义…」：选它只亮出字体框、不改字体；字体不是任何预设（例如霞鹜文楷）时打开页面就是「自定义…」和字体框，选回预设时字体框收起。HarmonyOS 手机的「更多选项」里不传 `onCustomChange`，那里字体框照旧一直在。
- 「系统字体列表」一行并进字体框：读取状态写在第一个字体框那一行的说明里，刷新按钮在输入框旁边，只要宿主能列字体就一直在。主字体框收起时，「更多选项」里的英文字体框自己带着状态和刷新，所以读取失败后的重试和装了新字体后的刷新都还能找到。
- HarmonyOS 手机的「候选栏」页保持原布局；手机和 HarmonyOS 2in1 用同一列布局，只是触屏宿主没有候选窗口预编辑和行内预编辑两行。

### 保留不动的

刷新皮肤、显示 logo、圆角、预编辑字号、内边距和页码开关、显示展开键都只是收起，不砍：皮肤作者原地换图后只能靠「刷新皮肤」重读；logo 的旧文档默认显示是有记录的决定；圆角和内边距是 #7761、#7316、#7974 的外部用户在调；预编辑字号是五个宿主共用的约定。「双拼预编辑」留在原来的 `PreeditSettingsSection` 里，由输入页那一批移走。`candidate-tabs.ts` 和包入口的 `candidateTabs` 导出保留，共享页不再用它（见 [候选窗口标签模型](2026-10-16-candidate-tabs.md)）。

### 各平台跟进

macOS 之后，Windows、Linux 和 HarmonyOS 照同样的理由跟进：固定明暗的内置主题在这三个宿主上同样会盖过工具栏、菜单和各面板（Windows `server_main.cpp` 用解析出的固定配色，Fcitx5 的角标和语音浮层、鸿蒙的 `KeyboardSession.ets` 用 `GlobalTheme.fixedAppearance`），这几项都不在云同步表里，编号色由文字颜色推出、边框是主题和皮肤的发丝线也是共享解析器（`crates/client-services/src/skin/theme.rs`）的规则，与宿主无关。三个能力位在 `host_surface.rs` 里一处改：`surface_themes_follow` 只在 Android、iOS 为假，`candidate_number_color` 和 `candidate_border_color` 只在 Android、iOS 为真。

- **Windows**：Server 的 `menu_theme`、`toolbar_theme`、`voice_theme` 三个原子量改由全局的 `theme` 算出，三个原子量和用它们的循环原样保留；`CandidateThemeSettings.h` 的 `candidate_theme_values` 带过线程边界时去掉自定义主题里的编号色和边框色，候选窗、工具栏、托盘菜单和语音浮层都从它取值，一处去掉就都不画。表情、手写和屏幕键盘是 Tauri 面板，随能力位经 `panelSurfaceTheme` 跟随。原生 WinUI 设置照共享页改：主题页去掉「设置界面主题」和整个「界面主题」组，「候选窗口主题」挪到「外观」组的「颜色模式」下面（组名不改，免得牵动无关的搜索和截图），取色器从候选窗口页搬回主题页，五个取色器（`ThemeSkinModel.h` 的 `color_slots` 去掉编号色和边框色）收进默认收起的「自定义配色」`Expander`，写法照本页已有的「更多选项」折叠区；候选窗口页去掉四个标签，改成预览、「布局与字体」和一个「更多选项」；「候选字体」加「自定义…」（以前字体不是预设时控件的值是 `custom`，却没有对应的选项）；「系统字体列表」一行并进第一个可见的字体框，WinUI 只有一份字体目录状态，所以没有共享组件「读两次」的问题。搜索落到折叠区里的行时，`record` 在索引时记下的折叠区让它展开，取代原来按标签切换的那张表。工具栏页的「悬浮工具栏主题」原位换成指向主题页的「颜色与明暗」，屏幕键盘页那一行改说跟随颜色模式，两处预览和设置窗口本身只看颜色模式。
- **Windows 刻意不同**：「游戏」组（游戏里候选窗能不能出现、两张程序名单）没有收进「更多选项」，放在折叠区之后照常展开：它是功能开关，不是外观，用户遇到游戏里候选窗不出现时要能直接找到。「双拼预编辑」不在折叠区里：输入页那一批（#8239）先合入，已把它挪到「输入方案」页的双拼详情里，本页跟着不画。
- **Linux**：Fcitx5 的中英文提示角标以前按 `toolbar_theme` 取明暗，而 Linux 设置页从不提供这一行（`theme-settings-section.tsx` 的 `floatingToolbar && !linux`），存下的值看不见却生效；`surface_dark_theme` 改为只看 `theme`，这个隐藏的读取一并去掉。Fcitx5 与 IBus 的语音浮层不再读 `voice_theme`。候选主题的解析请求（`candidate_theme_request`）去掉存下的编号色和边框色，选择指纹仍看整个 `custom_theme`。编号色只有 IBus 画（Fcitx5 经典界面里编号跟随正文），边框只有 Fcitx5 画。共享页随能力位收起「高级」组和两个取色器，没有边框取色器时悬停说明只讲悬停。
- **HarmonyOS**：`KeyboardSession.ets` 不再从偏好读七个单独明暗，交给视图的一律是 `follow`，键盘、工具栏、语音、表情、手写和 2in1 候选右键菜单都跟随颜色模式；`PreparedPreferences` 的字段声明保留，文档照旧解析。解析请求经 `GlobalTheme.hostDrawnCustomTheme` 去掉编号色和边框色（2in1 候选窗按候选配色画编号和边框）。2in1 走桌面式主题页，跟随后和 macOS 一样只剩候选窗口主题；手机的主题页是网格，本来就不给这几行，候选栏画的是键盘调色板，所以手机上看不到变化，但存下的值以前在手机上也照样生效，这次一并停掉。手机的「候选栏」页刻意不动：`HarmonyPhoneCandidateGroups` 按设计保留原来的分组和「更多选项」，触屏没有候选窗口预编辑和行内预编辑；也不给它加「自定义…」，字体框本来就在折叠区里，多一步选择省不下主流程的行数。这里没有触屏独需而桌面没有的设置被砍：手机键盘的明暗本来就只跟随 `theme`。
- **共享首页**：桌面首页「我的键盘 · 当前外观」的键盘预览以前直接读存下的 `screen_keyboard_theme`，macOS 那一批只改了屏幕键盘页的预览，漏了这里；跟随的宿主上真实键盘不读这个值，页面上也没有地方能改它，存着「深色」又用浅色颜色模式的人会在首页看到一张和实际不符的深色键盘。现在页面模型把宿主真正会用的值（跟随时不传）交给 `HomePage`，与屏幕键盘页的 `keyboardPreviewTheme` 同一条规则。
- **没有宿主再画的分支**：翻转之后，`ThemeSettingsSection` 里的工具栏、菜单和语音三行只在 Android、iOS 上才可能出现，而这两个宿主不满足这几行的显示条件（没有悬浮工具栏和桌面菜单），实际已没有宿主会画；悬浮工具栏页和屏幕键盘页里不跟随时的那套措辞同样只在测试宿主上还会走到。按「不删看起来没用的代码」的规矩保留，测试用显式 `surface_themes_follow: false` 的宿主继续钉住它们。

## Alternatives considered

- **升级时一次性把存下的值改回 `follow` / `null`** — 研究报告按 [去掉数字键盘设置](2026-10-11-remove-number-keypad-settings.md) 的先例建议这样做，最强的理由是文档里不会留着一个看不见的值。没有采用：负责人定了这一批不做数据迁移；桌面宿主不再读这些值，效果与重置相同，而 Android 和 iOS 的键盘仍读屏幕键盘、表情、手写的单独明暗和编号色、边框色，文档里的值要原样留给它们。
- **在 `InputController` 把这几项从字典里删掉再交给面板，面板代码不动** — 原生改动最少。没有采用：面板仍然会读这些键，以后从别的入口把字典交给面板就又生效了；`preference_coverage.py` 按源码里有没有这个名字判断宿主读不读，删字典的写法会让六个已经不读的偏好看起来仍被读取。
- **保留各界面自己的明暗** — 最强的理由是：跟随系统、浅色系统下想要一条深色工具栏或深色菜单的人，以后只能换「夜」「墨」这类主题，而它们会把候选窗口的配色一起换掉；设置窗口、表情面板、手写板和语音弹出条各自单独定明暗也有人用。没有采用：固定明暗的内置主题本来就会盖过工具栏和菜单的设置，这几项只在跟随系统时生效；这些界面一年开不了几次，单独定明暗没有记录在案的需求。
- **只在 macOS 收口，Windows、Linux、HarmonyOS 保留各界面的单独明暗和两个取色器** — 最强的理由是参考产品在 Windows 上就有这些项（`appearance.theme_ftb`、`theme_menu` 一类），Windows 原生设置页一直照参考产品提供它们，从参考产品迁来的用户会找。没有采用：同一份共享文档在不同桌面上能改的项不一样，用户在一台机器上看不见的值换到另一台机器又生效；Linux 的角标和鸿蒙手机已经在读页面从不提供的值；macOS 的理由（固定明暗的主题本来就盖过这几项、单独定明暗没有记录在案的需求）在这三个宿主上同样成立。参考产品的叫法和位置由 `docs/windows-parity.md` 记着，这次的订正也写在那里。
- **连「候选窗口主题」一起砍** — 核查推翻：默认全局主题就是跟随系统（`theme.rs`），这一项在默认安装下真的生效，「夜」「墨」不是等价替代。它是剩下唯一的一项。
- **把「候选窗口主题」挪进「自定义主题」组** — 负责人提的方案，同样能让「高级」组消失。改放进「明暗」组：它是明暗选择，不用自定义主题也生效，放在自定义主题下面会读成只对自定义主题有效。
- **保留编号色和边框色取色器** — 最强的理由是有人照搜狗皮肤单独设编号色；边框在浅色系统、深色自定义表面上几乎看不见。没有采用：编号色由文字颜色推出，边框是 1pt 发丝线且原生默认半透明，能适应任何底色；需要特别编号色或边框的人可以用皮肤包，`skin.toml` 照常带这两个槽位。
- **把悬停色也砍掉** — 核查推翻，理由见 Decision。
- **折叠区叫「高级」，配色折叠区叫「自定义配色…」** — 研究报告和负责人的叫法。`MoreOptions` 在 develop 上的约定是不另起「高级」这类笼统的名字（负责人的决定里也写的是「高级/更多选项」），所以候选窗口页用默认的「更多选项」，主题页用写出内容的「自定义配色」；不带省略号，因为它是原地展开的折叠区，省略号按惯例表示会打开新窗口或对话框。
- **把候选窗口预编辑和行内预编辑合成一个「编码显示位置」** — 核查推翻：两项组合出的几种设置对应不同的迁移习惯，`tsf_preedit_style` 还是 Windows TSF 的键，合并要改所有宿主。两行都进「更多选项」。
- **把字体读取状态和刷新放进字体下拉菜单的底部** — 更贴近「并进字体框」的字面意思。没有采用：下拉菜单在输入框失焦时关闭，键盘用户 Tab 不到里面的按钮；状态写在行说明里、刷新按钮放在输入框旁边，键盘和读屏都能到。
- **保留四个标签，只在标签里加折叠区** — 改动更小。没有采用：主流程只剩四行之后，标签只会把一列内容切成几屏。

桌面主题与本地皮肤的同页混排由[本地皮肤顶部标签](../feature/2026-10-18-local-skin-top-tab.md)部分取代；本文的能力过滤、颜色槽位和配色折叠决定继续适用。

## Consequences

- **收益**：macOS 主题页从颜色模式、主题轮播、外部皮肤、七个取色器加八个下拉框，减到颜色模式加候选窗口主题、主题轮播、外部皮肤和一个收起的「自定义配色」；候选窗口页的主流程只有四行，其余在一个折叠区里，不用在标签之间找设置。
- **代价**：在 Mac、Windows、Linux 或 HarmonyOS 上定过工具栏、菜单或某个面板单独明暗的人，升级后这些界面改为跟随颜色模式，页面上不再有能改回去的地方；设过编号色或边框色的人同样回到主题的颜色。Linux 上选过边框色的人改用主题或皮肤的发丝线，`system` 底上则是原生的发丝线：边框槽位留空时 Fcitx5 取原生 token 的 1px 描边（`CandidateColors.h`），没有人因此失去描边，变的只是颜色。
- **已知上限**：主字体框和英文字体框各有一份字体目录状态，两个框都点开时会各读一次系统字体列表。
- 共享页的标签模型 `candidateTabs` 不再被页面使用；以后要删它，先确认包入口的外部使用者。

## Verification

- `cargo test -p msime-client-core --lib host_surface` 和 `cargo test -p msime-host-api --lib host_capability_boundary` 钉住三个能力位在各宿主上的取值。
- `apps/desktop/tests/settings/candidate-settings-layout.test.tsx` 钉住候选窗口页的一列布局、折叠区里的行和顺序、「自定义…」的出现与收起；`skin-custom-colors.test.tsx` 钉住「自定义配色」折叠区和 macOS 上少掉的两个取色器；`surface-themes.test.tsx` 钉住跟随的桌面宿主只剩候选窗口主题、显式不跟随的宿主照旧给出各界面的明暗；`font-catalog.test.tsx` 钉住字体框里的状态和刷新；`desktop-keyboard.test.tsx` 和 `settings-theme-preferences.test.ts` 钉住 Tauri 面板和设置窗口在 macOS 上跟随颜色模式；`macos-keyboard.test.tsx` 钉住宿主能力晚到时跟随的面板从不画存下的屏幕键盘明暗、显式不跟随的宿主在答复后照旧套用；`skin-custom-colors.test.tsx` 还钉住桌面宿主上存下的编号色和边框色不进预览、仍读这两个槽位的宿主照旧画；`floating-toolbar-page.test.tsx` 和屏幕键盘页测试钉住两条指向行在 macOS 上的说明。
- 原生面板测试（`FloatingToolbarPanelTest.mm`、`InputModeHUDPanelTest.mm`、`ScreenKeyboardPanelTest.mm`、`VoiceWaveOverlayTest.mm`、`InputMenuTests.mm`、`ShortcutTest.mm`、`EmojiAppearanceTest.swift`、`HandwritingProviderTest.swift`）改为断言单独设置被忽略；`SkinPreviewTest.mm` 断言编号色由文字颜色推出、边框不取文档里的值、合并时原样带过文档的编号色。这些只在 CI 的 macOS 构建里编译运行，本地没有编译原生宿主。
- `platforms/macos/tests/settings/preference_coverage.py` 把六个不再读的偏好记进 `NOT_APPLICABLE`；`scripts/test-reference-config-coverage.py` 不变，Android 和 iOS 仍在共享页上画这些字段。
- 各平台跟进：`host_surface/tests.rs` 和 `host-api` 的边界测试钉住四个桌面宿主跟随、Android 和 iOS 照旧；`surface-themes.test.tsx`、`skin-custom-colors.test.tsx`、`macos-keyboard.test.tsx`、`desktop-keyboard.test.tsx`、`settings-theme-preferences.test.ts`、`host-capabilities.test.tsx`、`mobile-app-chrome.test.tsx`（2in1）和 `settings.test.tsx` 的参考小节表（`followedSurfaceThemeSections`）按各宿主的真实取值断言，另用显式不跟随的宿主钉住保留的分支。Windows 的 `tests/ui/candidate_theme_reload.cpp`（Server 不带编号色和边框色）、`theme_skin_model.cpp`（五个取色器）、`candidate_font_presets.cpp`（主字体框何时出现）和 `tests/tools/settings_ui.ps1`（一列布局、折叠区展开、搜索打开折叠区）；Linux 的 `candidate_palette.cpp`、`fcitx5/tests/native.cpp`（角标和语音浮层忽略存下的值）和 `runtime/engine_smoke.cpp`（IBus 编号色由文字颜色推出）；`apps/desktop/tests/core/home-page.test.tsx` 钉住桌面首页的键盘预览在跟随的宿主上照颜色模式画、显式不跟随的宿主照旧用存下的值；HarmonyOS 的 `platforms/harmony/tests/keyboard-logic.test.ts` 和新的 `scripts/test-harmony-surface-themes-follow.py`（`KeyboardSession.ets` 不再从偏好读七个单独明暗、解析请求去掉两个槽位）。Windows、Linux 和 HarmonyOS 的原生代码本地编译不了，CI 是第一个编译它们的地方。
