# Agent Note: Windows 选中行释义按选中底色换色并保证 4.5:1

Status: implemented

## Problem

Windows 候选窗选中行的释义和读音对选中底色的对比度太低，[Windows 候选读音单独一行](2026-11-01-windows-candidate-pronunciation-own-line.md)（#10461）把它记成后续项。`CandidatePalette.h` 的 `candidate_row_translation_color` 在选中行只认皮肤写的 `selected_translation`，没写时画皮肤包的释义色，再没有时画选中序号色 `selected_number`：

- 皮肤包的释义色是给卡片底色配的。选中底色不透明又和释义色同一色系时（`selected = "#4D6BFE"`、`translation = "#2F5BD3"`），选中行是蓝底蓝字，约 1.4:1。macOS 已经用 `SkinTokens::selectedTranslation` 修过（[macOS 候选读音单独一行，选中行释义按底色换色](2026-10-16-macos-candidate-pronunciation-own-line.md)），Windows 没有。
- 默认皮肤也不够：选中序号色是白色 0.82，在浅色的 `#2C7A4B` 上约 4.1:1，在深色的 `#5FBF84` 上约 2.0:1，CI 的 `windows-skin-gallery` 截图 `no-skin__horizontal-dark.png` 里选中行的 `input /ˈɪnpʊt/` 几乎看不清。读音在不透明选中底色上不减淡，和释义同色，所以同样读不清。

## Decision

选中行的释义色（读音同色）按下面的顺序取，规则都在 `candidate_row_translation_color`，绘制处 `CandidateWindow.cpp` 不变：

1. 皮肤写了 `selected_translation`（`apply_candidate_skin_extra_slots` 读进来的），照它画，不推导，和 macOS 相同。
2. 选中底色不透明（alpha ≥ 0.85，和 `candidate_selected_foreground`、macOS `ContrastingText` 同一门槛）时：
   - 有皮肤包的释义色：换成按底色推导的颜色 `candidate_selected_gloss_color`，和 macOS 的 `DeriveSelectedForegrounds` 一样，包的释义色只留给未选中行。
   - 没有皮肤包的释义色：仍画选中序号色；它对底色不到 4.5:1 时换成推导的颜色。
3. 选中底色半透明时下面透出的是卡片底色，照旧画包的释义色，没有时画选中序号色。

`candidate_selected_gloss_color` 按底色挑白或近黑 `#1A1A1A`，挑法和 Windows 推导选中文字的 `candidate_selected_foreground` 相同（比较两者对底色的对比度，取高的）；不透明度从和选中序号一样的 0.82 起，对底色还不到 4.5:1（WCAG AA 正文）时按 0.01 往上加，到够了或者到不透明为止。对比度由 `candidate_contrast_on` 按释义色自己的 alpha 合成到底色上再算。

结果：默认浅色的选中行释义是白色 0.89，约 4.5:1；默认深色是 `#1A1A1A` 0.82，约 5.5:1。未选中行一律不变。

## 和 macOS 的差别

- macOS 的推导固定 0.82，只保证约 3:1；Windows 往上加到 4.5:1。
- macOS 只在皮肤包有释义色时推导，默认皮肤的选中行仍是白色 0.82 的选中序号色；Windows 在选中序号色不到 4.5:1 时也推导，所以默认皮肤会换色。macOS 的默认皮肤这次没动。
- macOS 用亮度阈值 0.45 挑黑白，`#5FBF84`（亮度约 0.41）会挑白；Windows 沿用自己 `candidate_selected_foreground` 的对比度比较，挑 `#1A1A1A`。

## Alternatives considered

- **照搬 macOS，只在皮肤包有释义色时推导，固定 0.82** — 和 macOS 完全一致、默认皮肤的外观不变。但默认皮肤正是用户最常看到的，深色约 2.0:1、浅色约 4.1:1 都不到 AA，这次要修的低对比度在默认皮肤上原样留着。
- **默认深色也用白色，只把不透明度提到 1** — 和选中行的白色候选文字一致。但白色在 `#5FBF84` 上最多约 2.3:1，怎么调不透明度都到不了 4.5:1。
- **把默认深色选中行的候选文字和序号也一起换成近黑** — 一行里颜色统一。但那改的是选中文字的设计（macOS 和 Windows 的默认深色都是绿底白字），不在这次「释义和读音」的范围里，要单独由产品决定。
- **把推导结果写进 `CandidatePalette::selected_translation`** — 绘制处直接读字段。但这个字段现在的意思是「皮肤明确写了」，调色板又分两步合成（`candidate_palette` 再 `apply_candidate_skin_extra_slots`，取色器能让包的选中槽位不生效），在哪一步写都要再判断一次；在取色时推导只有一处。

## Consequences

- **收益**：默认浅色、深色和不透明选中底色的皮肤，选中行的释义和读音对底色至少 4.5:1（底色本身让白和近黑都到不了时取不透明的那一个，例如 `#4D6BFE` 上的白色约 4.3:1，和选中文字一样）；释义色和选中底色同一色系的皮肤不再蓝底蓝字。
- **代价**：默认深色的选中行里候选文字和序号是白色、释义和读音是近黑，同一格两种前景色。选中文字本身（白色在 `#5FBF84` 上约 2.3:1）同样不到 AA，这次没动，要不要改是产品决定。皮肤写了可读的 `selected_number`、没写释义色时照旧用它；写了读不清的 `selected_number` 时释义不再跟着它。

## Verification

- `tests/ui/candidate_palette.cpp`：默认浅色、深色的选中行释义对底色至少 4.5:1、不透明度不低于 0.82，浅色仍是白色、深色是 `#1A1A1A` 0.82，未选中行和固定排位行不变；可读的选中序号色照用；`#23262F` 配 `#6B7180`、`#F2C94C` 配 `#B8860B` 的包到 4.5:1，`#4D6BFE` 配 `#2F5BD3` 取不透明白色；半透明选中底色保留包的释义色；写了 `selected_translation` 照它画。原来断言选中行画 `selected_number` 和包释义色的两处改为推导色（其中 `selected_number` 与选中底色同为 `#5FBF84`，原来就是看不见的）。
- `tests/ui/candidate_skin.cpp`：选中取色器让包的 `selected_translation` 不生效时，选中行释义按底色推导、至少 4.5:1，未选中行不变。
- 两项都在 macOS 上用 `clang++ -std=c++17 -Wall -Wextra -Werror` 编译运行通过；Windows 构建和 `windows-skin-gallery` 截图交给 PR 的 CI。
