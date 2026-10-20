# Agent Note: Windows 候选窗的缺省几何、动效时机与细节对齐 macOS

Status: implemented

## Problem

macOS 与 Windows 候选窗的差异调查（批 20、21）发现，Windows 在没有皮肤版式键时仍沿用 msime-windows 呈现器的旧几何：序号按完整候选字号画在 0.8em 宽的一栏里右对齐，候选文字从 1em + 8 DIP 处开始；行高是 1.35 倍字号加 2，行间另留 2 DIP；卡片内边距左右 6、文字后再加 14 DIP；释义是 0.78 倍候选字号，字号调大释义跟着变大；最小宽度固定 160 DIP；首行 22 DIP，翻页箭头只有 16 DIP 的点击区，页码离箭头 12 DIP；引擎注释原样画、离候选 4 DIP。18pt 时 Windows 一行约 26 DIP，macOS 约 34pt，两边看上去是两个产品。共享偏好 `show_candidate_page_number` 在 Windows 上没人读，页码和箭头一直画。

动效和细节也不一致：皮肤动效在快速打字时每个新词开头都重播进场，`stagger_ms` 只解析不使用，缓动是 smoothstep；失焦、断开时旧候选也在新的前台上播退场；脚本插件的注释接在引擎注释后面、非斜体；读音和释义同色；固定排位候选用主题强调色；右键菜单「删除」不提示 Windows 已有的 Ctrl+Shift+Alt+N；拼音里的插入点压在字上，光标走出可见范围就不画。

## Decision

**缺省几何照 macOS，只改皮肤没写对应键的路径。** 规则都在 `src/candidate/CandidateCardSize.h` 的 `candidate_card_metrics`：

- 卡片四边到候选行 4 DIP（`pad_x` 8、`pad_y`、`slack_y` 4、`slack_x` 0），行间距 0；高亮和悬停底色比行四边各缩进 1 DIP（`selection_inset`，macOS 的 `NSInsetRect(bounds, 1, 1)`），皮肤写了 `item_gap` 时底色铺满整行。
- 行高是 DirectWrite 实测的一行候选字高加上下各 6 DIP。窗口用 `CandidateWindow::candidate_line_height` 量：主字体空串的一行，以及这一页每个候选的文字和注释各量一次，取最高的，回退字体比主字体高时行也跟着高，和 macOS 按每个候选量 `MSIMECandidateTextHeight` 一样。量出来的值经 `CandidateCardInput::line_height` 和 `candidate_card_metrics` 的 `line_height` 参数进来，`card_bounds` 和 `paint` 用同一个，没量时按 1.35 倍字号估。折行的候选文字也在量出的高度上加这 12 DIP（`row_padding`），和 macOS 的 `MSIMECandidateRunMeasure` 一样。皮肤写了 `item_extra_height` 时仍是 1.35 倍字号加它，折行不另加。
- 序号是 0.8 倍候选字号，从行左缘 8 DIP 处左对齐，候选文字从这一页最宽的序号（`number_label_width` 现在总是量）之后 1.5 DIP 处开始，内容后留 8 DIP（`item_padding_right`）。皮肤写了 `number_inset` 时保持 msime-windows 的算法；写了 `item_padding_right` 时卡片右侧另留宿主的 14 DIP，和以前一样。
- 引擎注释一律包在半角括号里、紧贴候选（`candidate_annotation_run`，规则同 macOS 的 `MSIMEParenthesizedAnnotation`：去首尾空白，已带半角或全角括号的不再加，`annotation_gap` 为 0）。
- 释义固定 12 DIP（`translation_font`），不随候选字号变大；用户的整体缩放照样作用在它上面。
- 最小宽度是 7 倍候选字号，超过半个工作区时由封顶压回去。带着共享层出厂版式（catalog 的 `resolvedLayout`，`CandidateSkinLayout::shared`）的皮肤和 macOS 一样用 msime-windows 固定的 160 DIP，`fit_content` 时没有下限。
- 首行有 logo 或翻页时 26 DIP，页码离拼音和箭头各 4 DIP，每个箭头是 28 DIP 宽、与首行等高的点击区。

**页码开关。** `CandidateLayoutSettings::show_page_number` 读 `show_candidate_page_number`（缺值按开），编码进主循环的原子量。关掉时 `pager_label` 返回空，`card_bounds` 和 `paint` 都不画页码和箭头、首行不为它们留高度；键盘和滚轮照常翻页。原生设置窗口还没有这一行，它等展开候选通用化之后和展开、内边距一起加。

**「下一页」箭头在最后一页不变暗。** 和 macOS 不同：`input-runtime` 的 `page_count` 是已放出的候选数除以页大小，Engine 扣着的尾部要翻到最后一页才放出来，最后数到的一页往后翻仍可能有候选，变暗会让用户以为没有了。

