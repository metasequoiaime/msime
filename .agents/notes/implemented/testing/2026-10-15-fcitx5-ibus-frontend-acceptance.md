# Agent Note: Fcitx5 的 IBus 兼容前端验收

Status: implemented

## Problem

#8009 补充了 Deepin 25 的实际证据：只有 Fcitx5 daemon，应用使用 IBus 模块，框架收到 `frontend:ibus cap:52 focus:1` 的上下文，程序名为空。既有验收只经过 Fcitx5 自己的 D-Bus 前端和 GTK Fcitx 模块，不能覆盖这个组合。程序名为空还不能将用户日志里的某个上下文归属到失败应用。历史玲珑说明把运行时缺模块当成所有案例的根因，证据不足。

运行现有资源验收还发现两个原生测试因菜单数量断言过时而失败：实际 44 项，断言仍为 43 项。这使后面的会话行为检查提前退出。

## Decision

沿用隔离 D-Bus、真实 Fcitx5 daemon 和现有合成资源准备流程，新增 IBus 协议组字、空格上屏和焦点重建验证，以及使用 `ibus` 模块的真实 GTK 编辑器验收。GTK 验收等待真实前端上下文获得焦点，再切换输入法；不能用旧上下文仍返回输入法名称作为就绪条件。容器验收脚本执行新测试。菜单断言更新到当前 44 项。

说明区分 daemon、应用模块和前端，不再从 `GTK_IM_MODULE=ibus` 推断 daemon 是 IBus，也不保证切换到 Fcitx5 就能解决 #8009。没有 Deepin 玲珑真实验收前，issue 保持打开。

## Historical notes

- [旧玲珑提示](../../implemented/feature/2026-10-10-linux-linglong-ibus-hint.md)：部分重叠。保留提示的适用范围和 legacy 包限制，修正其对已连接 IBus 兼容前端的推断；两篇互链。
- [Linux 测试构建门禁](../../implemented/testing/2026-10-09-linux-build-testing-gate.md)：无关，决定的是 `BUILD_TESTING` 的目标边界。
- [会话偏好跟随存储](../../implemented/bug-fix/2026-10-10-fcitx5-session-preferences-from-store.md)：无关，决定的是会话配置来源。
- 其余匹配是脚本命令菜单、发行版依赖和跨平台功能记录，不涉及前端传输验收，保持原有决定。

## Alternatives considered

- 只通过 IBus D-Bus 手工构造按键：容易控制能力标志和焦点，但不能证明 GTK 模块、地址发现与真实编辑器上屏，保留协议检查并增加 GTK 测试。
- 直接修改按键路由或匿名程序模式恢复：可能修中真实问题，但当前代码已在同样能力标志、匿名程序名的兼容前端成功上屏，没有证据支持修改生产逻辑。

## Verification

固定 Debian 容器中，修复前 `fcitx5-native-context` 与 `fcitx5-native-ai` 均在菜单数量断言失败；更新为 44 后，两项继续执行并通过。完整 Fcitx5 资源验收 5/5 通过，包括原生上下文、原生 AI、原有 D-Bus 前端、GTK Fcitx 模块和新增的 IBus 协议/GTK IBus 模块验收。新增协议检查分别使用有预编辑与周边文本、仅预编辑、仅周边文本的能力组合，复用匿名程序身份。

负面对照只在忽略的研究脚本中把 GTK 模块改成 `simple`，同一验收以 1 退出，在等待真实前端焦点时失败。恢复 IBus 模块的正式测试通过。Linux 配置提示测试、Python 语法、Shell 语法和 `git diff --check` 通过。

另在官方 Deepin 25 amd64 容器中安装 2026-10-15 的拼音版 nightly 包，使用发行版自带 Fcitx5 5.1.12 与 GTK IBus 模块运行同一协议/编辑器检查，通过。基础镜像固定为 `linuxdeepin/deepin@sha256:1f988aeed0675f2c594d253095dfda4e9f0242d2110f8a8618d9336695df66e1`，包 `msime-linux-pinyin_0.11.0_amd64.deb` 的 SHA256 为 `5156a35b2a857c15d97fdb02cab6384650396ff450c756391585e4e84dd8c39f`，与 GitHub release asset digest 一致；测试配置只将插件 ID、安装路径适配到拼音版，没有替换包内插件或 Host API。该检查使用 Xvfb，不包含 DDE 会话或玲珑沙盒。

`bash scripts/verify-local.sh --quick` 通过：Windows 本机测试 149 项、仓库契约与笔记、Rust workspace/Android/Wasm 编译、Android Java 宿主、Linux 桌面编译、Linux 原生 87 项与版本安装隔离、macOS 原生及共享 Apple bridge 编译。缺少工具链或工程准备输入的阶段按脚本跳过；这些结果不代表 Android 或其他平台的设备验收。

## Consequences

真实框架传输路径现在有固定验收入口；GTK 就绪判据不再提前选中旧上下文。新增验收依赖现有资源目录、Xvfb、xdotool、Python D-Bus/GI 和 IBus GTK 模块，固定验收镜像已经提供它们。

Debian 固定容器不能代表 Deepin 25 的发行版补丁和玲珑沙盒。此变更只补验证和修正错误提示，不关闭 #8009。
