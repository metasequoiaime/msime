# Agent Note: 精简「输入」「快捷键」两页：macOS 固定三项，其余收起、合并或按条件出现

Status: implemented

## Problem

macOS 的「输入」页默认状态（full 版、全拼、模糊音关）有约 56 个可见控件，「快捷键」页大半是只读的按键说明。多数行不是没用，而是平铺：少有人改的项和主流程放在同一层，一个决定被拆成几行，还有几行根本改不了什么：

- 日语、韩语、粤拼、注音、笔画、藏文各画一个只有一项的分段控件，`onChange={() => {}}`。
- 双拼方案在全拼、五笔下灰着显示。
- 以词定字是开关加两值分段，表达的是三种状态。
- 调频的触发频次在调频关闭时也显示，步长只有线性调频读却一直显示。
- 辅助码两族六行全部平铺，全拼用户也看到三行双拼的设置。
- 13 个快捷模式开关平铺在总开关下面；临时日语词库的下载在新装时单占一组。

另外有三项在 macOS 上关掉没有换来什么，反而让事情变坏：

- `navigation.tab`：候选窗显示时 `InputController.mm` 对关掉的 Tab 也返回已处理，Tab 什么也不做，并不会还给应用。
- `navigation.page_up_down`：同样，候选窗显示时关掉的 PageUp/PageDown 被吞掉。候选窗没显示时（组字但没有候选，或光标位置无效），关掉的 PageUp/PageDown 才交给应用，并先结束失焦上屏的组字。
- `mixed_input.minimum_prefix`：默认 5 有数据依据，调低后有 298 个常见简拼被英文词抢走首选（[英文领先的混输](../feature/2026-10-12-mixed-english-leads-unsegmentable-input.md)）。

还有两处默认值和 Rust 不一致：emoji 混输的 TS 回退值是关，而 macOS 和 Windows 上 Rust 默认开；「Control+Shift+F 切换繁体输出」缺键时页面按关显示，Rust 默认开。

## Decision

语言与中文方案的选择控件由[输入语言与中文方案分层选择](../feature/2026-10-19-input-language-scheme-hierarchy.md)补充；本篇关于单方案说明和按当前方案显示选项的约束继续保留。

### macOS 固定三项（按宿主能力位）

- `HostCapabilities` 新增 `tab_page_paging_switches` 和 `mixed_input_minimum_prefix`，macOS 为假。其余宿主上的取值见下面的「各平台跟进」：`tab_page_paging_switches` 在 Windows 和 HarmonyOS 上也为假，Linux、Android、iOS 为真；`mixed_input_minimum_prefix` 在 Windows 和 Linux 上也为假，三个触屏宿主为真。TS 类型里两项可选，缺省按真处理（早于它们的宿主照旧显示）。
- 宿主不再读存着的值：
  - macOS 的 `-[MSIMEAppearancePreferences navigationEnabled:]` 对 `tab`、`page_up_down` 一律返回 YES。
  - host-api 的 `host_english_minimum_prefix` 按 `HostPlatform::reads_mixed_input_minimum_prefix` 判断，macOS 上固定用 `DEFAULT_MIXED_INPUT_MINIMUM_PREFIX`（5）。
  - 不迁移数据，文档里的值原样留着。三项都不在账号同步表里（`settings_sync.rs` 没有 `navigation` 和 `minimum_prefix`），所以不会出现别的设备同步来的值在 Mac 上看不见却生效。
- 共享页按能力位不再画这三个控件。翻页方式下面多一句「Shift+Tab / Tab 和 PageUp / PageDown 在候选窗口显示时始终翻页」。快捷键页的速查经 `effectiveNavigation` 列出实际生效的翻页键，不看文档里存的关。
- 原生回退窗口跟着删：
  - `NavigationControls()` 不再列 Tab 和 PageUp 两个复选框。
  - 删掉「中英混输触发长度」弹出框和 `mixedEnglishMinimumPrefix` 属性。
  - `sharedPreferencesByMerging:` 不再写 `navigation.tab`、`navigation.page_up_down`，也不从本机写 `mixed_input.minimum_prefix`。`MSIMEMergePreferenceSnapshot` 按键合并，文档原有的值保持不动。但 `mixed_input` 在这个方法里是整个对象替换的，而 client-core 的 `minimum_prefix` 是必填字段，所以传入文档里原有的 `minimum_prefix` 要原样带过去；第一版漏了这一步，合并出的完整文档读不回来，CI 上 `local-mode-preferences`、`shortcut`、`shortcut-translations` 三个测试因此失败。
  - `applyNavigationPreset:` 不再写 `page_up_down`。