**动效时机，规则在 `src/candidate/CandidateMotion.h`。** 卡片开始收起（`hide()` 或 `hide_immediately()` 时窗口可见）记下时刻，400ms 内再出现不播进场（`candidate_enter_allowed`，macOS 的 `kTypingBurstInterval`）。`fade_scale`、`slide_up` 的进场里候选行逐行浮现：第 i 行晚 min(i × `stagger_ms`, 80ms) 开始，按自己的进度淡入并从下方 3 DIP 升到原位；`paint` 在行循环里给画刷乘这一行的进度、给那一行套一个平移，行的位置和点击区域不动；一次进场等最后一行播完才结束（`animation_total_ms_`）。进场和选中用 Core Animation 的 ease-out（cubic-bezier(0, 0, 0.58, 1)），退场用 ease-in，与 macOS 的 `SettleCurve` 和退场曲线相同，曲线按三次贝塞尔二分求值。

**退场按隐藏原因。** `CandidatePresentation::composition_ended` 说明一个不可见快照是不是组字正常结束：投递时 view 获得焦点、`editing_text` 为空、不是 UILess 的为真；`CandidateMailbox::event` 里切到英文的 IMESwitch（不带键码）收掉还开着的组字时为真；同样不带键码的 StatusSnapshot 和 FocusRestored 不算，前者也在线程焦点变化和重连时发，后者本身就是焦点转移，和 macOS 的 `focus_out` 一样立即收起；宿主在组字还开着时发来的 HideCandidateWnd（焦点变化之类）为假，上屏、取消后跟来的 HideCandidateWnd 只是确认，保留投递时的原因。`CandidateWindow::hide_for` 据此选择：为真走 `hide()`、可以播退场，其余（读不到快照、失焦、UILess、断开、换了租约）立即收起；只有一种例外：读不到快照而退场正在播时什么也不做。主循环的读取不阻塞（`try_lock`、闸门的 `try_with_active`、`try_current` 任一失败都返回空），上屏后退场正播、下一键的工作线程恰好占着闸门时，一次读空不该把卡片在退场中途收掉。代价是退场途中换了焦点（新焦点上的快照也是空的）时，这段已经因上屏或取消开始的退场会播完；没在退场的卡片照样立即收起，焦点变化不会让旧候选开始淡出。换显示器时也立即收起，免得退场计时在新位置把刚出现的卡片收掉。

**脚本插件的注释单独占释义那一段的最后一行，斜体；读音减淡。** `candidate_secondary_run` 给出释义那一段的文字，连同读音的位置（`candidate_gloss_display` 记下）和插件注释那一行的起点；`candidate_secondary_text` 是它的文字。插件注释先经 `candidate_script_annotation_line` 收成一行（控制字符和各种换行换成一个空格），加上去超过 4096 字节时不带。`paint` 用一个 `IDWriteTextLayout` 同时设预选列的下划线、读音的 0.72 倍透明度画刷（选中底色不透明时不减淡）和插件注释的斜体；建不出来时整段按释义色画。悬停提示和行数都把插件注释算进去，和 macOS 的 toolTip 一样。这推翻了 [Windows 候选窗补齐 macOS](2026-10-10-windows-candidate-window-mac-parity.md) 里「插件注释接在引擎注释后面」的决定，那篇的其余部分仍然成立。

**固定排位色。** 未高亮的固定排位候选用 macOS 的固定色 `#379AD3`（`candidate_fixed_position_rgb`），括号里的引擎注释和候选文字同色（macOS 用同一个 `titleAttributes` 画两者），没有皮肤释义色时它的释义是这个颜色的 0.62 倍透明度；高亮行仍用选中行的正文色，蓝字压在不透明的绿色选中底上读不出来，这里不照搬 macOS 在高亮行上也画蓝字的做法。

**右键菜单的快捷键提示。** `candidate_menu_items` 接受候选在本页的位置，前 8 个候选的「删除」行带 `Ctrl+Shift+Alt+N`（`MaintenanceHotkeys.h` 的那组快捷键，直接删本页第 N 个候选）；`candidate_menu_size` 在有提示时把菜单加宽 `shortcut_column`，`CandidateFlyoutWindow` 用正文色的 60% 右对齐画在行尾。「置顶」不改成「取消置顶」：Engine 不报告候选是否置顶过，macOS 的这个状态来自它自己存在本机的置顶表。菜单仍只响应鼠标，浮层不激活、拿不到键盘焦点。

**拼音里的插入点。** `candidate_preedit_caret` 照 macOS 的 `MSIMECandidatePreeditField`：1.25 DIP 宽的竖线，光标在拼音中间时文字在它两边各让开 0.85 DIP（后半段右移 `candidate_preedit_caret_slot`），在末尾时离最后一个字 1.5 DIP；插入点走出首行那一段时整行左移到它刚好露出来。`paint` 把拼音分成光标前后两段画，`card_bounds` 量拼音宽度时加上让出的那一段。macOS 只在 view 获得焦点时画插入点，Windows 的快照在失焦时本来就不可见，不另外判断。

**正好放得下的一段不折行。** 卡片宽度是内容宽加序号栏和留白，排版时再减回来；缺省版式不再有右侧 14 DIP 的余量，浮点误差会让正好放得下的文字被当成要折行。`candidate_item_layout` 比较宽度时容许 1e-6 DIP 的误差。

