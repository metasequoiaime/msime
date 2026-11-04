# Agent Note: macOS 候选窗按钮不再经 buttonWithTitle 建

Status: implemented

## Problem

macOS 候选窗每次按键都整页重建：`InputController.mm` 的 `renderCandidates` 只在候选只换了释义时复用旧按钮（`MSIMEOnlyCandidateTranslationsChanged`），打字时每键候选都变，于是每个候选行、翻页按钮、展开开关和在线预览按钮都重新建一遍。它们原来都用 `+[NSButton buttonWithTitle:target:action:]` 建，这个方法建好时会按标题 `sizeToFit` 一次，macOS 27 上走 SwiftUI 的尺寸计算；而每个调用方随后都直接赋排版算出的 frame，这次计算的结果从没被读过。维护者在 macOS 27 Apple Silicon 上自动打字（萤火虫皮肤，每页 6 行），诊断日志里 `candidate-frame show` 的 `build_ms` p50 21 ms、p95 29.6 ms，比桌面整键预算高一个量级，sample 里 `renderCandidates` 的时间大多在 `buttonWithTitle` 里（#10727）。

## Decision

候选窗的按钮统一由 `platforms/macos/src/candidate/CandidateChrome.h` 的 `MSIMEMakeCandidateButton(Class, title, target, action)` 建。它用 `initWithFrame:NSZeroRect` 建按钮，不做任何尺寸计算，再把 `initWithFrame:` 和 `buttonWithTitle` 不一样的四项默认值改回后者的值：

- 标题：`initWithFrame:` 默认是 "Button"，工厂总是写入调用方给的标题，空串也写。
- 换行方式：`initWithFrame:` 是按词换行，`buttonWithTitle` 是尾部截断（连带 `cell.wraps` 为 NO）。
- 图片缩放：`initWithFrame:` 是不缩放，`buttonWithTitle` 是按比例缩小。
- 边框样式：`buttonWithTitle` 给重写了 `-drawRect:` 的子类设 push 样式，给没重写的保持 automatic；`initWithFrame:` 一律是 automatic。候选按钮的三个类（`MSIMECandidateButton`、`MSIMECandidatePagerButton`、`MSIMECandidateExpandToggleButton`）都重写了 `-drawRect:`，所以工厂设 push（源码里写作 `NSBezelStyleRounded`，与仓库其余地方一致，旧 SDK 也认）。这一项是实测出来的，不在文档里。

`renderCandidates` 及其辅助方法里的六处（候选行 `candidateButtonFor:`、原生和皮肤两个展开开关、列表翻页、首行 ‹ ›、在线/AI 预览）都改用这个工厂；展开网格经 `candidateButtonFor:` 一并受益。旧版面板 `CandidatePanel.mm` 的两个翻页按钮和候选窗以外的按钮（设置页、词库窗口、浮动工具栏等）不在每键路径上，照旧用 `buttonWithTitle`。

## Alternatives considered

- **跨按键复用同一页的按钮** — 连 `initWithFrame:` 和加入视图树的开销都省掉，理论上收益最大。但复用要把菜单提供者、tracking area、无障碍标签、选中动效、悬停状态逐项重置，漏一项就是状态串到下一键的候选上，改动面和风险都比这次大得多。先去掉白算的 `sizeToFit`，等设备上的 `build_ms` 数据出来仍超预算再单独做。
- **只换类型、不补默认值，直接 `[[cls alloc] initWithFrame:]`** — 代码最少。但标题会是 "Button"（两个展开开关和列表翻页按钮原来是空标题），首行 ‹ › 和在线预览走 `[super drawRect:]`，换行方式、图片缩放和边框样式的差别可能改变它们的文字和图标排布。只为省一两行赋值不值得冒外观变化的风险。

## Consequences

- **收益**：每个候选窗按钮少一次按标题的尺寸计算。在无窗口的进程里实测，`buttonWithTitle` 建一个按钮约 0.5–0.6 ms，工厂约 0.05–0.11 ms；一页 6 行加首行和翻页按钮，每键少建约 8–10 次。真实候选窗里的 `build_ms` 改善要以设备上同一套自动打字的对比为准。
- **代价与已知上限**：工厂对齐的是当前 AppKit 下 `buttonWithTitle` 的默认值，其中边框样式这一项依赖未写进文档的行为；以后 AppKit 改了 `buttonWithTitle` 的默认值，`candidate-button-factory` 会报出差在哪一项，那时按实测改工厂。新加到候选窗里的按钮类若没有重写 `-drawRect:`，`buttonWithTitle` 给它的边框样式是 automatic，要把这个类加进测试并按结果决定工厂是否分情况设置。

## Verification

- `platforms/macos/tests/candidate/CandidateButtonFactoryTest.mm`（CTest `candidate-button-factory`）对三个候选按钮类、空串和长标题在内的五种标题，分别用 `buttonWithTitle` 和工厂建按钮，逐项比较标题、边框、对齐、字体、换行与截断、图片位置与缩放、高亮方式、控件尺寸、动作触发掩码和 `intrinsicContentSize` 等属性，并断言工厂建出的按钮 frame 为零。
- `InputController.mm` 里 `buttonWithTitle` 只剩会话恢复窗口（「中文输入暂不可用」）的三个按钮，它不在每键路径上。