- `scripts/test-reference-config-coverage.py` 不改。三个字段在别的宿主的共享页上照旧可改（各平台跟进之后，两组翻页键在 Linux、Android、iOS 上可改，触发字符数在 Android、iOS、HarmonyOS 上可改），不属于 `PLATFORM_LOCAL`（那里只收在本仓库根本没有对应设置对象的项）；记进去只会让「共享页上有控件」的检查对它们失效。

### 各平台跟进（Windows、Linux、HarmonyOS）

macOS 这一轮之后，三个平台逐项核对：控件在这个平台上存在、macOS 的理由在这里也成立，才跟进；理由不成立的保留，并写明原因。

**Windows**

- Tab / PageUp 固定开启。理由在 Windows 上更强：候选窗显示时 TIP 总把 Tab、PageUp、PageDown 交给 Server（`CompositionProcessorEngine.cpp` 的 `FUNCTION_SERVER_CANDIDATE_KEY`），组字时也一样；Server 的 `navigation_action`（`NavigationPolicy.h`）对关掉的绑定只回答 `NavigationReply::Ignored`，`ServerSession::navigate` 只在有组字时运行。开关从来不会把键还给应用，只会让它失效。改法：
  - `preference_navigation` 解码共享偏好后把 `tab`、`page_up_down` 置为开。开发者预览显式给出的 `key_bindings`（`PreviewConfig.h`）不经过这里，不动。
  - WinUI「选词与翻页」不再列这两个复选框，「翻页方式」的说明用共享页的同一句话。
  - `navigation_policy.cpp` 断言存着关时两组仍开，`preference_monitor.cpp` 改为期望两组为开。
- 触发字符数固定为 5：理由来自 Engine，与平台无关；host-api 是唯一的读取方。WinUI 去掉「触发字符数」滑块。
- 双拼预编辑从「候选窗口 › 预编辑」移到「输入方案 › 当前方案选项」，只在双拼下出现。原来它对所有方案显示，说明却写着只在双拼下生效，与 macOS 是同一个缺陷。
- 以词定字：WinUI 本来就是一个「关闭 / [ / ] / - / =」下拉，「关闭」也不动记着的键组。但选键组时会悄悄关掉同组翻页键（`doc.SetBoolean(L"navigation." + value, false)`），正是下面 Alternatives considered 里否决的「以词定字选键时把同组翻页键抢过来」。现在已用于翻页的键组不可选（`Option.enabled`，当前选中的除外），改翻页勾选时重画页面以刷新可选状态。
- 调频：学习选词习惯关闭时整组不画；调频关闭时不画触发频次；步长只在线性调频下出现。学习开关和调频方式改为改动后重画。
- 「更多选项」折叠：
  - 符号混输、颜文字混输收进「候选与联想」组末；全角输入收进「输出」。
  - 各快捷模式的勾选收进新的「逐个模式」组，总开关的说明换成共享页那句。
  - 折叠用 WinUI 已有的 `Expander` 写法（本地识别模型、AI 辅助两处），抽成 `add_more_options`，标题固定为「更多选项」；搜索落到折叠里的某一行时自动展开。
- emoji 混输的字面回退值改为开，与 Windows 的 `source_mixed_emoji_default` 一致。平时由出厂偏好兜底，只在读不到出厂偏好时起作用。
- 已经一致、无需改动的：
  - 六种单方案语言：WinUI 本来没画假控件。
  - 双拼各行本来只在双拼下出现；Ctrl+Shift+F 缺键本来按开。
  - 快捷键页：WinUI 的「按键」页本来只有真控件，没有只读速查行。页面数不变，`SettingsNavigation.h` 不动。