**设置窗口的预览不共享 `CandidateCardSize.h`。** 它是 XAML 画的近似预览（`settings/main.cpp` 的 `make_preview`），这次只把它的序号字号、行内边距、行间距、序号到文字的空和释义字号改成同一组缺省值。

## Alternatives considered

- **连带改掉皮肤键的含义，让写了版式键的皮肤也照 macOS 画** — 最强的理由是两种来源只剩一套几何，代码更简单。不用：msime-windows 的皮肤是照它们的几何画的图，`number_inset`、`item_padding_right` 等键改了含义，装饰和背景就对不齐；用户的决定也是只改缺省路径。
- **行高照旧按 1.35 倍字号估，只把加的 2 DIP 换成 12** — 最强的理由是不用每次排版多量十来个文本布局。不用：DirectWrite 下中文字体和回退字体的行高差别很大（emoji、生僻字的回退字体更高），按字号估会让行和 macOS 一样高却装不下字，或者在回退字体上被裁掉；macOS 也是按每个候选实测。
- **按 macOS 在最后一页把「下一页」箭头变暗** — 最强的理由是两边行为一致。不用，理由见上：Windows 用的 `page_count` 会随翻页增长，最后数到的一页不一定是最后一页，变暗是错的提示。macOS 那边同样可能把能翻的一页画成不能翻，这个差异留给 macOS 自己处理。
- **插件注释保持在注释那一段，只换成斜体** — 最强的理由是 2026-10-10 那篇写下的：释义那一段的第 N 行是第 N 种目标语言，预选列、按列上屏都按行号找。现在插件注释只追加在最后一行之后，`candidate_gloss_column_range` 从开头数行，前面各行的位置都不变；横排预留的释义行数也不算插件注释，插件注释到了这一行才长高，和 macOS 一样。换来的是插件注释和引擎注释在两个平台上画在同一个位置。
- **用 `IDWriteTextLayout::SetInlineObject` 在光标处插一个空白内联对象** — 和 macOS 的 `CTRunDelegate` 最像，整段拼音只排一次版。不用：内联对象要实现一个 COM 接口，度量还要和拼音的字体对齐；拼音是一行不折的文字，分成光标前后两段画、后一段右移，量出来的位置和画出来的一样，代码少得多。
- **右键菜单加键盘导航** — macOS 的 NSMenu 自带。不做：浮层是 NOACTIVATE 窗口，焦点一直在宿主应用里，键盘导航只能由 Server 的按键路径转发，代价远大于一行快捷键提示。

## Consequences

- **收益**：没有皮肤（以及没写版式键的皮肤）时，Windows 候选窗的序号、行高、内边距、释义字号、最小宽度、首行和翻页控件与 macOS 一致；`show_candidate_page_number` 生效；快速打字不再每个词都闪一次进场；隐私相关的收起不再在新的前台上淡出旧候选；插件注释、读音、固定排位色、删除快捷键提示和插入点的画法与 macOS 相同。
- **代价与已知上限**：每次排版多量这一页的行高和序号宽度（各十来个 DirectWrite 文本布局）。缺省卡片比以前高（18pt 时每行约多 8 DIP），竖排一页 9 个候选时更容易在屏幕底部翻到光标上方。写了版式键的 msime-windows 皮肤里没写的那几个键（例如只写了 `padding` 的皮肤的行高）也跟着换成新的缺省值，这是逐键回落的必然结果。设置窗口里还没有「显示页码」开关，现在只能经 MCP 或偏好文件改。

## Verification

本机能跑的纯策略测试（clang++ -std=c++17 编译运行）：`tests/ui/candidate_card_size.cpp`（缺省几何的每个数、实测行高、序号宽度取整、12 DIP 释义、7em 与 160 DIP 下限、首行与翻页、正好放得下不折行、插入点让位和滚动）、`tests/ui/candidate_motion.cpp`（连续输入、错峰、两条缓动曲线）、`tests/ui/candidate_layout_reload.cpp`（页码开关及其编码）、`tests/ui/candidate_palette.cpp`（固定排位色和它的释义色）、`tests/ui/candidate_menu_layout.cpp`（删除行的快捷键提示和菜单宽度）、`tests/ui/candidate_skin_layout.cpp`（`resolvedLayout` 标记共享版式）、`tests/ui/candidate_canvas.cpp`。依赖 Engine 的测试只用 MinGW 做了语法检查：`tests/candidate/script_annotation_presentation.cpp`（括号规则、插件注释那一行、读音位置、4096 字节上限）、`tests/ui/candidate_mailbox.cpp`（隐藏原因）；其中纯函数部分在本机另用一个只含这些函数的小程序跑过。`CandidateWindow.cpp`、`CandidateFlyoutWindow.cpp`、`server_main.cpp` 用 `x86_64-w64-mingw32-g++ -std=c++17 -fsyntax-only -Wall -Wextra` 检查无错误无警告，`settings/main.cpp` 只能由 PR 的 Windows CI 编译。Windows 真机上还要看：新的行高和序号位置、逐行浮现与缓动的观感、退场只在上屏和切英文时出现、插入点在长拼音里的滚动、菜单提示的宽度是否放得下。
