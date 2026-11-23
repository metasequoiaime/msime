# Agent Note: GNOME Shell 扩展渲染 IBus 候选右键菜单

Status: implemented

## Problem

[2026-11-19 的候选维护笔记](../../implemented/feature/2026-11-19-linux-ibus-candidate-maintenance.md)最终把右键菜单做成宿主自绘 X11 浮层，靠 GNOME Shell 扩展导出候选框矩形来避让合成器绘制的候选框 actor。功能可用但归属不对：菜单和候选框分处两个层级（一个是 X11 客户端窗口，一个是 Shell actor），宿主必须自己处理命中、点外收起、显示连接事件泵与工作区夹取；扩展明明就在合成器侧、已经能拿到候选框 actor，却没有画菜单。本次把 GNOME 会话下的菜单渲染职责移到扩展，保留四项维护动作与快捷键。

## Decision

GNOME Shell 会话下，右键菜单由随装扩展 `msime-pointer@msime` 用 `PopupMenu` 画在候选框旁；非 GNOME 会话继续使用宿主的 `CandidateMenuSurface` 自绘浮层，两者共用同一份条目来源与动作校验。

- **条目只来自 C++**：`candidate_menu_items()` 产出的 `CandidateMenuItem`（文案、顺序、checked/disabled/separator）经 `shell_menu_entries()` 转成 wire 条目，动作名用 `candidate_action_name()` 编码成 `Action/session/generation/index`。新增 `CandidateActionName.h` 让引擎分发与扩展回传共用这份编码，`ClientEngine.cpp` 的 `candidate_action_name(const char*, const Json&)` 转发到它，行为不变。
- **wire 契约**：`org.msime.Pointer` 的 `ShowCandidateMenu(s token, s identity, a(ssuy) items)` 推整份菜单，`HideCandidateMenu(s token)` 收起。`a(ssuy)` 的四个字段依次是 action_name、label、flags(uint32)、position(uint8)；`flags` 位为 checked=1、disabled=2、separator=4。`identity` 只是 `session/generation/index` 的展示层一致性信息，扩展不拿它做信任判断。token 来自 `next_client_token`，会话出口由 `release_candidate_menu()` 按 token 调 `HideCandidateMenu`，扩展只在 token 相同时收起，迟到的收起不会误关新菜单。
- **点击回传复用 IBus 既有通道**：扩展把 `action_name` 原样交给 `IBusManager.getIBusManager()._panelService.property_activate(action_name, 0)`，命中 `ClientEngine::property_activate` 里已有的 `CandidatePin/Remove/Fix/Clear` 分支；该分支按编码里的 session/generation/index 定位候选，并核对 `session`、`rendered_session`、view generation、`dictionary_actions_available`。引擎不新增回调入口，扩展也不内置文案或策略。
- **收起不碰输入焦点**：不用 `PopupMenuManager` / `Main.pushModal`——它的 modal grab 可能让引擎进入 `!focused` / `blocked`，而 `property_activate` 开头就拒绝这种状态下的点击。改为扩展自己在 `global.stage` 上接 `button-press-event`（点菜单外收起，返回 `EVENT_PROPAGATE` 让菜单项自己的 activate 生效）与 `key-press-event`（Escape 收起并返回 `EVENT_STOP`，不放它到引擎变成 `MSIME_CANCEL` 取消组字），候选 popup 隐藏时收起，`disable()` 时断开全部监听。ClutterStage 只有带 `-event` 后缀的信号名；连不存在的信号在 gjs 里直接抛异常，会让 `ShowCandidateMenu` 在 `menu.open()` 之前中断、菜单完全弹不出来。引擎侧 Esc 分支也按 `candidate_menu_token` 兜底收起，覆盖事件仍下发到引擎的环境。透明 `sourceActor` 放在候选框右边缘，`PopupMenu.open()` 把菜单 actor 提到输入面板 group 同级最上方并定位。
- **GNOME 下不发布候选动作属性**：`publish_candidate_properties()` 在 GNOME 会话直接 return 是历史保护（大属性树反复重建会让 Shell 冻结），本次不动它。菜单要激活的 `CandidatePin/...` property 在 GNOME 下并未注册；IBus 的 `PropertyActivate` 按名字转发给引擎 vfunc、不查注册表，这是回调链的前提。

## Verification