- 有意不跟进的：
  - 辅助码另一族：WinUI 只画当前方案那一族，比共享页更紧，再加一个折叠的另一族是加行而不是减行。
  - 临时日语词库的下载行：WinUI 没有这一行。
- 已知缺口已单独修复：WinUI「输入习惯」补上日文混输、只出单字、本地整句联想三行，详见[Windows 输入习惯页补齐共享候选偏好](../bug-fix/2026-10-17-windows-input-habits-parity.md)。

**Linux**

- 触发字符数固定为 5，理由同上。共享页按能力位自动藏起这一行，`platforms/linux` 下没有读取方，不用改宿主。
- Tab / PageUp 保留，因为理由在 Linux 上不成立：
  - fcitx5 对关掉的绑定跳出分派，落到 `if (composing) command(MSIME_FINISH_COMPOSITION); return false;`，结束组字并把 Tab、PageUp 还给应用（`FcitxEngine.cpp`）。
  - IBus 的 `NavigationBindings::command` 同样返回空，交还按键。
  - 开关在这里有实际作用，所以 `tab_page_paging_switches` 仍为真，`navigation_bindings.cpp` 不改。
- 其余布局改动都在共享页里，#8217 已经覆盖 Linux。

**HarmonyOS**

- Tab / PageUp 固定开启。组字时 `HardwareKeyRouter` 把关掉的 Tab、PageUp 判为 `HardwareKeyAction.IGNORED`，`HardwareKeyDispatch` 消费掉这个键，与 macOS 是同一个陷阱。只有韩文汉字列表和注音选字列表（都走 `routeHanjaList`）把关掉的键交还应用，固定开启后在列表里也翻页，这与 macOS「没有候选窗时」的代价相当。改法：
  - `KeyboardSession.ets` 固定 `tab: true`、`pageUpDown: true`，不再读存值；`scripts/test-harmony-fixed-paging-keys.py` 钉住这条接线和能力位。
  - 手机「外接键盘快捷键」页的 `NavigationSection` 原来没接能力位，现在传 `tabPageSwitches`，说明里写「候选栏」。网格从六项变成四项，偏离 [iOS 与鸿蒙全平台设计](../feature/2026-10-07-ios-harmony-all-platform-design.md) 里的六项网格。
  - 二合一的输入页本来就传这个能力位；速查经 `effectiveNavigation` 自动把 Tab、PgUp 列为生效。
- 触发字符数保留：
  - 鸿蒙是触屏宿主，九键、十四键上触发字符数数的是按键次数；Android 也是应用户要求（#7081）才加的这一项。
  - 在玻璃上多打几个字母代价更大，「让英文更早出现」的理由更站得住。
  - 所以鸿蒙与 Android、iOS 一样继续读；手机布局已经把它收在「更多」里。
- 双拼预编辑留在候选窗口/候选栏页：每种鸿蒙形态都有触屏方案，方案组不画双拼各行（上面的二合一修正已覆盖）。
- 快捷模式总开关在鸿蒙上不存在（`shift_letter_entry_switch` 为假），保持平铺。

**共同的同步前提**：`settings_sync.rs` 目前不同步 `navigation.*` 和 `mixed_input.minimum_prefix`。哪天开始同步，所有固定它们的宿主都要改为按条件显示（翻页键是 macOS、Windows、HarmonyOS，触发字符数是三个桌面宿主），否则别的设备同步来的值会在这些宿主上看不见却生效。

### 共享页的布局改动（所有宿主）

其中桌面快捷键页的页尾折叠入口已由[按键速查顶部标签](../feature/2026-10-19-shortcuts-reference-top-tabs.md)取代；手机仍保留本篇的折叠布局。

