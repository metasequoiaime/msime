# Agent Note: macOS 候选注释一律包括号、紧贴候选字

Status: implemented

## Problem

macOS 候选窗里，候选字后面的注释有两种画法。五笔编码提示（`MSIMEWubiCodeHint`，只给已输入编码之后剩下的几码）由 `InputController.mm` 包成「(riy)」；引擎给的 `annotation`（拼音方案下的辅助码如 `wqvb`、emoji 和颜文字说明、地名所属省份）原样画出，没有括号。两种都作为单独一段画在候选字后面，中间留 `annotationGap`（4pt）。用户看到的是「你好 wqvb」和「株洲 (riy)」并存，辅助码像是候选字的一部分，被要求改成「你好(wqvb)」：加括号，不要空格。

## Decision

- 跟在候选字后面的注释一律包在半角括号里：`MSIMEParenthesizedAnnotation` 去掉首尾空白，空串不显示，已经由半角或全角括号括起的原样保留，其余包成 `(…)`。五笔编码提示本来就是 `(…)`，经过这一步不变。
- 候选窗那一段（`CandidateAnnotationRun` 的结果）和悬停提示、读屏用的完整文字（`CandidateDisplay`）用同一个规则，三处一致。
- `MSIMECandidateLayoutMetrics` 的 `annotationGap` 改为 0：括号本身就是分隔，注释紧贴候选字。同一行放不下时整段挪到下一行的规则不变。
- 韩文汉字列表的훈음 仍走释义行（`hanjaReadings` 分支），不加括号：它画在汉字下面的小字行里，不跟在候选字后面。脚本插件的注释（`script_annotation`）另有画法，不受影响。

## Alternatives considered

- **只给辅助码加括号，emoji 说明、地名省份不加。** 最强的理由是这几类注释本来就是说明文字，加括号多两个字符宽。不用它，是因为引擎的 `annotation` 不区分来源，按内容猜来源（字母就是辅助码）会把「笑脸 grinning」这类混合说明猜错；而且同一个候选窗里一部分有括号一部分没有，正是这次要消除的不一致。
- **保留 4pt 空隙，只加括号。** 「你好 (wqvb)」也能分清，但用户明确要求不要空格；括号已经把注释和候选字分开，再留空隙只是让每个候选更宽、一页放得更少。
- **用全角括号「（wqvb）」。** 和中文更搭，但全角括号各占一个汉字宽，横排一页会少放候选；而五笔编码提示一直是半角括号，改全角要动两处的约定。

## Consequences

- 收益：辅助码、五笔编码提示和其他注释在同一个候选窗里写法一致，一眼能分出哪部分是候选字。注释紧贴候选字，带注释的候选比原来窄 4pt。
- 代价：没有注释的候选不受影响；带注释的候选多了一对括号的宽度。原来就带括号的注释（测试里的「(aB)」）显示不变。
- Windows 的候选窗（`candidate_annotation_run`）仍是原来的画法，待跟进。

## Verification

`ShortcutTest.mm` 新增断言：裸辅助码 `wqvb` 显示为「你好(wqvb)」，首尾空白去掉，全角括号的「（广东省）」原样保留，只有空白的注释不显示；原有「漢語(aB)」等断言不变。`shortcut-test`、`candidate-row-layout-test` 在本机图形会话里通过，`candidate-item-layout` 的 ctest 在 Studio 上通过。
