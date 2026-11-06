# Agent Note: Windows 候选窗拼音行按引擎给的位置画插入点

Status: implemented

## Problem

#10792：Windows 候选窗的拼音行从来不画组字插入点，全拼、双拼和各种双拼预编辑样式都一样，develop 上也是。PR #10720 的真机验收里，macOS 候选窗打完 `nimf` 在 `ni'mf|` 画出插入点，Windows 同样的步骤在末尾和移动后都没有。

候选窗只在 `CandidatePresentation::preedit_caret` 不是 npos 时画插入点。这个值有两条投影来源：按键送达后的 `candidate_presentation`（每次打字、移光标都走它）和异步结果回来时的 `candidate_presentation_from_view`（释义、云候选重画）。#2097 加插入点时只给后者算了位置，前者从来不设，所以每次按键重画都把插入点丢掉，只有偶尔一次异步重画时才露出来。后者用的又是宿主自己的 `preedit_display_caret`，按按键原文的字母数换算，双拼「拼音分词」展开的全拼（`uiui` 显示为 `shi'shi`）字母对不上就不画；引擎早已在 `View.preedit_caret` 里给出按音节换好的位置（[组字光标在 preedit 里的位置由引擎给出](2026-11-02-engine-preedit-caret.md)），候选窗没有读。

验收里另外看到的两件事不是缺陷，这次不改：第一次按左键进入整句改字（行内变成 `你|们`，之后一次移一个字）是 `msime_client.h`「整句改字」一节规定的桌面行为，在拼音里按音节移光标用 Shift/Ctrl+左右；有候选时 Home/End 翻到第一页/最后一页是 TSF 既有的键位映射。

## Decision

`platforms/windows/src/candidate/CandidatePresentation.h` 新增 `candidate_view_preedit_caret(view, preedit)`，两条投影都用它算插入点，再加上宿主画在前面的已选词段长度：

- view 带 `preedit_caret`（非负整数）时按它放。它按 Unicode 标量计，换成 UTF-8 字节；越过 `preedit` 时两边对不上，返回 npos，不画。
- view 没有这一项时退回 `preedit_display_caret`，按原文字母数换，规则和以前一样；字段明确存在但为负数、空值或非整数时返回 npos，不借旧协议的回退猜光标。
- 先区分有符号与无符号整数，再以 `uint64_t` 检查位置不超过预编辑字节数，之后才能收窄成 `size_t` 并按标量遍历。否则大于 `INT64_MAX` 的无符号数可能被当成负数回退，32 位宿主还可能把大整数截成有效位置。

联想（`prediction`）没有拼音，仍不画插入点。候选窗的绘制和几何（`candidate_preedit_caret`、拼音宽度里预留的让位）不变。

引擎在两段文字没有可靠对应时（韩文音节、假名、笔画字形）把 `preedit_caret` 放在 `preedit` 末尾，所以这些方案的拼音行现在在末尾画插入点，和 macOS 候选窗一样；以前 Windows 在刷新路径上对它们不画。

## Alternatives considered

- **只在按键路径上补一份 `preedit_display_caret`，不读引擎的位置。** 改动最小，全拼和双拼分隔（`ni'mf` 只是字母间加了隔音符）都能画对。但双拼「拼音分词」仍然不画，而且候选窗和 TSF 行内（#10677 已改读 `preedit_caret`）在同一个组字上用两套规则，以后再出现展开的拼写还要再修一次。
- **和 Linux（#10788）一样，展开过的拼写只在光标停在中间时画、末尾不画。** Linux 的 IBus 辅助行只能插一个字符 `|`，末尾那个竖线对日文、韩文是多出来的显示。Windows 候选窗画的是几何插入点，末尾一直画（全拼、原始按键本来就是这样），在末尾不画反而让同一个候选窗在不同方案里时有时无；macOS 候选窗也是末尾照画。
- **改字（`View.conversion` 非空）时拼音行不画插入点。** 改字期间左右键移的是行内整句里的光标，拼音行的插入点不随左右键动，可能让人以为左右键没生效。但这时键入的字母会先退出改字、落在 `caret_position` 处，拼音行的插入点正好标出下一个字母的位置；多一条特殊判断也就多一处和 macOS、Linux 不一样的地方。没有采用。

## Consequences

收益：打字和用 Shift/Ctrl+左右移光标时，候选窗拼音行一直有插入点，位置和 TSF 行内、macOS 候选窗用的是同一个引擎位置；双拼「拼音分词」也能画。代价：插入点现在每次按键都画，拼音行宽度相应多算一段让位（`candidate_preedit_caret_slot`，约 2.95 DIP），卡片可能比以前宽这么一点；韩文、日文、笔画的拼音行末尾多一条竖线。

验证：`platforms/windows/tests/input/preedit_caret.cpp` 补了按键路径（原样、末尾、已选词段前缀）、引擎位置（双拼分隔 `ni'mf` 的四个位置在两条投影上一致、拼音分词 `shi'shi`、韩文按字节换算、越界不画）的用例。改代码前在 Mac Studio 上用 clang 编译运行，按键路径第一条断言失败；改后通过，`preedit_display_caret.cpp`、`prediction_presentation.cpp` 等候选窗投影测试也通过，`scripts/test-windows-native-run.py` 全部通过。`preedit_caret.cpp` 要链接繁体换算，`scripts/test-windows-native-run.py` 链接真实 `msime-host-api` 静态库运行它（见下一段），不用恒等转换替身。没有在 Windows 真机上看插入点，留给设备验收。

本轮审查补齐非法字段边界：合成 `ni` / 原始按键光标 1，明确给 `preedit_caret` 空值、布尔、浮点、字符串、负数、容器或超大无符号数，两条投影都必须不画；合法无符号 0、1、2 仍按原位置画。旧实现先在空值断言失败（第 229 行），修复后使用真实 `msime-host-api` 静态库运行 `preedit_caret.cpp` 与 `candidate_initialization.cpp`，两项全部通过、无跳过，不再用恒等转换替身。同步到 `776b4703c7` 后本地 quick 通过：原生 runner 215 通过、2 项宿主差异排除、84 项需 Windows 构建；Rust workspace、共享 Apple bridge 及 x86_64/i686 pipe 构建通过，其他平台按实际 PR 差异跳过。正常 Rust 生产者输出 `usize`，这项是协议字段的防御边界，不声称普通输入能产生负数或空值。

## Existing notes audit

检索 `preedit_display_caret`、`candidate_presentation`、`preedit_caret`、插入点。[组字光标在 preedit 里的位置由引擎给出](2026-11-02-engine-preedit-caret.md) 当时写明 Windows 候选窗不改，本篇接着把候选窗改读引擎位置，部分重叠，已在那篇加链接。[全拼宿主预编辑使用引擎分词](2026-10-23-quanpin-host-preedit-segmentation.md) 引入了 `preedit_display_caret` 并写了「按键回包那条路本来就不画插入点」，本篇保留它作视图缺字段时的退路，按键路径改为画，部分重叠，已在那篇补链接。[Windows 候选窗的缺省几何、动效时机与细节对齐 macOS](../feature/2026-10-20-windows-candidate-mac-default-geometry.md) 持有插入点的画法和让位几何，本篇不动，无关。[双拼预编辑的第三档「双拼分隔」存成单独的布尔键](../feature/2026-11-03-shuangpin-separated-preedit.md) 说宿主按字母数换算与引擎一致，仍成立，无关。