- **只有一个方案的语言**：日语、韩语、粤拼、注音、笔画、藏文改成没有控件的说明行（「罗马音：直接输入罗马音……」），仍按当前方案隐藏而不删除。
- **双拼的各行**：双拼方案、编辑自定义键位、键位提示，以及从「候选窗口」页移来的「双拼预编辑」，只在双拼方案下出现。
  - 双拼预编辑只在没有触屏方案的宿主上移到方案组（`showShuangpinPreedit && !client.touchKeyboardSchemes`）。有触屏方案的宿主，方案组本来就不画双拼各行（`hasTouchKeyboardSchemes`），所以 iOS、Android、鸿蒙手机和鸿蒙二合一仍在候选窗口/候选栏页设置它。两页用同一个条件，保证这一行恰好出现在一个页面上；鸿蒙二合一不是手机布局（`mobile_settings` 为假）却有触屏方案，按 `mobilePlatform` 判断会让它两页都找不到这一行。
  - `InputSchemeDetailsSection` 的 `macos` 属性随之删掉。
- **以词定字**：实体键盘宿主上合成一个「关闭 / [ / ] / - / =」分段选择（`wordCharacterChoiceChange`）。
  - 选「关闭」只把 `enabled` 置假，记着的键组不动。
  - 被翻页占用的键组不能选（当前选中的除外），与原生回退窗口的互斥一致。
  - iOS 保留长按开关。
- **调频**：学习选词习惯关闭时整组不画；方式是「关闭」时不画触发频次；不是「线性调频」时不画步长。三项会上云，所以只按条件显示，不固定。这改掉了[拼音调频偏好独立于设置控件](2026-10-15-frequency-preferences.md)里「组件保持可编辑状态、不受学习开关限制」的说法，那篇的纯模块归属仍然有效，已在那里注明。
- **辅助码**：桌面传 `activeFamily`，显示当前方案那一族的三行，另一族三行收进「更多选项」。鸿蒙手机保持原来的紧凑形式（只显示一行码表），iOS 和 Android 照旧两族平铺。
- **快捷模式**：有「Shift+字母进入快捷模式」总开关的桌面宿主上，各模式开关收进紧跟其后的「逐个模式」组的「更多选项」。临时日语词库的下载行接在 R 模式那一行后面，不再单占一组。触屏宿主没有总开关，照旧平铺。
- **候选与联想**：
  - 符号混输、颜文字混输、日文混输、只出单字、本地整句联想收进组末的「更多选项」。为此拆出 `MixedInputMoreRows` 和 `SentenceWordLatticeRow`。
  - 全角输入（起始状态）在共享页的桌面布局里直接显示于「输出」组末尾：只有这一行时，折叠本身多占一行、还多一次点击。Windows 原生页的折叠不受这个共享组件改动影响。
  - 鸿蒙手机已经把整页收在一个「更多选项」里，那里不再套第二层，这些行也留在原来的位置上：混输各行和只出单字在学习开关之前，全角输入在「输出」组最前，鸿蒙手机的顺序与改动前一致（唯一的差别是符号混输排到 emoji 混输之后）。
- **快捷键页**：候选参考行和「打开屏幕键盘」并进一个「按键速查」组，内容收在「更多选项」里，与鸿蒙手机的做法一致。「候选操作」组只在有数字键选词开关时出现。鸿蒙手机的布局不变。
- **文案**：菜单栏中/英状态的说明缩成一句。

折叠区一律用现成的 `MoreOptions`，标题固定为「更多选项」（它的注释规定不另起「高级」之类的叫法）。需要一个可见名字的地方（「逐个模式」「按键速查」），照鸿蒙手机「按键速查」的先例，用带标题的 `GroupList` 包住 `MoreOptions`。

### 默认值修正

- `settingsInputPreferences(draft, platform)` 在文档缺 `mixed_input` 时用 `mixedInputDefaultsFor(platform)`：macOS 和 Windows 上 emoji 混输开，与 `source_mixed_emoji_default` 一致。`defaultMixedInput` 本身不变。
- 「Control+Shift+F 切换繁体输出」缺键时按 `defaultKeybindings` 显示为开。

### 保留不动

