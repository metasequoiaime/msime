# Agent Note: 默认皮肤的选中行达到 WCAG AA

Status: implemented

## Problem

[Windows 选中行释义跟随选中文字换色](2026-11-01-windows-selected-gloss-contrast.md)（#10494）把选中行的释义和读音换成跟着选中文字走，但没动默认皮肤的选中底色，留下一个待维护者决定的产品问题：默认深色的选中底是强调色 `#5FBF84`，上面的白字只有约 2.3:1，到不了 WCAG AA 正文的 4.5:1。逐个平台量过「跟随系统」默认皮肤的选中行（选中文字、没有皮肤释义色时的释义，读音在选中行上和释义同色）：

| 平台 | 明暗 | 选中底 | 选中文字 | 释义 |
| --- | --- | --- | --- | --- |
| macOS（非玻璃：macOS 26 以前，或打开了「降低透明度」） | 浅色 | `#2C7A4B` | 白色 5.3:1 | 白色 0.82，4.1:1 |
| macOS（非玻璃） | 深色 | `#5FBF84` | 白色 2.3:1 | 白色 0.82，2.0:1 |
| macOS 26 Liquid Glass | 浅色 | `#2C7A4B` | 白色 5.3:1 | 白色 5.3:1 |
| macOS 26 Liquid Glass | 深色 | `#5FBF84` | `#102419` 7.2:1 | `#1D3327` 6.0:1 |
| Windows（#10494 之后） | 浅色 | `#2C7A4B` | 白色 5.3:1 | 白色 0.89，4.5:1 |
| Windows（#10494 之后） | 深色 | `#5FBF84` | 白色 2.3:1 | 白色，2.3:1 |
| Linux 原生画布、classicui、IBus | 浅色、深色 | `#3584E4` | 白色 3.8:1 | 白色 0.85，3.2:1 |
| HarmonyOS 2in1 候选窗 | 浅色 | `#2C7A4B` 13% 淡底 | `#2C7A4B` 4.4:1 | `#182431` 0.6，4.0:1 |
| HarmonyOS 2in1 候选窗 | 深色 | `#5FBF84` 25% 淡底 | `#5FBF84` 4.1:1 | 白色 0.6，4.5:1 |

iOS、Android 和 HarmonyOS 的触屏候选条不画选中底：首选是画在键帽或键盘底色上的强调色文字，默认配色来自季节应用主题（`app_theme.rs` 的种子色），不是 `#5FBF84` 底配白字，不在这次范围里。

## Decision

只改到不了 AA 的默认值，强调色本身不变（预编辑文字、选中竖条、键位图高亮这些不承载选中行正文的地方照旧用它）。桌面三端是实心选中底，加深底色、保留白字；HarmonyOS 2in1 是淡底配强调色字，保留淡底、加深选中文字（#10527 之后的单独 PR）。

- **macOS、Windows 深色**：选中底从强调色 `#5FBF84` 换成品牌的强调墨绿 `#17783F`（msime-web `src/styles/tokens.css` 的浅色 `--accent-ink`），白字约 5.5:1。浅色的 `#2C7A4B` 白字已经 5.3:1，不动。macOS 在 `NativeCandidateTokens` 里把深色的 `selected` 写成 `#17783F`，`ResolveSkin` 只在主题或取色器给了强调色时才让选中底跟着强调色，强调色也留给平台时保留原生选中底；Windows 是 `CandidatePalette::selected`，`candidate_palette` 本来就只在给了强调色时跟随。
- **macOS Liquid Glass 不跟着换**：玻璃底板上的深色选中行是浅绿底配深色字，本来就到 7.2:1，`ApplyNativeGlassCandidateContrast` 显式保留 `#5FBF84`。
- **选中行释义（读音同色）**：`#17783F`、`#2C7A4B` 上白色 0.82 的选中序号色只有约 4.3:1、4.1:1。Windows 沿用 #10494 的规则（`candidate_row_translation_color`：序号色不到 4.5:1 时取选中文字色，不透明度从 0.82 往上加到够为止），默认浅色是白色 0.89、深色是白色 0.86。macOS 新增同一规则的 `SelectedRowGloss`，用在皮肤没有释义色的选中行上（`InputController.mm` 的 `translationColor`）；皮肤有释义色时仍走 `DeriveSelectedForegrounds` 的 0.82，不在这次范围里。
- **Linux**：选中底从 GNOME 强调色 `#3584E4` 换成同一色系更深的 Adwaita 调色板 Blue 5 `#1A5FB4`（`kLinuxSelection`），白字约 6.3:1，85% 白的序号约 5.0:1；选中行没有皮肤释义色时释义画序号色，所以释义和读音也是 5.0:1。`kLinuxAccent` 仍是 `#3584E4`。悬浮工具栏的勾本来就取候选窗的选中底和选中文字（`floating_toolbar_colors`），跟着变成 `#1A5FB4` 配白字；双拼键位图的高亮键仍填 `kLinuxAccent`。
- **共享设置页的候选预览**：macOS、Windows 两块默认值的 `--cand-selected` 按明暗写成 `#2C7A4B` 和 `#17783F`，调色板给了强调色、没给选中底时经 `candidatePaletteStyle` 写出的 `--cand-mac-selected` 跟随强调色，和原生 `ResolveSkin` 一致；`candidate-preview-native-tokens.test.ts` 照旧对照原生源码。

