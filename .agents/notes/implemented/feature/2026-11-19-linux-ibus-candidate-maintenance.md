# Agent Note: iBus 候选维护快捷键与右键自绘菜单

Status: implemented

## Problem

五笔/拼音自造词会往个人词库写低权重新词（沉底），要维护这些词就需要删除、置顶、固定排位。Windows 早已有逐候选右键菜单（`CandidateMenuLayout.h`）和对应快捷键；Linux iBus 面板没有逐候选菜单接口，候选维护能力不齐。本仓的 iBus 实现此前只有 Ctrl+Shift+Alt+1..8 删除高亮候选（计划文档里误记为缺失）。

## Decision

从五笔造词 feature 分支拆出独立的 Linux 配套 PR（本分支），基于 develop 只带三个代码提交：

- **快捷键（子线 C）**：Ctrl+Delete 删除高亮候选、Ctrl+↑ 置顶、Ctrl+1..5 固定到第 1..5 位、Ctrl+0 取消固定（主键盘与小键盘等价）。解析抽成 `CandidateMaintenanceShortcut.h` 纯函数：修饰键恰为 Control，不抢 Ctrl+BackSpace / Ctrl+左右的分段编辑；动作按 `rendered_candidates` 里 `highlighted` 行的 identity 落库，围栏与面板属性菜单同一组（会话、渲染代次、`candidate_dictionary_actions_available`，删除另有可删除判定）；高亮行不在本页时按键照常往下走。
- **右键菜单（子线 D）**：IBus 面板没有逐候选菜单接口，自绘 X11 弹层（`OverlayX11Window` + `XGrabPointer`，点外收起、hover 重画、事件泵；Xwayland 会话同样可用）。条目文案与顺序照 Windows `CandidateMenuLayout.h`（置顶 / 固定 1..5 / 分隔线 / 取消固定 / 删除），布局、命中、绘制与悬浮工具栏菜单共用同一套模板化函数（条目类型同构，属纯重构）。动作按被点渲染行的 identity 在 show 时绑定，回调核对会话与渲染代次；连不上 X 时退回候选窗提示条。收起时机统一：render 开头、clear（focus_out 与会话出口）、reset、Esc（只收菜单不取消组字）。
  - **定位按会话分流（本轮修正）**：原实现右键一律用 `XQueryPointer` 取根窗口指针位置定位；实机发现 Wayland 会话下菜单稳定出现在桌面右上角。根因是 Xwayland 根窗口根本不跟踪被 Wayland 合成器（GNOME Shell）管理的指针——`XQueryPointer` 连续采样恒返回陈旧值 `(1901,66)`、`child=0x0`，菜单据此展开后被夹到工作区右上角。现在纯 X11 会话仍用指针模式（坐标可信，`show()` 内 `XQueryPointer` 兜底）；Wayland 会话改用 IBus 上报的 `engine->cursor_area` 作光标锚点。锚点守卫在 `CandidateMenuPlacement.h` 纯函数里：四分量全 0（客户端尚未报光标）或矩形不完整落在工作区内（Wayland 原生客户端报的是表面局部坐标，不是根坐标）都判不可用，回退到工作区水平居中、下沿离底边四分之一处，绝不落回右上角。可用时光标模式优先放光标右方，放不下依次翻左方、下方（留出候选窗估计高度）、上方，全程夹在工作区。宿主拿不到 GNOME Shell 画的候选窗矩形，只能按光标矩形 + 字号估计，无法精确到「所选行」。`ClientEngine.cpp:6937` 既有判断（Wayland 下 IBus 分不出客户端显示类型、光标坐标未必是根坐标）由「完全不用」细化为「用 + 守卫 + 回退」，不无条件信任。

原先这些内容写在五笔造词的决策笔记 `2026-11-15-wubi-run-phrase-learning.md` 里。本 PR 独立成篇：造词 PR 不含 Linux 代码，iBus 段落随代码一起归属本 PR；造词笔记恢复为不含 iBus 段落的版本。

## Consequences

- 新增两个纯函数单测：`linux-candidate-maintenance-shortcut`（四键解析、小键盘、修饰组合排除、不抢既有组合键）与 `linux-candidate-menu-items`（文案顺序、勾选、可用性），由 Linux ctest 覆盖。
- 新增 `CandidateMenuPlacement.h` 纯函数与 `linux-floating-panels-model` 中的定位单测：光标右方、贴右边缘翻左方、全 0/工作区外回退到中下部（断言不落右上角）、指针模式保持原「下方放不下翻上方」。定位从 `CandidateMenuSurface::show()` 的 Xlib 分支抽到纯函数，Xlib 层只负责取工作区与缩放。
- 自绘弹层依赖 X11；纯 Wayland 会话下退回候选窗提示条，属已知边界，layer-shell 原生菜单留作后续增强。
- 与造词 PR 无代码依赖：本 PR 只碰 `platforms/linux/`，独立验证、独立 PR。

## Verification

