# Agent Note: satellite 会话回退浮层以保住输入焦点

Status: implemented

## Problem

#9157 报告 Fedora 44 / niri 26.04 的微信输入异常。隔离 Fedora 44 ARM 环境运行发行版 niri 26.04、IBus 1.5.34 和 xwayland-satellite 0.8.2，在原生 GTK Wayland 合成输入框中重现工具栏反复成为顶层并获得焦点，IBus 随之不断 focus in/out；同环境 Xwayland GTK 输入框的两窗口组字和切换通过。这个产品缺陷已经确认，但尚未取得报告者 x86_64 微信安装包，不能据此关闭整个 issue。

## Decision

在两种浮层共用的 `open_overlay_display` 连接入口，识别真实 EWMH 窗口管理器为 xwayland-satellite 时关闭连接并返回不可用。工具栏沿既有菜单回退，双拼键位图不画；不改用户偏好，不按 `XDG_CURRENT_DESKTOP` 黑名单，不禁用其余 Xwayland。

检查 root 与 WM 窗口的 `_NET_SUPPORTING_WM_CHECK` 为单项 WINDOW/32，WM 窗口反向指向自己；读取 WM 的 `_NET_WM_NAME`，只匹配完整的 `xwayland-satellite`。发行版 0.8.2 实际写 STRING/8，兼容标准 UTF8_STRING/8。属性不完整、类型错误、窗口已销毁或名字不同均不能把普通 X11 禁用；探测中的 BadWindow 不能结束宿主。探测沿现有主线程 Xlib 错误保护约定执行，并恢复原错误处理器。

## 依据与笔记审计

- [上游缺少 root overlay 的 issue](https://github.com/Supreeeme/xwayland-satellite/issues/429) 说明独立 override-redirect 浮层会成为顶层，不能只靠 InputHint 或窗口类型解决。
- 固定 [v0.8.2 源码](https://github.com/Supreeeme/xwayland-satellite/blob/8d135d3b2854b30fd01ea6cd6c27e523dd50a839/src/xstate/mod.rs) 的 `create_ewmh_window` 提供实际属性契约。
- 按 `open_overlay_display|FloatingToolbarSurface|xwayland-satellite|Xwayland.*根` 检索 proposed、implemented、rejected，仅命中 [Linux X11 浮层决定](../feature/2026-10-21-linux-x11-floating-panels.md)：部分重叠，本篇收窄其中“有 DISPLAY 即可使用”的前提，保留窗口、绘制、菜单和拖动架构并互链。其他平台工具栏、Linux 候选画布及切换徽章的决定不涉及此连接入口，不改它们。

## Alternatives considered

- **新增 Wayland layer-shell 后端**：能在支持协议的桌面恢复原生悬浮工具栏，还可避免 Xwayland 缩放问题。但需要独立处理拖动、指针菜单、缓冲与输出生命周期，超出本次已确认缺陷；既有菜单可提供安全回退，后端接口仍保留。
- **改窗口类型或 InputHint**：改动最小，且能提示普通窗口管理器不要抢焦点。但现有窗口已经 override-redirect、InputHint=False，实测仍成为顶层，上游也确认缺少 root overlay 语义，不能用未经验证的属性组合替代回退。
- **niri 环境变量黑名单或关闭全部 Wayland 浮层**：容易实现，不必读 X 属性。但前者无法识别别的合成器接入 satellite，后者会关闭仍正常工作的后端，因此按实际 WM 精确判断。

## Testing

`linux-floating-panels-x11` 的真实 EWMH 夹具先在原实现失败（satellite 上仍创建浮层连接），修补后通过（2.91 秒）。覆盖 STRING、UTF8_STRING、普通 WM、名字前缀、嵌入 NUL、缺少名称、错误属性类型、多项 WM_CHECK、不一致的反向指针和已销毁 WM 窗口；两种 surface 不创建窗口、不保留监听。原来的窗口、按钮、菜单、提示、拖动、全屏和断线恢复断言同样执行。

从本分支源码构建 IBus、Fcitx5 和全套测试，111 项注册，110 项通过、`linux-ibus-demand-activation` 按环境条件跳过，无失败。Fedora 44 ARM 的真实 niri 26.04 / satellite 0.8.2 嵌套环境使用本分支 Host API 与宿主，两类 GTK 窗口分别经 Xwayland 和原生 Wayland 运行，以 `GTK_IM_MODULE=ibus` 接入 IBus：第一个输入框完整组字 `nihao` 后切到第二个，再输入并提交 `你好`，首个输入框无误提交、旧预编辑清空，未出现浮层顶层或抢焦点。执行前后产品源码和三个原生产物 SHA-256 一致；词库是只读挂载的公开资源产物，未借用其他分支的宿主或源码。

早期研究脚本只等待首个预编辑就切窗，且没有等待 IBus 当前输入上下文和全局引擎 enabled，出现按键跨窗口或透传；日志保留。后续按实际就绪状态及每个合成组字前缀推进，断言不减弱。后续同轮 D-Bus 与 Wayland 协议捕获确认研究环境也曾出现 `ibus-ui-gtk3` 顶层获焦，不能把这些失败归因于水杉先清空组字。研究脚本在外层 XTest 激活后重新确认首框焦点，并在两种后端切窗时等待真实的新 IBus 输入上下文（排除旧上下文及 fake 上下文）。恢复原始 niri 配置、没有添加窗口规则后，两种后端均通过全部断言；默认 `text-input-v3` 协议记录包含两个输入框分别完整的 `n` 到 `nihao` 预编辑链及第二框 `commit_string("你好")`。该轮 D-Bus 记录只覆盖到第二框 FocusIn，不能声称完整捕获后续 Engine 按键和提交信号。加入仅禁止 IBus 顶层自动获焦规则的中间实验也保留，但最终通过结论来自无规则轮。虚拟键盘协议绕过输入法的尝试不作为产品验收。

共享 React 设置的类型检查与生产构建通过；完整 quick 首轮只在语音目录检查构建超过 120 秒时失败，其余阶段完成，失败日志保留，预编同源例程后沿原门禁复验，完整 quick 退出 0。Windows 原生阶段按本机缺少 MinGW 跳过，Harmony HAP 按未 staging 原生资源跳过。

## Consequences

输入焦点不再被这两类 root 浮层抢走，普通 X11 浮层及既有菜单保持。satellite 用户暂时没有悬浮工具栏或键位图，偏好值不变；satellite 将来支持 root overlay 后需要重新核实这个判断。本切片不改变候选窗、切换徽章或 Wayland 输入协议。

没有真实微信包、报告者 x86_64 硬件或完整真实桌面验收，PR 使用 `Refs #9157`，不自动关闭 issue；合成 GTK 输入不等价于微信验收。
