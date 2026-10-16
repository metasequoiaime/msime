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

### macOS 固定三项（按宿主能力位）

- `HostCapabilities` 新增 `tab_page_paging_switches` 和 `mixed_input_minimum_prefix`，macOS 为假，其余宿主为真。TS 类型里两项可选，缺省按真处理（早于它们的宿主照旧显示）。
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
- `scripts/test-reference-config-coverage.py` 不改。三个字段在 Windows、Linux 和移动端的共享页上照旧可改，不属于 `PLATFORM_LOCAL`（那里只收在本仓库根本没有对应设置对象的项）；记进去只会让「共享页上有控件」的检查对它们失效。

### 共享页的布局改动（所有宿主）

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
  - 全角输入（起始状态）收进「输出」组的「更多选项」。
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
- **保留触发字符数的最强理由**：打英文多的人想让短词更早出现。默认值 5 是用数据定的，调低的代价落在多数人身上；不想要英文候选的人有「中英混输」开关。Windows、Linux 和移动端的页面仍然给出这一项。
- **把触发频次、步长、调频方式一起固定或并进学习开关**：它们会上云，iOS 和 Windows 原生设置也能改，只在 macOS 固定会让同步来的值看不见却生效。`learning=false` 还会停掉新词和整句学习，并成一个选择要六个取值。所以只按条件显示。
- **以词定字选键时把同组翻页键抢过来**（旧开关打开时的做法）：不会出现选不了的情况。但这会悄悄改掉另一项设置；禁用被占用的键组，和原生窗口、旧分段控件的互斥一致。只有在用户把两组都设成翻页键时，才需要先取消一组。
- **给 `MoreOptions` 加标题参数，直接叫「高级」或「逐个模式」**：最贴近报告的叫法。但组件注释明确规定折叠区名字固定为「更多选项」、不另起「高级」，各平台布局都依赖这个统一叫法。需要名字的地方用组标题表达。
- **桌面辅助码直接用鸿蒙手机的紧凑形式**（只显示一行码表，启用开关也收起来）：现成、零改动。但启用开关收起后，可见的码表行在辅助码关闭时是灰的，看不出怎么打开。手机的设计稿接受这一点；桌面更适合把当前族的三行都摆出来。
- **把双拼预编辑在所有宿主上都移走**：最整齐。但触屏键盘的方案组不画双拼各行（用触屏方案列表代替），移走后 iOS、Android 和鸿蒙就找不到它了，所以只在没有触屏方案的宿主上移。
- **按 `mobilePlatform` 决定双拼预编辑放哪一页**：初版这样做。鸿蒙二合一的 `mobile_settings` 为假，却有触屏方案，结果候选窗口页不画、方案组也不画，两页都找不到。改为两页都看 `client.touchKeyboardSchemes`。

## Consequences

- **收益**：
  - macOS 默认的全拼视图里，「输入」页少了灰着的双拼方案、Tab / PageUp 两个复选框、触发字符数、调频步长、双拼辅助码三行、11 个逐个模式开关、临时日语词库组，以及候选与联想里的五行和全角输入。
  - 「快捷键」页只剩输入模式切换一组，加一个收起的「按键速查」。
  - 六种只有一个方案的语言不再画假控件。
- **代价**：
  - macOS 上改过 Tab / PageUp 翻页或触发字符数的人，会回到固定值且无处可改。
  - 把触发字符数改成别的值的人，英文候选出现的时机会变。
  - 关掉过 PageUp/PageDown 的人，在组字而候选窗没显示时按这两个键，原本直接交给应用（先结束失焦上屏的组字），现在先作为翻页命令交给会话，会话不处理才交给应用。
  - 共享页上，所有宿主的「以词定字」都换成三选一，Windows 和 Linux 的「触发字符数」「Tab」行不变。
  - Windows 原生 WinUI 页仍有这些控件，与共享页的布局暂时不一致，属于后续工作。
- 文档里留着的 `navigation.tab=false` 之类的值不清理。macOS 不读它们，其他宿主照旧读。
- **需要重访的信号**：有人要求 Tab 在候选窗显示时交还应用；或者账号同步开始收录 `navigation` 或 `mixed_input`，那时这三项在 macOS 上的固定需要改成按条件显示。

## Verification

- `cargo test -p msime-client-core host_surface` 的 `macos_fixes_the_tab_page_paging_keys_and_the_mixed_input_prefix` 钉住能力位。
- `cargo test -p msime-host-api` 的 `english_minimum_prefix_is_fixed_where_the_host_does_not_read_it`，以及 `mixed_input_changes_defer_until_composition_ends` 在 macOS 上读到 5，钉住宿主不读存值。
- `pnpm --filter @msime/desktop test` 覆盖：
  - 方案说明行、双拼各行与双拼预编辑只在双拼下出现、鸿蒙二合一的双拼预编辑恰好出现在一个页面上、以词定字三选一。
  - 调频按条件显示、各组的「更多选项」、逐个模式与日文词库行。
  - 桌面辅助码布局、按键速查。
  - macOS 无 Tab / PageUp 复选框且速查仍列出它们、macOS 无触发字符数。
  - 缺键时的 emoji 混输与 Control+Shift+F 默认值。
- 原生 macOS 的改动只能由 CI 编译：
  - `SkinPreviewTest.mm` 断言窗口里没有两个翻页复选框、存着的关不起作用、合并结果不写回。
  - `ShortcutTest.mm` 断言存着关时 Tab、Shift+Tab、PageUp、PageDown 照样翻页；混输合并进空字典时不带 `minimum_prefix`，合并进带着它的文档时原样保留。
- `scripts/test-reference-config-coverage.py` 和 `platforms/macos/tests/settings/preference_coverage.py` 通过。
