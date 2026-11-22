# Agent Note: #11620 Wayland 候选框越界调查

Status: proposed

## Problem

#11620 报告 Deepin 25、DDE Wayland 和 Fcitx5 下输入框靠近屏幕边缘时候选框超出屏幕（水杉 v0.17.0）。维护者在 issue 里请报告者补充诊断后，报告者 11-20 贴出了完整的 `fcitx5-diagnose`，其中可确认：会话是 Wayland，合成器是 treeland（`WAYLAND_DISPLAY='treeland.socket'`）；Fcitx 5.1.12；采集时唯一聚焦的输入上下文是 deepin-terminal，走 `frontend:wayland_v2`，Firefox、钉钉等程序同时有 `frontend:wayland_v2` 和 `frontend:dbus` 两类输入上下文；Qt6 的 `QT_IM_MODULES="wayland;fcitx"`，实际模块是 `QtWaylandClient::QWaylandInputContext`（text-input），Qt5 和 GTK 走 fcitx 模块；已启用的用户界面插件有 Classic User Interface、KDE Input Method Panel 和 DBus Virtual Keyboard 三个，诊断没有说明当时实际由哪一个绘制候选；诊断还报告找不到 `/usr/lib/x86_64-linux-gnu/fcitx5/libmsime-fcitx5`，同时插件列表里 MSIME 处于启用状态；安装包里的 `msime.conf` 本来就把 `Library=` 写成不带 `.so` 后缀的绝对路径（见 `platforms/linux/fcitx5/CMakeLists.txt`），这条提示可能只是诊断脚本按字面路径查找造成的误报，但尚未在报告者环境里核实插件实际加载自哪里。诊断仍未覆盖显示缩放比例、截图所在的具体应用，以及切到 Classic UI 后是否仍越界。

仓库的 Fcitx5 Wayland 路径只更新候选内容和 `InputPanel`，候选窗由 Fcitx5 的用户界面绘制；水杉的原生画布 `NativeCanvasX11` 只在 X11 会话（以及 Wayland 下走 XWayland、使用 canvas 皮肤包的程序）启用，并自行把窗口夹在工作区内，不能解释原生 Wayland 客户端里的越界。`frontend:wayland_v2` 用的是 input-method-v2 协议，弹出面由合成器相对文本光标矩形放置，因此 treeland 的放置逻辑也是候选责任方之一。截至 2026-11-22，没有找到重复 issue，也没有引用 #11620 的已合并修复 PR。

## Proposal

维护者决定把本笔记作为 #11620 的跟踪记录合入 develop，本 PR 不含产品代码改动。后续先向报告者补齐剩余字段（显示缩放、截图应用及其走的前端、实际生效的用户界面插件、Classic UI 对照，并核实 MSIME 插件的实际加载路径），再在最小合成编辑器里确认候选窗由 Fcitx5 Classic UI、kimpanel、treeland 还是水杉路径定位。只有确认水杉拥有可修改的定位边界后，才另开写明 `Fixes #11620` 的 PR 加入行为修复与测试；若责任在 Fcitx5 或 treeland，改为向上游报告并在 issue 中说明。

## Alternatives considered

- 直接修改 X11 `NativeCanvasX11` 的工作区夹紧：报告中的原生 Wayland 客户端不经过该窗口，无法覆盖现象。
- 强制把候选窗改成水杉自绘浮层：会绕过 Fcitx5 和 input-method-v2 的面板放置，改变所有 Wayland 桌面行为，缺少宿主证据。
- 让本 PR 继续保持 Draft 直到修复：调查结论和缺失字段只停留在 Draft 分支里不可见，维护者选择先把跟踪记录合入，修复另走独立 PR。

## Acceptance criteria

- 拿到报告者环境的显示缩放、截图应用及其前端（`wayland_v2`、text-input 或 dbus）、实际生效的用户界面插件和 Classic UI 对照结果。
- 在最小合成编辑器中确认候选窗定位的实际责任方（Fcitx5 用户界面、treeland 或水杉）。
- 只有确认产品代码拥有定位责任后，才在单独的 `Fixes #11620` PR 中加入行为修复和对应测试；否则在 issue 中给出上游报告链接。

## Risks

在未确认宿主责任前修改坐标会影响所有 Linux 桌面，并可能重新引入 Wayland 合成器兼容问题。若报告者不再回复，本调查可能长期停在缺失字段上，需要在 issue 中明确仍缺哪些信息。