- **HarmonyOS 2in1 候选窗**：淡底（`#2C7A4B` 13%、`#5FBF84` 25%）不变，`GlobalTheme.ts` 的原生候选 token 里选中文字换成品牌的强调墨绿，浅色 `#17783F`（msime-web 浅色 `--accent-ink`）约 4.6:1，深色 `#8FE59C`（深色 `--accent-ink`）约 6.2:1。浅色的选中序号从 `#182431` 60% 提到 70%（`#B3182431`），约 5.4:1。2in1 选中行的释义、注释和编码提示原来画次要色（浅色 4.0:1），改成 `GlobalTheme.candidateDetail`：次要色就是序号色（没有皮肤包自己的释义色）时选中行取选中序号色，和 macOS 把设计稿的 `candSelTr` 画成选中序号色一样；读音和释义同色。触屏候选条不走这条规则。

### 对比度测试

- macOS `CandidateSkinTest.cpp`：两种明暗的默认 token，选中文字和 `SelectedRowGloss` 对选中底都不低于 4.5:1，序号色本身低于 4.5:1（所以释义确实被加深过），玻璃两种明暗的释义就是选中序号色；`ShortcutTest.mm` 在真实候选窗上检查跟随系统的选中行文字和释义。
- Windows `candidate_palette.cpp`：两种明暗的默认调色板，选中文字和选中行释义不低于 4.5:1，释义是白色、不透明度在 0.82 和 1 之间；另用白对黑 21:1、白对 `#5FBF84` 约 2.27:1、白对 `#17783F` 约 5.53:1 校准对比度算法。
- Linux `candidate_palette.cpp`：两种明暗的原生调色板，选中底是 `kLinuxSelection`，选中文字和序号不低于 4.5:1，`kLinuxAccent` 上的白字低于 4.5:1。
- 共享预览：默认选中文字对两种明暗的默认选中底不低于 4.5:1。
- HarmonyOS `keyboard-logic.test.ts`：两种明暗的 2in1 原生 token，选中文字和选中行释义合成到淡底上不低于 4.5:1，强调色本身低于 4.5:1，未选中行仍是次要色，皮肤包自己的释义色在选中行照用；`tests/run.sh` 的接线检查改成带 `candidate.highlighted` 的调用。

### Windows 的取色测试在 CI 上执行

#10494 记下的缺口：`windows-candidate-palette` 和 `windows-candidate-skin` 在 Windows GNU 交叉编译作业里只编译不运行。两者只依赖头文件和 nlohmann_json，`windows-candidate-skin` 挪到 `platforms/windows/CMakeLists.txt` 的 pipe-only `return()` 之前（`windows-candidate-palette` 本来就在那里），`ci-platforms.yml` 的 `windows-scripts` 作业在状态目录那一步之后，复用 x64 的 pipe-only 构建树，用 MSVC 编译并运行这两项。不加新依赖，也不新开 runner。

### 截图

`windows-candidate-skin-gallery` 的截图只作为 CI 产物上传，没有像素基线，不需要更新期望；`no-skin__*-dark.png` 的选中行在这次之后应是 `#17783F` 底。

## Alternatives considered

- **深色选中行换成近黑文字，选中底仍是 `#5FBF84`** — 不改品牌绿的亮度，`#102419` 这样的深色字能到 7:1 以上，macOS 的玻璃底板就是这样做的。不采用：非玻璃的卡片是不透明的深灰，亮绿底配黑字在深色卡片里非常扎眼，和浅色模式的绿底白字也不是同一套设计；维护者批准的方向是加深选中底、保留白字。
- **深色也用浅色的 `#2C7A4B`** — 两种明暗同一个选中底，Windows 设置窗口里白字填充就是这样做的。不采用：维护者批准的是品牌的强调墨绿 `#17783F`，白字 5.5:1 也比 `#2C7A4B` 的 5.3:1 略高；浅色本来就达标，不改。
- **把强调色本身换成 `#17783F`** — 改一个值，选中底照旧跟着强调色。不采用：macOS 的预编辑文字、选中竖条和候选窗外的很多地方画的是强调色，`#17783F` 画在深灰卡片 `#28282A` 上只有约 2.7:1（`#5FBF84` 是 6.5:1），会让本来达标的地方不达标。
- **Linux 也换成品牌绿 `#17783F`** — 三端桌面同一个选中底。不采用：Linux 的默认值取的是设计稿里的 Adwaita token，蓝色选中行是这个平台的一致外观；加深到同一色系里的 Blue 5 就够 AA，不必改色相。
- **macOS 有皮肤释义色时也改成加深到 4.5:1** — 和 Windows 完全一致。不采用：那是皮肤包的配色，不是默认皮肤；这次只处理默认皮肤，皮肤包的推导规则留给 #10494 那篇记下的差别。

## Consequences

- **收益**：macOS（非玻璃）、Windows、Linux 的默认选中行，候选文字、释义和读音在两种明暗下都不低于 4.5:1；新的对比度断言让以后改默认色时退化会在单测里失败，Windows 的两项取色测试第一次在 CI 上真正运行。
- **代价**：深色模式的选中行比原来暗，和强调色不再是同一个绿；macOS 玻璃和非玻璃的深色选中行颜色不同（玻璃 `#5FBF84` 配深色字，非玻璃 `#17783F` 配白字）。Linux 的选中行不再是 GNOME 强调色。默认选中行的序号仍是白色 0.82（`#17783F` 上约 4.3:1，`#2C7A4B` 上约 4.1:1），这次只保证候选文字和释义、读音。
- **未覆盖**：触屏候选条不在范围里；HarmonyOS 2in1 未选中行的次要色在浅色白卡片上约 4.2:1，不是选中行，这次不动。HarmonyOS 的改动只在 node 下的逻辑测试和 CI 的编译里验证，没有在 2in1 设备或模拟器上看过。macOS 的改动在 CI 和本机单测上验证，Windows 和 Linux 的画面以 CI 的截图产物和单测为准，没有在真机上看过。
