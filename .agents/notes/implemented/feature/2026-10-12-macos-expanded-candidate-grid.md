# Agent Note: macOS 横排候选窗按 ↓ 展开多行候选网格

Status: implemented

## Problem

#7307：横排候选窗一次只看得到一页，翻得快了会错过要的词，还得翻回来再找一遍。报告人希望像搜狗和 macOS 自带输入法那样，按 ↑/↓ 把后面几页一起展开。

桌面三平台的方向键规则是刻意统一过的（[2026-10-10-sentence-conversion-edit.md](2026-10-10-sentence-conversion-edit.md)）：组字时 ↑/↓ 移候选高亮，←/→ 移组字光标或进入整句改字，不看候选窗横排还是竖排。直接把 ↑/↓ 改成「展开」会让 macOS 再次和 Windows、Linux 分叉，也会改掉所有现有用户的 ↓。触屏键盘早就有展开面板（Android、iOS 的候选网格，[2026-10-08-nine-key-expanded-panel.md](2026-10-08-nine-key-expanded-panel.md)），桌面没有。

## Decision

macOS 普通拼音候选的方向键分工及展开末页显示由[拼音与候选方向键分工](../bug-fix/2026-10-14-macos-arrow-candidate-navigation.md)部分取代，原有引擎契约和其它平台行为保留。

普通候选的鼠标展开按钮由[独立展开按钮](2026-10-14-macos-independent-expand-button.md)取代绑定页码的显示规则；本篇的网格按键、候选身份与重载机制保留。

收起后的高亮由[收起时保留当前高亮](../bug-fix/2026-10-22-macos-expanded-collapse-highlight.md)取代原先恢复引擎旧高亮的规则；收起只导航，不提交组字。

**新的共享偏好 `expand_candidates`（设置页叫「展开候选」），默认关闭。** 字段在 `crates/client-core/src/preferences.rs`（`#[serde(default)]`，缺字段的旧文档读成关闭），能力位 `HostCapabilities::expand_candidates` 目前只在 macOS 为真；共享设置页只在这个能力下于「候选窗口 › 布局」组显示开关，原生设置窗口的候选窗口卡片里有同名开关，本机键 `MSIMEClientExpandCandidates` 在该分区的恢复列表里。Windows、Linux 以后接上同一个偏好时只需打开各自的能力位。

皮肤窗口的布局选择由[共享出厂布局](2026-10-20-skin-factory-layout.md)补充约束：明确 `expandPanel = false` 的皮肤使用普通列表，true 才在用户已启用展开时提供卷轴；没有共享版式的原生主题保留本篇规则。韩文汉字、注音和释义页不受卷轴接管。

**网格之外的方向键规则不变。** 偏好关闭、候选窗竖排、只有一页、候选列表（韩文汉字、注音）或释义页开着时，↓ 和以前一样移高亮；←/→ 无论偏好如何都照旧移光标或改字。只有偏好打开、横排、多页时，↓ 和候选窗首行 ‹ › 后面多出的展开按钮才打开网格。展开按钮跟着页码和翻页按钮按 `show_candidate_page_number` 显示：关掉它们的人要的是只剩候选行的卡片，↓ 照样能展开。注音组字时 ↓ 打开候选列表（`MSIMECandidateListDownKey`）排在前面，优先于展开。

**网格里的键。** 网格开着时它接管键盘（`handleExpandedCandidateEvent:client:`，在 `handleKeyEvent:client:` 里紧跟 `syncPageSize` 之后，先于数字、删除和翻页的判断）：

- ←/→ 在当前行里移动，不换行；↑/↓ 在行之间移动并保持列，目标行比这一列短时停在它的最后一个。
- PageUp/PageDown、打开的字符翻页键（- =、[ ]、, .）和打开时的 Tab 一次滚过可见的几行，到头为止。字符翻页键被以词定字占用、是拼写输入（表达式模式）或临时日文的 - = 时不算翻页，判断和普通候选窗一致。
- 空格、回车选高亮的那一个；数字键选高亮那一行里的第几个（只有那一行标序号）。九键组字或数字、空格是拼写输入时它们不是选词键。
- Esc，或在第一行再按 ↑，收起网格，组字保留，普通候选窗与引擎高亮同步到网格当前候选。
- 网格不认的键（字母、退格、带修饰键的组合等）先收起网格再照常处理，免得被一个用户忘了的模式吞掉，和释义页（`handleGlossSenseEvent:client:`）是同一个约定。
- 鼠标点网格里的候选选它（先 `reportPointerToLifecycle`）；首行右端的收起按钮收起；滚轮在网格上一次滚一行，与「滚轮翻页」开关无关，高亮跟着留在可见范围里。

