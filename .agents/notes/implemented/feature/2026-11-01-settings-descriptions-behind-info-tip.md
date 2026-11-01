# Agent Note: 设置项说明收进标题旁的「?」，全平台设计语言

Status: implemented

## Problem

设置页上每一行标题下面都有一到三行灰字说明，分组里还夹着整段的解释。以 macOS 的「剪贴板」页为例，「云剪贴板同步方式」一组选了 WebDAV 之后，三项输入各带两三行说明，组尾还有一段四行的注意事项，真正要填的三个框淹没在文字里。用户看了截图的反馈是「这样一点都不清爽」，要求所有页面都不要常驻大量说明，改成标题旁一个问号，鼠标放上去显示说明，并且这是全平台的设计语言，要写进项目规则。

说明本身并不都是同一种东西。同一个 `description` 属性和同一个 `SettingsGroupNote` 组件，有的在解释设置做什么，有的在显示当前值（滑块的「12pt」、数据目录路径）、校验和错误（「请输入两个不同的单个英文标点」、代理地址格式错误）、列表条目的元信息（版本、作者、登记时间、核对码），有的是空列表提示（「还没有应用例外」），有的是「当前桌面下这组设置不起作用」的宿主限制。把它们一律收进「?」会把状态和错误藏起来，比原来更糟。

## Decision

解释性说明不常驻页面，放在标题旁的「?」里；状态、值、错误、条目元信息、前置条件、宿主限制、空列表提示和紧贴按钮讲后果的一句话留在原处。规则全文在 `docs/ARCHITECTURE.md` 的「设置项说明收进「?」」，`AGENTS.md` 的「界面」一节有一条指向它的硬规则。

交互：桌面悬停或键盘聚焦弹出气泡，点一下固定，点别处或按 Esc 收起；手机没有悬停，点「?」弹出。说明文字以视觉隐藏的形式留在原处，控件的 `aria-describedby` 照旧指向它，读屏和按文字查找不受影响。

### 共享设置页（macOS、Linux、HarmonyOS）

- `packages/ui/src/core/info-tip.tsx` 的 `InfoTip` 是唯一的「?」。气泡用 portal 挂到 `[data-settings-shell]` 下按固定定位画出，与 `AiProviderSelect` 的菜单一样：分组卡片有 `overflow-hidden` 和液态玻璃的 `backdrop-filter`，画在行内的气泡会被裁掉或定位错乱；挂在设置窗口根节点下仍能拿到 `[data-platform]` 的配色 token。
- `Row` 拆成两个属性：`description` 画成标题旁的 `InfoTip`，`detail` 常驻标题下方。`SwitchRow`、`SelectRow`（含 HarmonyOS 的选择面板）、`SliderRow`、`ActionRow`、`TextInputRow`、`SegmentedRow`、`SecretSettingRow` 透传两者。约 60 处原本把状态或错误塞进 `description` 的调用改用 `detail`，「解释 + 当前值」拼在一起的（`${radius}pt，覆盖皮肤自带的圆角`）拆成两半；滑块已有 `valueText` 显示数值的，描述里重复的数值去掉。
- `SettingsGroupNote` 在有组名的 `GroupList` 里不画在原处，交给组名旁的「?」，一组多段合在同一个气泡里。`role="status"` 的、标了 `inline` 的（18 处内容型段落）和 `SettingsManagerBlock` 里的照旧画在原处。HarmonyOS 的组下页脚（语音、手写）和双拼编辑器每组的说明也改成组内说明，进组名的「?」。
- 整行是按钮的 `LinkRow` 和旧式 `<label>` 包裹的 `SettingSectionTitle` 里不能再嵌按钮（嵌进 label 还会取代输入框成为 label 指向的控件），那里的「?」是只在悬停时出气泡的图标，说明由外层的描述或标签读出。HarmonyOS 手机的 `PickerRow` 整行是按钮，说明改为写在点开后选项面板的副标题里。
- 「?」按钮统一叫「说明」，不以设置标题开头：以标题开头会和测试、读屏里按标题找控件的查询撞名。设置搜索定位到一行时跳过它（`data-info-tip`），聚焦行里真正的控件。组名外面多了一层 `[data-group-header]`，搜索索引读组名的选择器跟着改。

### 原生设置界面（Windows、iOS、Android）

各自实现一个可复用的「?」组件，按同一套分类改完所有页面，分别在各自的 PR 里落地。

## Alternatives considered

- **用元素的 `title` 属性做 tooltip。** 最省事，一行代码，浏览器自带。但它要停留约一秒才出现、样式不可控、键盘聚焦不显示，触屏设备上完全没有，HarmonyOS 手机等于把说明删掉了。
- **CSS 悬停气泡直接画在行里。** 不需要 portal 和定位计算。但分组卡片的 `overflow-hidden` 会裁掉气泡，macOS 液态玻璃的 `backdrop-filter` 还会让固定定位以卡片为包含块，气泡要么被裁要么跑偏。
- **Popover API（`popover` 属性）进顶层。** 不受裁剪，浏览器负责点外关闭。但 macOS 设置窗口跑在系统 WKWebView 上，最低支持的 macOS 13 带的是 Safari 16，Linux 的 WebKitGTK 旧版本也没有它，要再写一套回退。
- **每组底部一个「更多说明」折叠区。** 不用逐行加图标。但还是占一行、要多点一次，说明和它解释的设置项分开了，用户要自己对哪句说的是哪一项。
- **保留说明、只把文字改短。** 用户明确不要页面上的常驻说明，缩短只是程度不同。

## Consequences

- 页面只剩标题、控件和必须看见的状态，长页面短了很多；剪贴板、语音、皮肤编辑这类说明多的页面变化最大。
- 新设置项多了一个判断：这句话是解释（`description`）还是状态（`detail`）。判断错了会把错误藏进「?」，或把解释又摊回页面；规则和本笔记列了每一类的例子，code review 时按它查。
- 说明要靠一次悬停或点按才看得到，第一次接触某个设置的用户多一步。说明对读屏仍然直接可读。
- 一组多段说明合进同一个气泡，长的组（语音服务介绍、应用例外）气泡会比较高；气泡宽度限制在 340px 以内，放不下时向上展开。
- 测试里按「说明文字的父元素就是行」「组名是 section 的直接子元素」去找元素的写法跟着改了几处。

## Verification

- 共享设置页：`apps/desktop`、`apps/harmony` 的 `tsc --noEmit`，`vp lint`、`vp fmt --check`，`apps/desktop` 全量 vitest。
- 用临时页面以 `data-platform="mac"` 渲染剪贴板页的两个分组，无头 Chromium 截了收起、行的气泡展开、组名气泡展开三种状态；截图留在 `~/msime-screenshots/settings-description-tooltip/`。
- 没有在真实的 macOS 设置应用、Linux 设置应用和 HarmonyOS 设备上看过；原生三端的验证写在各自的 PR 里。
