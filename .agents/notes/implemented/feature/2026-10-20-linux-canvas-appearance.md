# Agent Note: Linux 原生画布消费选中外观与预编辑细分色

Status: implemented

## Problem

[原生 X11 画布](../../implemented/architecture/2026-10-20-linux-native-canvas-x11.md)已消费[字体与布局](../../implemented/feature/2026-10-20-linux-canvas-layout.md)，但高亮仍是矩形，不消费选中条、贴角、悬停与预编辑背景。固定参考 `msime-windows@dd018f79858314ed542a47c528608752b13bd261` 的 tokens、scene 与 `CandidateList.cpp`。

## Decision

在既有 Pango/Cairo 画布里接入逐角高亮、选中条、按下和悬停，以及预编辑文字、背景、分隔线与光标。基础槽位和 `candidate_details` 统一由共享 Rust 层规范化；后者新增预编辑背景、分隔线和菜单四槽位，新增字段为空时省略，旧宿主可继续忽略。Linux 不从原始包重新取色。显式用户文字取色器仍覆盖预编辑文字。预编辑光标只使用与显示字符串同源的字节位置，不推测变换后拼音的位置。

X11 指针状态仅触发重绘，不更改候选身份或键盘选中项；候选更新、菜单切换、隐藏时清掉旧的按下与悬停。计时和资源缓存继续归原有动画生命周期。保留未写字段的原生默认绘制。

### 旧笔记审计

- 原生 X11、字体布局、图片动画：部分重叠，互链保留各自窗口、测量和计时决定。
- [共享清单](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)和 [Windows 布局](../../implemented/feature/2026-10-19-windows-candidate-skin-layout-keys.md)：部分重叠，沿用已校验 JSON 和固定来源，不新建解析器。
- 其他平台、IBus 点击围栏与展开网格：不改变这些决定。

## Alternatives considered

把原始包配色完整覆盖到画布最容易保留全部字段，却会覆盖共享层已经处理的用户取色器与可读文字色，而且清单允许 CSS 缩写与 rgba；因此在共享层新增可选细分槽位，原生只消费规范化颜色。

直接在 pointer motion 中更改选中候选可以复用现有高亮，但会改变键盘选择语义；悬停作为独立绘制状态。

## Testing

- 共享主题 26 项相关测试与 Host API 配色协商回归通过，覆盖 CSS 颜色规范化、用户取色器、明暗／排列边界和独立菜单色。
- iOS 画布配色回归同时断言无取色器时保留皮肤预编辑色、设置文字取色器后覆盖候选和预编辑文字；选中翻译、选中条颜色及开关在两种状态下保持独立，与固定上游的取色顺序一致。run `20261020-070616` 在 iOS 27 模拟器按现有 PR 范围运行四个原生测试包，共 1,034 项，1,024 项通过、10 项因未签名容器或缺少附加词库／释义而跳过、零失败；现有脚本另外排除三个 ML Kit 识别用例和两个已知失败。该 run 的笔记校验和完整 quick 也通过，不把跳过项计作设备验收。
- Debian 12 隔离容器 run `20261020-062933`：35/35 原生、真实 GTK、图片动画及 Wayland 静态回退检查通过，无跳过。
- 修正菜单标题的候选背景继承后，run `20261020-063431` 再次通过六组横竖排／2 倍缩放／合成器 GTK 验收及 Cairo 像素检查，共 7/7；覆盖选中条实际宽度、圆角、预编辑配色、悬停、按下、拖出释放、菜单及返回、取色器热更新与恢复。实际截图已检查。
- 同一 run 的完整 quick 通过：Linux 原生 93/93、full/wubi 构建及安装隔离、Rust 工作区、Android target／Java、WASM、Linux 设置、macOS 原生和 Apple 桥接均通过；`client-services` 与 `host-api` 的 all-targets clippy 在 `-D warnings` 下通过。
- 构建副本缺 HarmonyOS HAP 打包前置和 Windows 原生／MinGW／pipe 工具链，相关项跳过；当前宿主 Windows 策略 171 项通过、2 项按宿主差异排除、62 项需 Windows 构建。macOS Tauri 工作区项因未暂存 bundle 跳过，Linux 设置壳已独立检查；参考仓库比较、词库再生成及部分可选检查按脚本跳过，不计作设备验收。
- Rust 格式、Python 语法、笔记及差异检查通过。

## Consequences

[释义分段与配色](2026-10-20-candidate-translation-runs.md)补充同源范围和原生 Pango 绘制。[列表内翻页](2026-10-20-linux-canvas-layout.md)补充可选分隔线槽位和按钮状态；本次不覆盖材质、声明式进退场和展开面板，也不代表 Wayland、IBus 或全平台视觉兼容完成。九宫格图片保留自身 alpha，不把卡片圆角额外裁到图片上。菜单不叠候选图片与挂件，标题不继承候选预编辑背景和分隔线，停掉其不可见帧的唤醒；返回候选后按原单调时钟继续。收益是清单外观进入真实绘制；代价是需要维护独立指针状态和细分颜色的透明回退语义。