光标怎么走、可见的是哪几行是纯下标计算，放在 `shared/candidate/CandidateExpandedGrid.h`（`StepExpandedGrid`、`ExpandedGridFirstRow`，一次最多画 `kExpandedGridVisibleRows` 行），不碰 AppKit；Linux 原生画布的网格用同一份（[X11 原生画布默认](../architecture/2026-10-21-linux-x11-native-candidate-default.md)）。

**数据与代次。** 网格不经 View 的分页：打开时经 `msime_client_all_candidates` 复制一次当前这一代的全部候选（`MSIMEClientSession allCandidatesWithError:`），只认当前视图这个会话、这一代、身份合法的条目，每一行里按本机置顶词重排，和普通候选窗画那一页时一样；记下这一代和打开时的组字（`editing_text`、`preedit`、`caret_position`、`phrase_prefix`）。选择用 `msime_client_select_any_candidate` 带着复制时的一代（`selectAnyCandidateGeneration:index:error:`），控制器先用 `MSIMECurrentCandidateIdentity` 对照当前视图，Engine 再拒绝过期的一代。每次重画前对照一次当前视图（`expandedCandidatesStillApply`）：视图对象没变就直接画；变了而组字不变（云候选、AI 回答、整句重排到达，或同一代补上释义）就按新的一代重载，高亮留在原来那个词上、找不到时夹在范围内；组字变了、偏好关掉、改成竖排、切到英文、候选列表或释义页打开、失焦，就收起。ABI 没有改动，两个函数在 `crates/host-api/include/msime_client.h` 里早已存在，Android 和 iOS 的展开面板用的就是它们。

**画法。** `renderExpandedCandidates` 和普通候选窗共用皮肤、字体、首行（logo、拼音）和装饰：每一行按普通候选窗排一页的方式排（`candidatePageGeometry:`，即 `shared/candidate/CandidateItemLayout.h`），行与行上下叠起来，卡片宽度取最宽的一行；网格的行不为还没到的释义预留高度（`reservesGloss:NO`），普通候选窗为防止释义到达时卡片变高才预留，而网格里只有 Engine 当前那一页会请求释义，其余行一直空着那一行会把网格撑得很稀；候选按钮由两条路径共用的 `candidateButtonFor:geometry:slot:number:...` 建，`MSIMECandidateButton`、`MSIMECandidateChromeView` 和 `refreshCandidateSkin` 原样复用。网格按钮的标签是它在网格里的位置，高亮跟网格的光标走；首行右端是高亮所在的行「2 / 7」和收起按钮（标签 -3，模板符号图，按皮肤次要色着色）。网格里的候选不挂右键菜单：置顶、删除、固定排位的 Engine 调用只认当前这一页。双拼键位图按普通候选窗一页的高度让开候选窗，网格开着时先收起它。画网格时清掉 `_renderedCandidateView`，收起后第一次重画走完整重建，而不是在网格的按钮上原地改释义。

## Alternatives considered

- **照 issue 原话，↑/↓ 总是展开、不设开关**：最强的理由是零配置，和搜狗、macOS 自带输入法的手感一致。不选它：桌面三平台的方向键规则是产品决定统一过的，改掉它会让 macOS 和 Windows、Linux 再次分叉，也会改掉所有现有用户的 ↓；维护者决定保留规则、加默认关闭的偏好。
- **按 Android 的做法，候选一换代就收起网格**：最强的理由是简单，网格永远只画打开时那一代，不可能和 Engine 对不上。不选它：云候选、AI 回答和整句重排都会在用户看着网格时换一代，iOS 的展开面板正是因为「AI 结果一到面板就没了」改成了原地重载（#6888）。这里按组字是否变化区分：组字没变就重载，变了才收起。
- **靠 View 的分页逐页取后面几页（发 MSIME_NEXT_PAGE）**：最强的理由是不需要第二种选择接口，所有候选都走 `msime_client_select`。不选它：每翻一页 Engine 的当前页和高亮都跟着动，收起网格后普通候选窗停在别的页上；画五行要发五次派发，每次都进生命周期和脚本插件的钩子。`msime_client_all_candidates` 一次复制全部，正是为展开面板设计的。
- **用 NSScrollView 做真正的平滑滚动**：最强的理由是触控板滚动更自然，长列表不必按行跳。不选它：非激活面板里的滚动视图要自己处理焦点、滚轮和皮肤背景，高亮行保持可见还得算偏移；按行开窗口（一次画几行）让键盘、滚轮和高亮跟随共用一套下标计算，候选按钮和皮肤代码原样复用。
- **复用 `src/candidate/CandidatePanel.mm`**：维护者提到它，它的名字看起来就是候选面板。不选它：那是 Apple 快照里直连引擎的控制器留下的面板，产品不编译它，只有 `candidate-panel-test` 用（见 `platforms/macos/CMakeLists.txt` 的注释）；产品的候选窗在 `InputController.mm`，画法在 `CandidateChrome.h` 和 `CandidateItemLayout.h`，网格复用的是这两份。
- **网格里不认的键一律吞掉**：最强的理由是不会因为误按而丢掉网格。不选它：字母、退格是用户接着打字，吞掉它们等于让网格变成一个要先退出的模式；释义页已经是「不认的键先收起再照常处理」，两处保持一致。

