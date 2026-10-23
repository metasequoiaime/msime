# Agent Note: 横排候选强制单行由逐页自适应宽度上限吸收

Status: implemented

## Problem

参考产品 v0.11.1 有 `appearance.candidate_window_single_line`：横排候选窗开时以屏宽为上限、放不下才换行，关时以半屏为上限、超过半屏就换行（`server/src/window/candidate_presenter.cpp` 的 `widthFraction` 取 1.0 或 0.5，`ui/src/CandidateList.cpp` 的 `Measure` 按上限折行；出厂默认关）。新架构偏好层没有对应字段，需要判定 macOS（及其他桌面宿主）是缺了这条设置，还是已经由别的机制回答。

## Decision

不新增偏好字段。三个桌面宿主共用同一条逐页自适应规则，已经逐页产出这个开关两端中较好的那一个结果：

- 卡片默认以光标屏可见区域一半封顶：`kCandidateWidthCapRatio = 0.5`（`shared/candidate/CandidateChromeMetrics.h`）。
- 横排页的全部候选行在单行里放得下屏宽减两侧边距（`kCandidateScreenMargin = 16`）时，上限放宽到该页自然宽度，整页保持一行：判定走 `SingleLineMinimumWidth`，行内各列由 `SingleLineColumns` 分配，放不下时才收窄释义行而不是起第二行。
- 候选行本身超出屏宽时，上限退回半屏，`LayoutCandidatePage` 按自然列宽折行成多行（列宽超整行的单独收窄并在列内折行）。

同一规则在三处实现且互相对齐：macOS `platforms/macos/src/input/InputController.mm` 的 `candidatePageGeometry`（`widthCap`/`screenCap` 分支）、Linux `platforms/linux/src/candidates/NativeCanvasChrome.h` 的 `cap`/`screen_cap`、Windows `platforms/windows/src/candidate/CandidateWindow.cpp` 的 `max_width`/`max_single_line_width` 与 `CandidateCardSize.h` 的 `width_cap`。

按页宽分三段对照，新规则每一段的产出都等于开关某一档的产出：

| 页单行最小宽度 | 参考「开」 | 参考「关」（出厂默认） | 新架构自适应 |
| --- | --- | --- | --- |
| ≤ 半屏 | 单行 | 单行 | 单行 |
| 半屏 ～ 屏宽 | 单行 | 半屏内折行 | 单行（同「开」） |
| > 屏宽 | 屏宽内折行 | 半屏内折行 | 半屏内折行（同「关」） |

也就是说：能单行的页总是单行——这是「强制单行」这个开关的本意；放不下的页给半屏紧凑折行——这是「关」档的形状。开关两端各自牺牲的恰好是对方区间里较好的结果，自适应把它们都保住了。参考配置键 `appearance.candidate_window_single_line` 在 `scripts/test-reference-config-coverage.py` 的 MAPPING 里以 `!replaced:` 登记为被这条规则取代。

## Existing notes audit

- [Windows 候选窗对齐 macOS](../feature/2026-10-10-windows-candidate-window-mac-parity.md)：把同一套单行最小宽度、列分配与折行几何从 macOS 移植到 Windows 宿主，本判定依赖它留下的 `max_single_line_width` 无条件生效这一事实。
- [Windows 候选窗缺省几何对齐 macOS](../feature/2026-10-20-windows-candidate-mac-default-geometry.md)与[Linux 候选样式对齐](../feature/2026-10-20-linux-candidate-style-mac-parity.md)：共享常量 `kCandidateWidthCapRatio`/`kCandidateScreenMargin` 与 `shared/candidate/CandidateItemLayout.h` 的移植归宿，三宿主规则一致是本次判定的前提。

## Alternatives considered

- **照搬布尔字段 `candidate_window_single_line`。** 最强理由是逐键对齐参考、迁移用户能找回出厂的「关」档形状。不用：两档固定上限各自在某段页宽上回退——「关」让半屏到屏宽之间本来能单行的页折行，「开」让超屏宽的页在整屏宽度里折行；默认值无论选哪档都会让一部分现有用户看到更差的形状。而且这个键所属产品的后继（新架构 Windows 宿主）自己也没有接它，只在 macOS 接会造成共享偏好在各宿主语义不一致，三宿主都接则为一个卡片宽度上限付出三份管线。
- **三态 `auto | always | never`。** 最强理由是 `auto` 保留自适应、`always`/`never` 恢复两档固定上限，语义诚实且默认无回归。不用：为一个纯外观宽度上限新增枚举字段、三宿主管线与设置页控件，收益只是恢复被自适应规则支配的两个固定档；设置页外观区正在精简而不是扩充。

## Consequences

- 收益：不新增偏好字段、设置控件与各宿主的 NOT_APPLICABLE 登记；三个桌面宿主的横排几何继续保持一份规则，MAPPING 条目防止以后再把这键当缺口翻出来。
- 代价：用户无法强制两种固定形状——横排时想要永远紧凑（参考「关」）或永远顶到屏宽上限（参考「开」）都不行，差异可感知：中带页在参考「关」下折行、这里单行；超宽页在参考「开」下屏宽折行、这里半屏折行。要一直窄窗口的用户可走竖排候选；超宽页两种折行都只是卡片宽度差别，行数与可读性接近。
- 验证：本次不落地代码行为变更，无新测试；现有 `candidate-item-layout` 已覆盖 `SingleLineColumns`/`LayoutCandidatePage` 的单行分配与折行回退，`scripts/test-reference-config-coverage.py` 通过即 MAPPING 条目合法。