- 输入模式和输入方案保留两行。
- 上 / 下移动候选项保留：它还决定 ←/→ 是移动候选还是移动光标。
- 学习选词习惯保留为独立开关：关掉它还会停掉新词和整句学习，而且这个值上云。
- `/` 和 `@` 模式留在输入页（折进「逐个模式」）。
- 五笔查码、`ime_mode_scope`、Control+Shift+F、Option+Shift+H 保留。
- 中英文切换提示和菜单栏中/英状态保留为两个开关。

### `showDesktopMaintenanceShortcuts`

`settings-capabilities.ts` 仍在计算它，但没有消费方。消费方是 `252562694a`（feat(ui): drop the maintenance shortcuts section from the shortcuts page）有意删掉的：那次提交删除了 `MaintenanceShortcutsSection` 及其测试，并把断言改成快捷键页不再出现维护组合键。所以不重新接上，也不在这次删除这个计算，只记录在这里。

## Alternatives considered

- **只藏起 Tab / PageUp 和触发字符数的控件，宿主继续读存着的值**：改动最小，改过的人保留原行为。没有采用，是因为改过的人从此改不回来，与[撤掉数字键面设置](2026-10-11-remove-number-keypad-settings.md)否决的做法相同。三项都不上云，固定下来不会和别的设备冲突。
- **保留 Tab / PageUp 开关的最强理由**：有人可能希望候选窗显示时 Tab 交还应用（例如在表格里跳格）。但 macOS 上候选窗显示时关掉它并不交还，只是让这个键失效。候选窗没显示时，关掉的 PageUp/PageDown 确实交给应用，这是保留这个开关唯一实际的作用；它只在组字却没有候选窗的少数时刻起作用（韩文音节、注音组字，或光标位置无效），不值得为它保留一个控件。真要交还，需要先改 `InputController.mm` 的按键路由，那是另一项功能，不是保留开关的理由。
- **保留触发字符数的最强理由**：打英文多的人想让短词更早出现。默认值 5 是用数据定的，调低的代价落在多数人身上；不想要英文候选的人有「中英混输」开关。触屏宿主（Android、iOS、HarmonyOS）的页面仍然给出这一项，理由见「各平台跟进」。
- **把触发频次、步长、调频方式一起固定或并进学习开关**：它们会上云，iOS 和 Windows 原生设置也能改，只在 macOS 固定会让同步来的值看不见却生效。`learning=false` 还会停掉新词和整句学习，并成一个选择要六个取值。所以只按条件显示。
- **以词定字选键时把同组翻页键抢过来**（旧开关打开时的做法）：不会出现选不了的情况。但这会悄悄改掉另一项设置；禁用被占用的键组，和原生窗口、旧分段控件的互斥一致。只有在用户把两组都设成翻页键时，才需要先取消一组。
- **给 `MoreOptions` 加标题参数，直接叫「高级」或「逐个模式」**：最贴近报告的叫法。但组件注释明确规定折叠区名字固定为「更多选项」、不另起「高级」，各平台布局都依赖这个统一叫法。需要名字的地方用组标题表达。
- **桌面辅助码直接用鸿蒙手机的紧凑形式**（只显示一行码表，启用开关也收起来）：现成、零改动。但启用开关收起后，可见的码表行在辅助码关闭时是灰的，看不出怎么打开。手机的设计稿接受这一点；桌面更适合把当前族的三行都摆出来。
- **把双拼预编辑在所有宿主上都移走**：最整齐。但触屏键盘的方案组不画双拼各行（用触屏方案列表代替），移走后 iOS、Android 和鸿蒙就找不到它了，所以只在没有触屏方案的宿主上移。
- **按 `mobilePlatform` 决定双拼预编辑放哪一页**：初版这样做。鸿蒙二合一的 `mobile_settings` 为假，却有触屏方案，结果候选窗口页不画、方案组也不画，两页都找不到。改为两页都看 `client.touchKeyboardSchemes`。

## Consequences

[可选罗马字布局](../feature/2026-10-23-korean-romaja-layout.md)提供第二个实际布局后，韩语说明行在支持的桌面宿主上变为布局选择控件。