展开视图与普通候选窗共用 `show_candidate_page_number` 的首行显示规则：关闭时不绘制行号和收起按钮，logo、预编辑也关闭时不预留首行高度。键盘的 `Esc` 和第一行 `↑` 仍能收起网格。#7764 的空白首行和隐藏页码后仍出现控件来自展开渲染漏用此规则；控制器集成测试覆盖空首行、仅预编辑和恢复控件三种状态。展开动画属于独立视觉选择，不改变此处布局契约。

## Consequences

- **收益**：横排候选窗可以一眼看几页、用方向键在页与页之间走，鼠标用户有展开按钮；默认关闭，现有用户的方向键、翻页键和数字键一个都没变。Engine 和 C ABI 没有改动；网格的下标计算可以单独测。
- **代价与已知上限**：`expand_candidates` 是 `Preferences` 的顶层字段，结构带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读写过这个字段的文档，与加 `input_mode_status_item` 时相同；MCP 还没有暴露这个开关。设置页没有界面语言切换，开关的文字只有简体中文（仓库目前没有本地化框架，没有地方放繁体和英文文案）。网格里没有右键菜单；收起网格时普通候选窗停在 Engine 原来的那一页和高亮上，不跟网格的光标走。网格不认的标点键先收起网格再交给 Engine，上屏的是 Engine 那一页的高亮，不是网格里的光标。释义只为 Engine 当前那一页请求，网格里其他行的候选没有释义。打开网格时只比较「组字是否变化」，所以同一组字下 Engine 主动换掉整批候选（比如设置变了）会被当成重载而不是收起。行高不同（有的行折行或带释义）时滚动会让卡片高度略有变化。Windows 和 Linux 还没有实现，它们的设置页不显示这个开关。
- **验证**：`cargo test -p msime-client-core -p msime-host-api` 覆盖偏好默认关闭、缺字段读成关闭、往返，能力位只在 macOS 为真，以及经 C ABI 读到的能力值；`apps/desktop/tests/settings/settings.test.tsx` 断言开关只在 macOS 出现、默认关闭、打开后保存 `expand_candidates: true`。macOS 原生：`candidate-expanded-grid`（`tests/candidate/CandidateExpandedGridTest.cpp`）覆盖行内、行间、短行、第一行 ↑ 收起、翻页和可见窗口；`shortcut` 里的 `TestExpandedCandidateGrid` 覆盖偏好关闭、竖排、只有一页时 ↓ 和 ←/→ 不变，展开不发 Engine 命令，网格里的移动、数字、空格、回车、Esc、第一行 ↑、不认的键、鼠标展开/收起/点选（指针先于选择报告）、偏好关闭或改竖排后收起，以及原生开关与共享文档读写同一个值；`TestExpandedCandidateGridGeneration` 覆盖同一组字换代时重载并保持高亮、原来的词没了时夹住、组字变化时收起、视图已换代时点旧按钮不选、Engine 拒绝过期一代时不上屏、焦点换客户端时收起。真机上的观感（浅色、深色、皮肤装饰、触控板滚动、刘海屏宽度）还没有在装好的输入法里看过。

共享皮肤声明 `expandPanel = true` 时，展开／收起入口另按[共享皮肤布局](2026-10-20-skin-factory-layout.md)使用来源字号、两段折线和候选右侧窄列；收起入口由独立按钮偏好控制，不借用原生行号开关。本篇的首行模板图与行号布局继续适用于没有该声明的原生布局，键盘选词、身份校验和视图换代规则不变。

主动部分选词后继续保留网格的生命周期由[部分选词保留展开](../bug-fix/2026-10-25-macos-expanded-partial-selection.md)补充：剩一页也保留，整句结束、旧代次拒绝与普通编辑仍沿用关闭规则。
