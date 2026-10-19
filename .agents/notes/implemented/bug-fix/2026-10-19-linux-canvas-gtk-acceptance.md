# Agent Note: Fcitx5 皮肤资源热更新后刷新真实候选窗口

Status: implemented

## Problem

Fcitx5 的静态 canvas 面板已有九宫格像素单测和真实 classicui 配置测试，但前者只检查生成的 PNG，后者没有显示候选窗口。真实 GTK 编辑器验收发现，组合期间损坏皮肤图片后，`theme.conf` 已切回安全配色和原生卡片，屏幕仍显示旧贴图，直到下一次按键才更新。仅重载 classicui 配置不会请求当前 InputPanel 重绘。

## Decision

`applyCandidatePanelTheme` 返回主题是否成功套用或退出覆盖；未变输入、尚未到重试时间、第三方主题所有权和写入失败都不要求重绘。`FcitxState::syncCandidatePanelTheme` 仅在返回成功变化后请求 InputPanel 更新，让资源检查触发的图片变化立即进入真实窗口，不修改组合、候选身份或编辑器焦点。不变的轮询仍保留廉价缓存路径。

在已有的隔离 D-Bus／Xvfb 验收中注册横排、竖排两个真实 GTK 编辑器用例。共享 Host API 准备已校验词库与合成偏好，本次构建的插件加载九色 PNG 和清单。实际按键唤起候选后读取 X11 根窗口像素，验证浅色、深色、损坏后的原生回退、修复后的贴图恢复。切换和资源变更期间不再发送按键，最后检查编辑器焦点仍在、空格提交合成词且面板消失。临时文件、显示服务和 daemon 随用例清理，不操作用户桌面或已安装输入法。

## Historical audit

- [静态九宫格面板](../feature/2026-10-19-linux-fcitx-canvas-panel.md)部分重叠：保留产品映射与像素单测，本篇修复其资源热更新时的窗口重绘，并增加真实窗口证据。
- [IBus 兼容前端验收](../testing/2026-10-15-fcitx5-ibus-frontend-acceptance.md)部分重叠：复用隔离环境和 GTK 输入思路，不替换其前端协议验收。
- [候选渲染边界](../architecture/2026-10-13-cross-platform-candidate-rendering-boundaries.md)仍有效：窗口由 Fcitx5 classicui 拥有，测试不另开模拟候选窗口代替产品。
- 其他 canvas 笔记分别归属共享资源、其他平台或静态图片几何，不改变本次的刷新决定；原有主题所有权、字体与轮询缓存决定继续有效。

## Alternatives considered

- **仅给现有配置测试补更多断言**：不需要显示服务，执行快；但缺陷恰好发生在配置已经正确之后，保留第一层测试并增加窗口像素验收。
- **每次资源轮询都刷新面板**：不需要额外返回值即可覆盖缺陷，但每个有焦点的上下文每 250ms 都会检查主题，输入没有变化时不应重复请求重绘。
- **重启插件来刷新主题**：可以重新加载全部资源，但会清空当前组合，不能作为普通换肤与资源修复的实现或验收手段。
- **目测人工截图**：能覆盖更多桌面环境和真实字体，但不适合作为重复回归检查；保留实机验收缺口，加入合成像素断言。

## Verification

修复前，真实窗口保留旧 canvas 像素，而生成的配置已显示 `Image=shape-*` 和安全配色；修复后，同一编辑器中依次通过浅色、深色、坏图回退、修复恢复、焦点保持及空格提交 `你好`，不依赖中途按键或重启插件。原生配置测试同时断言首次套用需要刷新、不变输入不需要刷新。

当前共享库重新构建后的九项验收全部通过：三项真实 classicui 主题测试、智能 ABC、真实 daemon 前端、原有 GTK 编辑器、新增横排／竖排 canvas 像素用例与 GTK IBus 兼容前端。横排和竖排各保留明暗、损坏、修复四个阶段的实际窗口截图；生成图片不提交。

提交前 `bash scripts/verify-local.sh --quick` 在独立远端镜像通过：Linux 原生 92 项、全拼与五笔构建及安装隔离、Linux 设置壳、Rust workspace、Android Rust／Java、WebAssembly、macOS 原生宿主和共享 Apple bridge 均通过；Windows 本机可运行 170 项通过。完整 Windows 交叉构建与 HarmonyOS 真打包因工具链或工程准备不全而跳过，不计通过。本机笔记校验、Python／Shell 语法、Fcitx5 源码契约与差异检查通过。

扩展验收中的 `fcitx5-native-context` 和 `fcitx5-native-ai` 在共享的快捷模式断言 `Disabled expression mode must pass through` 失败。对照使用同一套当前构建的共享库与固定资源，将插件和测试源码恢复为未修改的 `e3a4af9681810b16c89ec058d47101a7f52f325b`，两项仍在同处失败；它们属于既有基线问题，不在本次重绘修复中改写快捷模式行为或增加已知失败豁免。

## Consequences

资源变更不再等待下一次按键才反映到候选窗口。新增测试依赖验收镜像已有的 GTK、Python D-Bus/GI、Xvfb 与 xdotool，沿用现有已验证资源及 Xvfb／xdotool 的注册门槛；普通原生构建门禁仍可独立运行。像素统计使用合成纯色边框，避免把字体抗锯齿差异当作皮肤失败。

该证据只覆盖隔离的 X11／GTK／classicui 路径。Xvfb 不包含桌面合成器和真实显示缩放，最小镜像的中文字体也不完整；Unicode 提交断言不能代替真实字形验收。Wayland、GNOME Shell、kimpanel、动画、挂件及其他图片格式仍需各自实现与验收，不能据此宣布全平台兼容完成。