- 本机 Linux 工具链可用（CMake 4.3.4、GCC 15.3.0、ibus-1.0 1.5.34、nlohmann_json 3.11.3）。手工打出 `msime-linux_0.17.0_amd64.deb`：源码取本分支，编译产物（host-api、MCP、桌面壳、词库/模型）复用主 worktree 已验证的那套（本 PR 不碰 `crates/`，host-api ABI 不变）。`-Wall -Wextra -Werror` 编译 `msime-ibus-host`（含改动的 `ClientEngine.cpp` 与新增的 `CandidateMenuSurface.cpp`）及全部原生测试通过。
- 全量 `ctest`：124 项中 122 项通过。两项 `linux-candidate-native-canvas`（`/...-chrome`）失败是本机字体渲染环境的既有问题——在不含本 PR 改动的基线上以相同断言、相同行号失败（`auxiliary_ink > 10`、`colors(...)[0x808080] > 10`），与本 PR 无关；打包时排除这两项后 122/122 通过，deb 后处理去掉了 Fcitx5 库依赖（`ibus | fcitx5` 保留）。
- `ctest -R 'linux-candidate-(maintenance-shortcut|menu-items)$'`：2/2 通过。前者覆盖四键解析、小键盘、修饰组合排除、不抢既有组合键；后者覆盖文案顺序、勾选与可用性。
- **实机验证（新装 deb）**：`pkexec dpkg -i` 装上 0.17.0（系统版本 0.16.1 → 0.17.0），在独立 D-Bus session、全新 `state_root`、新装 `/usr/bin/msime-linux-ibus` 下，用 gi IBus 真实打字出候选并逐键验证：Ctrl+↑ 置顶、Ctrl+1 固定第 1 位（候选出现「固定1」标记）、Ctrl+0 取消固定（标记消失）、Ctrl+Delete 删除高亮候选，四键 `process_key_event` 均返回 TRUE（被消费）且候选正确重绘。删除的权威性用数据层对照坐实：删除前用户词库有合成词「测试词」词条（`source=user`），删除后查询为空；重绘后仍出现的同文本「测试词」是语言模型同音候选，不是用户词条。另确认被污染词库（预置「固定3」）下动作会按设计围栏拒绝，说明复验必须用全新 `state_root`。测试词为合成值，全程写临时词库，不碰用户词库。
- **菜单定位实机验证（本轮修正）**：本机是 GNOME Wayland 会话（`WAYLAND_DISPLAY=wayland-0`），先只读实证根因——`XQueryPointer` 在 Xwayland 根窗口连续 5 次恒返回 `(1901,66)`、`child=0x0`。随后起独立 IBus daemon（不碰系统 daemon），直接运行新编译的引擎二进制（`readlink /proc/<pid>/exe` 核对确为新构建），全新 `state_root`，用 gi IBus 把光标设到屏幕中央 `(600,400,8,20)`、五笔编码 `aa` 打出候选，经引擎 DBus `CandidateClicked(0,3,0)` 触发右键。`xwininfo -root -tree` 读到菜单窗口 `msime-candidate-menu` 左上角为 `(620,392)`——正是光标右方，不再落在桌面右上角。改动后全量 `ctest` 113/113 通过（本构建目录下两项字体渲染测试也通过，非先前基线失败的那套）。测试词为合成值，全程用临时 state_root，不碰用户词库。
- **系统 deb 安装路径复验**：另把新编译引擎、PR B 的 host-api、注入 0.17.0 的 desktop 壳一起打进 `msime-linux_0.17.0_amd64_verify-menu.deb`（仅本机验证包，不进 PR），`pkexec dpkg -i` 覆盖安装。包内引擎 SHA `3ad9738b...` 与新编译 strip 后逐字节一致，装完 `/usr/bin/msime-linux-ibus` 的 SHA 与包内一致。再起独立 daemon，让系统 component 经 `/usr/bin/msime-linux-ibus-launcher` + `MSIME_IBUS_OPTIONS`（临时 state_root）启动引擎，`/proc/<pid>/exe` 确认为 `/usr/bin/msime-linux-ibus`（即 deb 装出来的那份，不是 worktree 二进制），同样光标 `(600,400,8,20)` + DBus 右键，读到菜单 `142x261+620+392`，`INSTALLED_MENU_POSITION_VERIFY_OK`。这一步补上「直接跑二进制」绕过的 deb 打包与安装路径缺口。
- 自绘 X11 弹层的点击、hover、收起交互仍未在真实编辑器里做完整实机验收；菜单定位已按上文实测，但不把纯函数单测和这次定位验证当作整个弹层的前端验收，交 PR 上的 Linux native job 与后续真机复验。
- `git diff --check` 通过。`pnpm run verify-notes` 因本机缺 node 未运行；笔记格式已按校验脚本的约束手工核对。

## Alternatives considered

- 给 IBus 面板加逐候选属性菜单入口——IBus 属性菜单是全局菜单，动态逐候选绑定要维护额外的属性树且与桌面面板渲染耦合；自绘弹层照搬已有的工具栏菜单栈更省。
- Wayland layer-shell 原生菜单——X11 弹层在 Xwayland 下就够用，layer-shell 留作后续增强，不影响本次验收。
- 定位方案（本轮修正）：光标锚定 + 守卫。否掉的两条——(1) Wayland 下把菜单固定放在工作区中上部：最稳、绝不会跑到角落，但完全不跟随输入光标，用户明确要靠近候选行；仅在锚点守卫判不可用时作为回退形态使用。(2) 改成 IBus 原生动作列表、由 GNOME Shell 画、数字键触发：能贴近候选框，但要改交互方式、属架构级改动，超出本次「修右键菜单位置」的范围。选光标锚定是因为候选框通常紧贴光标，侧方摆放不挡候选，是宿主拿不到候选窗矩形时最接近期望的可行解；代价是 `engine->cursor_area` 在 Wayland 下可能是表面局部坐标，靠「必须完整落在工作区内」的守卫拦截，拦不住的边界情况仍可能摆偏，故保留工作区中下部回退。