- **收益**：
  - macOS 默认的全拼视图里，「输入」页少了灰着的双拼方案、Tab / PageUp 两个复选框、触发字符数、调频步长、双拼辅助码三行、11 个逐个模式开关、临时日语词库组，以及候选与联想里的五行。
  - 「快捷键」页只剩输入模式切换一组，加一个收起的「按键速查」。
  - 六种只有一个方案的语言不再画假控件。
- **代价**：
  - macOS 上改过 Tab / PageUp 翻页或触发字符数的人，会回到固定值且无处可改。
  - 把触发字符数改成别的值的人，英文候选出现的时机会变。
  - 关掉过 PageUp/PageDown 的人，在组字而候选窗没显示时按这两个键，原本直接交给应用（先结束失焦上屏的组字），现在先作为翻页命令交给会话，会话不处理才交给应用。
  - 共享页上，所有宿主的「以词定字」都换成三选一。
  - Windows 上关掉过 Tab 或 PageUp/PageDown 的人，组字时按它们会翻页。原来这个键什么也不做，也从不交给应用。
  - Windows 上存着较小触发字符数的人回到 5，英文候选出现得晚一些。较早的 Windows 文档里可能存着 2（共享默认曾钉在 2，见 `docs/macos-parity.md`），这是有意的默认值，但看得出变化。Linux 同理。
  - 鸿蒙实体键盘上关掉过 Tab 或 PageUp/PageDown 的人，组字时按它们会翻页；手机「外接键盘翻页键」网格从六项变成四项。
- 文档里留着的 `navigation.tab=false` 之类的值不清理。macOS、Windows 和 HarmonyOS 不读翻页键的这两项，三个桌面宿主不读触发字符数，其他宿主照旧读。
- **需要重访的信号**：有人要求 Tab 在候选窗显示时交还应用；或者账号同步开始收录 `navigation` 或 `mixed_input`，那时这三项在各个固定它们的宿主上都需要改成按条件显示。

## Verification

- `cargo test -p msime-client-core host_surface` 的 `hosts_fix_the_tab_page_paging_keys_and_the_mixed_input_prefix` 逐个宿主钉住两个能力位。
- `cargo test -p msime-host-api` 的 `english_minimum_prefix_is_fixed_where_the_host_does_not_read_it`（三个桌面宿主拿 5），以及 `mixed_input_changes_defer_until_composition_ends` 在桌面构建上读到 5、触屏构建上照文档，钉住宿主不读存值。
- `pnpm --filter @msime/desktop test` 覆盖：
  - 方案说明行、双拼各行与双拼预编辑只在双拼下出现、鸿蒙二合一的双拼预编辑恰好出现在一个页面上、以词定字三选一。
  - 调频按条件显示、各组的「更多选项」、逐个模式与日文词库行。
  - 桌面辅助码布局、按键速查。
  - macOS 和 Windows 无 Tab / PageUp 复选框且速查仍列出它们，Linux 照旧列出；macOS、Windows、Linux 无触发字符数。
  - 鸿蒙手机「外接键盘翻页键」只剩四项并写「候选栏」，点「↑ / ↓」写回偏好（`candidate-shortcuts-harmony.test.tsx`）。
  - 缺键时的 emoji 混输与 Control+Shift+F 默认值。
- 原生 macOS 的改动只能由 CI 编译：
  - `SkinPreviewTest.mm` 断言窗口里没有两个翻页复选框、存着的关不起作用、合并结果不写回。
  - `ShortcutTest.mm` 断言存着关时 Tab、Shift+Tab、PageUp、PageDown 照样翻页；混输合并进空字典时不带 `minimum_prefix`，合并进带着它的文档时原样保留。
- Windows 和鸿蒙的原生改动也只能由 CI 编译：
  - `platforms/windows/tests/input/navigation_policy.cpp` 断言存着关时 Tab、PageUp 仍翻页，`preference_monitor.cpp` 期望发布后两组为开。WinUI `main.cpp` 没有测试框架，只有 CI 构建。
  - `scripts/test-harmony-fixed-paging-keys.py` 钉住 `KeyboardSession.ets` 不读存值、能力位对鸿蒙为假。
- `scripts/test-reference-config-coverage.py` 和 `platforms/macos/tests/settings/preference_coverage.py` 通过。
