# Agent Note: 候选窗皮肤 v2 的有限动画契约

Status: implemented

候选界面的进场、退场、逐行揭示与高亮呈现由[所有平台的候选立即呈现](../simplification/2026-10-30-candidate-immediate-presentation.md)取代；本篇的清单格式、旧包兼容性与装饰图独立契约继续成立。

## Problem

候选窗皮肤原有 `schema_version = 1` 只描述颜色和几何。方案文档要求候选展示继续由各平台宿主拥有，同时希望候选窗有漂亮的动画；直接把 CSS 或 WebView 动画放进共享皮肤包会破坏 Linux 面板、原生宿主和社区同步的边界，也无法在 reduced-motion 或不支持动画的平台上安全降级。

## Decision

皮肤清单现在接受版本 1 和版本 2。版本 1 的动画固定为静态；版本 2 可以声明 `[animation]`，但只能从 `none`、`fade`、`fade_scale`、`slide_up`、`spring`、`glow` 六个固定预设中选择，持续时间限制在 40–600ms，交错间隔限制在 0–80ms。没有声明表的 v2 包使用短时的 `fade_scale` / `fade` / `spring` 默认组合。

共享 Rust catalog 解析并序列化这份语义，MCP 的皮肤列表报告它，创建工具和 `make_skin` prompt 描述同一组字段，CLI 帮助也指向 schema v2。Web engine 将预设映射为自己的固定关键帧，不执行包内 CSS 或脚本；候选栏在显示、选中和隐藏时使用动画，并由 `prefers-reduced-motion` 关闭。macOS 的非激活 `NSPanel` 在显示、隐藏和选中候选时分别映射淡入/淡出、弹簧/发光等 Core Animation 效果，并用代次栅栏避免关闭动画的迟到回调收起新一帧。Windows 原生候选窗同样解析这份模型，在 Direct2D 合成层上执行淡入、缩放、上移、弹簧和发光，并把 Windows 的 `SPI_GETCLIENTAREAANIMATION` 作为 reduced-motion 开关。Linux 的 Fcitx5/IBus 面板由桌面面板拥有，保留宿主自身的动效能力，因此忽略动画字段而不影响候选身份、焦点或 Wayland 定位。

## Alternatives considered

- **允许皮肤携带任意 CSS `@keyframes`**：视觉自由度最高，但会把不可信样式和动画解析带进同步包，且无法让 Windows、macOS、Fcitx5/IBus 对同一份 CSS 保持一致，因此不采用。
- **在所有平台新建统一的 Web/浮窗渲染器**：动画更容易复用，但会绕过方案规定的 TSF、InputMethodKit、Fcitx5 和 IBus 宿主边界，尤其会给 Wayland 带来定位和焦点风险，因此不采用。
- **仅在各原生宿主分别添加动画字段**：能快速做出局部效果，但共享清单、MCP、CLI 和网页 SDK 会漂移，皮肤作者无法得到稳定的能力契约，因此不采用。

## Consequences

动画字段是可同步、可审计的纯数据，旧版静态包仍可读；不支持动画的宿主不会拒绝皮肤或影响输入。Web 候选栏具有淡入、上移、弹簧高亮和淡出效果，macOS 原生候选按钮和 Windows Direct2D 候选卡片也会播放对应动效，且系统减弱动态效果时不播放。新增预设需要同时修改 Rust 校验、MCP/CLI 文案、Web 映射、原生映射和测试；预设数量刻意保持小，不能满足任意复杂的逐元素编排。

验证覆盖 `msime-client-core` 全量单测、MCP server 单测、Host API 皮肤测试、Web skin 单测、macOS 候选面板 Objective-C++ 严格语法检查和 Rust 格式检查；Web 测试还核对动画 CSS 变量只包含有限的 `ms` 数值，不能跳出样式声明。

macOS 动画映射先由[即时显隐决定](../bug-fix/2026-10-14-macos-candidate-instant-display.md)取代，现由[只播放清单写出的动效](../feature/2026-10-16-macos-candidate-declared-motion.md)决定。macOS 原生候选窗只执行清单 `[animation]` 表里显式写出的预设，省略的预设（包括没有这张表的 v2 包得到的活泼默认组合）按 `none` 处理；catalog 用 `SkinAnimation.declared` 标出哪些预设是写出的。显隐和选中状态同步变化，动画只加在 Core Animation 图层上，退场在独立的不接收鼠标的窗口里进行。各预设在 macOS 上的映射见那篇笔记。共享格式、隐式默认组合与其他平台映射仍成立。

`[animation]` 只管显隐和选中。候选窗在屏幕上时装饰图自己的待机动效和 APNG 帧动画是另一份契约，见[装饰图的待机动效与 APNG 帧动画](../feature/2026-10-16-skin-decoration-idle-motion-and-apng.md)。