- TDD 红灯先行：`linux-shell-candidate-menu` 在 `ShellCandidateMenu.h` 不存在时编译失败，实现映射后通过；`linux-gnome-pointer-dbus` 在 `shell_show_candidate_menu` 未定义时编译失败，实现客户端后通过。
- `linux-gnome-pointer-dbus` 用假服务真跑（本机有 session bus）：验证 `a(ssuy)` 三项 fixture 逐字段一致、hide token 原样回传、名字释放后两条调用都返回 false。
- `ctest -R 'linux-candidate-menu-items|linux-shell-candidate-menu|linux-gnome-pointer-dbus'` 3/3 通过；`msime-ibus-host` 以 `-Wall -Wextra -Werror` 编译通过。
- 扩展 JS 经 `node --input-type=module --check` 语法检查。最终审查发现 `global.stage.connect('button-press'/'key-press')` 连的是 ClutterStage 不存在的信号（gjs introspection：`button-press=0`、`button-press-event=28`、`key-press=0`、`key-press-event=31`，连不存在的信号会抛异常）。用桩模块在 Node 里跑了扩展行为夹具：修复前在 `button-press` 处抛错（RED），改为 `*-event`、Esc 返回 `EVENT_STOP`、点外返回 `EVENT_PROPAGATE` 并保存 popup 引用后夹具全通过（GREEN）；夹具同时断言点击原样回传 `action_name` 与 state 0、迟到 hide 不误关新菜单、监听全部断开。夹具是一次性验证脚本，未入库——仓库没有 GNOME Shell 扩展的 JS 测试框架，见任务 3 裁决。
- 真机验证：从当前工作树手工 CMake/CPack 构建 `target/local-deb/dist/msime-linux_0.17.0_amd64.deb`（本机无 Docker，复用 `target/linux-verify-full` 缓存增量编译 ibus 宿主与扩展并补 `MSIME_ENABLE_FCITX5=ON`），装后核对扩展为 6884 字节且含 `ShowCandidateMenu`、ibus 二进制含 `show_candidate_menu`。本机是 GNOME Shell 51 Wayland，系统扩展不能热加载，重新登录后由用户完成实机确认并报告通过。
- `ibus-stale-candidate-click` 本机构建未注册（`MSIME_ENGINE_RESOURCES` 为空），未跑；本次改动未触及 `candidate_clicked` 顶部的 session/generation 护栏。

## Alternatives considered

- **扩展读已发布的 IBus 属性树**：GNOME 下 `candidate_actions()` 根本不发布；即便发布，那是托盘菜单语义（含朗读、生词本、翻页），右键菜单的筛选、顺序、分隔线与文案要在 JS 里重做一遍，会与 `CandidateActionPolicy.h` 漂移——不选。
- **扩展回调新增独立 D-Bus 服务**：能让参数更结构化，但要新增 well-known name、own_name 与引擎侧一套校验；现有 `property_activate` 已实现四项动作和校验——不选。
- **在 GNOME 下发布候选动作属性让注册成立**：会让大属性树反复重建、重新引入 Shell 冻结的历史回归——明确不选，宁可依赖未注册属性也能激活这一前提并在真机验证。
- **保留宿主自绘浮层、继续改进避让**：这是被取代的方案，已实证 X11 窗口压不过合成器 actor，未解决根因——不选。
- **用 `PopupMenuManager` 管 Esc/点外收起**：Shell 现成方案，但 modal grab 与 `property_activate` 的 `!focused`/`blocked` 护栏冲突，可能让每次点击都被拒——不选，改自建 stage 监听。

## Consequences

- 收益：菜单和候选框同处合成器层级，不再有遮挡问题；宿主删掉 GNOME 分支的浮层定位、事件泵与显示连接监听，菜单策略仍集中在 C++；`GetCandidateRect()` 保留供兼容与契约测试使用。
- 代价：新增两个 D-Bus 方法与 C++/JS 两侧必须同步的 `a(ssuy)` 签名；依赖 Shell 内部 API（`IBusManager._panelService`、`_candidatePopup`、`global.compositor.get_input_panel_group()`、`PopupMenu`），45–51 目前一致但升级大版本要回归；未注册 property 激活这一前提已随实机验证确认可用。菜单推送/收起的生产超时从矩形查询的 40ms 提到 200ms，避免 Shell 忙时「调用已执行但引擎拿不到 token」，扩展没装时 `DO_NOT_AUTO_START` 仍立即失败。
- 非 GNOME 会话（纯 X11、KDE 等）的右键菜单行为完全不变；fcitx5 走独立模块，不受影响。
- 本改动与工作树里既有的 41 项未提交改动同处一个工作树，提交时需按文件清单拆分，避免把成对标点光标等无关 WIP 混进本功能 PR。

[上一篇候选维护笔记](../../implemented/feature/2026-11-19-linux-ibus-candidate-maintenance.md)记录了自绘浮层与 GNOME 定位的迭代史；本篇接管其中「菜单由谁渲染」的决定，快捷键与动作校验的取舍仍以上一篇为准。
