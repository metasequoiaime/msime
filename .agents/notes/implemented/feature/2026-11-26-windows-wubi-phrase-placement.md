# Agent Note: Windows 原生设置补「五笔自造词位置」入口

Status: implemented

## Problem

#11840（五笔按输入习惯自动造词）加了设置键 `input.wubi_phrase_placement`（`bottom` / `top`），决定新造的五笔词入库时排在同码已有词之后（默认，等于现状）还是之前。偏好、账号同步和引擎的 `SessionOptions` 管线都已接好，但按 AGENTS.md「`packages/ui` 只修缺陷，不加新设置」，共享 React 设置页里的下拉框被撤掉，六个平台都没有入口能改它（#11863）。

Windows 侧有两个设置宿主：现代 WinUI 窗口（`platforms/windows/settings/main.cpp`）和 Win7 兼容窗口（`platforms/windows/settings/win7/`）。两者都是原生实现、都已有「学习选词习惯」开关，都要补这一行。

约束：

- 只在五笔方案、并且学习选词习惯开启时出现：造词本身受学习开关控制，关掉学习时这一项不会被用到。
- 与「学习选词习惯」同组（共享页「候选与联想」组、学习开关之后），不是方案页的选项。
- 取值、默认值和显示规则以共享 Rust 为准，不在宿主另写一套：`bottom` 是默认且序列化时省略，不认识的取值按 `bottom` 读。
- 不改任何出厂默认值：默认「同码词后」等于这个字段出现之前的行为。

## Decision

两个 Windows 宿主都从 `InputSettingsModel.h` 的同一张 `wubi_phrase_placements` 表渲染这一行，取值和显示谓词也在那里（`wubi_phrase_placement_id`、`wubi_phrase_placement_index`、`wubi_phrase_placement_offered`），与 Win7 共用的 `tests/ui/input_settings_model.cpp` 核对顺序、文字、回退和显示条件。

WinUI（`main.cpp` 的 `build_input_habits`，`candidates` 标签）：在「学习选词习惯」的 `bool_row` 之后，`wubi_phrase_placement_offered(scheme == "wubi", learning)` 为真时画一行 `select_control`，自动化 ID `wubi-phrase-placement`；改动写 `wubi_phrase_placement`，`redraw=false`。学习开关的 `bool_row` 本来就是 `redraw=true`，切换学习会重画整页，这一行随之出现或消失；换方案同样会重画。

Win7（`SettingsWindow.cpp` 的 `candidates_controls()`）：`candidate_checks` 里的 `learn_candidates` 之后插入标题、`?` 和下拉框，选项复用同一张表。这一页原先没有逐行显隐，所以给 `Item` 加了 `shown` 谓词和 `block` 高度：`layout_body()` 里同一页面内隐藏行的 `block` 累计成 `trim`，从后面各行的 `top` 里扣掉；页面总高度用 `hidden_height(surface)` 减去同一份高度，滚动范围不会留白。`refresh_candidates()` 按存储值选中，`enable()` 在隐藏时也保持禁用（避免键盘焦点落到看不见的控件上），`command()` 选哪一项都照实写回，`valid()` 只接受模型里的两档。`PreferencesStore` 的 `optional_style` 放行这个键，默认值第一次改成「置顶」时才补得上叶字段。

行文案：标题「自造词位置」，说明进标题旁的「?」（新造的五笔词排在同码已有词之后（默认，等于现状）还是之前；只管入库这一次，之后照常参与调频），选项「同码词后」「同码词前」。这条界面规则沿用全平台「设置项说明收进 ?」。

## Alternatives considered

- **放进 Win7 方案页的「五笔方案」组**（先按这个方向写了一半）：方案页本来就有一片五笔专属行，放在那里离码表版本近。否掉理由：issue 要求与「学习」开关同组，学习开关在「候选与联想」面板；共享设计也把它放在候选与联想组的学习开关之后。放到方案页会让用户在两个页面之间来回找，而且学习关掉时在方案页隐藏又得给方案页再加一套显隐规则。
- **只在 WinUI 做，Win7 兼容窗口不做**：理由是 Win7 用户少、`candidates_controls` 的布局没有逐行显隐，改动比 WinUI 大。否掉理由：issue 明确写了「含 win7 版设置窗口」，两个宿主共用同一份偏好，缺一个就是同一台机器上换窗口看到不同设置。
- **常显但灰掉**：不用新增显隐机制，实现最省。否掉理由：issue 验收要求「非五笔方案或学习关闭时不显示」，灰着一行还会让人以为改一改就能用。
- **回到共享 React 设置页加控件**：一处改动覆盖 macOS/Linux/HarmonyOS 的 Tauri 设置。否掉理由：AGENTS.md「`packages/ui` 只修缺陷，不加新设置」，界面在迁原生；#11863 给 macOS/iOS/Android/HarmonyOS/Linux 的路径也都写了「随原生迁移」或各自的原生设置目录。

## Testing

- `tests/ui/input_settings_model.cpp`（macOS 上可直接编译运行）钉住表的顺序与文字、`top`/`bottom` 的取值、缺省和不认识的取值回退到 `bottom`、以及只在「五笔 + 学习开启」时显示。
- `tests/ui/windows7_settings.cpp`（只在 Windows 上跑）：在五笔方案下预置学习开启和 `top`，重载后切到「候选与联想」确认这一行可见、选中「同码词前」，选回「同码词后」确认文档里不留下这个键；关掉学习后确认行和「?」一起隐藏、控件禁用、后面的行上移；再还原学习。
- `tests/tools/settings_ui.ps1`（Windows 上跑真实 WinUI 窗口）：切到「候选与联想」，确认默认不写进偏好，展开下拉核对两项文字并截图，选「同码词前」读回 `top`，再关掉、开回学习核对这一行跟着消失和出现。
- 本机没有 Windows 主机和游戏客户端，WinUI/Win7 的界面截图和运行验收只能由 Windows 上的 CI（`Windows native settings`）完成；本机用 MinGW 交叉编译对 `SettingsWindow.cpp`、`windows7_settings.cpp` 做了 `-fsyntax-only` 语法检查，`InputSettingsModel.h` 的测试直接编译运行通过。

## Consequences

收益：Windows 两个设置窗口都能改这个选项，默认值等于现状，老用户的手感不变；选项表只有一份，WinUI、Win7 和测试不会各说各的。

代价与风险：

- Win7 布局引擎多了一套「按页面内谓词隐藏整行、按 `block` 扣高度」的机制。目前只有这一行用它；以后再加显隐行时要保证 `block` 与 `top` 的推进量一致，否则页面高度和后面行的位置会对不上。
- Win7 的下拉框在隐藏时被禁用而不是销毁，`refresh_candidates` 在不可见页面也会写选中值；这是既有做法，但隐藏行的可访问性状态依赖 `ShowWindow` 一并收掉「?」按钮。
- 只在 Windows 落地，issue 其余平台（macOS、Android、iOS、HarmonyOS、Linux）仍缺入口，PR 写 `Refs #11863` 并在正文列明。
