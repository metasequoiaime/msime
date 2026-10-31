# Agent Note: Android 候选栏支持整体字号与高度调整

Status: implemented

## Problem

#9816 希望候选栏字号和高度可调。Android 已经读取 `candidate_font_size`，但候选行高度固定为 42 DIP，且共享设置页把整体大小控件隐藏在 Android 上，导致字号变大时用户仍无法调整候选栏占用的高度。

## Decision

复用已有共享偏好 `candidate_scale_percent`，将 Android 宿主声明为支持候选栏整体大小。Android 对候选字号、预编辑字号和候选行（包括释义附加行）使用同一 50–200% 缩放策略，并在偏好热更新时立即重排候选栏。共享字段的范围和默认值保持不变，不新增 Android 私有配置。

展开的「更多候选」面板（#10400）同样跟随：格子字号取候选条实际字号（候选字号 × 候选栏大小）相对 client-core 默认字号 18 的倍数乘以设计值 17 sp，格高取「44 dp（带读音行时 60 dp）× 候选栏大小」与「同一字号倍数 × 设计格高」中较大的一个，并且不低于设计格高 44/60 dp（`CandidateScalePolicy.expandedTextSize`、`expandedCellHeight`）。默认字号 18、100% 时仍是 17 sp 和 44/60 dp，与改动前逐像素相同。鸿蒙的展开网格用同一条规则（`CandidateScalePolicy.expandedFontSize`、`expandedCellHeight`，设计值 17fp、44vp），iOS 的展开面板原本就按 `candidateScale`（候选字号 / 18 × 候选栏大小）缩放字号、格高不低于 44pt，三端一致。网格列数仍按固定的 64 dp 最小列宽分，每格占几列按实测的自然宽度算（`CandidateGridPolicy.span`），字号变大时一个候选会占更多列；但占列以一整行为上限，比一整行还宽的候选（整句、日文转换结果）仍会被截掉，而且字号越大越容易出现：360 dp 宽的手机上 17 sp 一行约放 19 个汉字，200% 时 34 sp 约 9 个。

## Alternatives considered

- **只扩大 `candidate_font_size` 的范围**：只能改变文字，不能保证候选 chip、释义行和候选栏容器一起变高，字号变大时仍可能裁切，因此不选。
- **展开面板按 `17 × 候选字号 / 16` 换算**：16 只是 Java 端偏好缺失时的兜底值，Android 实际生效的默认字号是 client-core 的 18，按 16 换算会让默认外观从 17 sp 变成约 19 sp，因此以 18 为基准。
- **展开面板只按 `candidate_scale_percent` 缩放**：100% 时同样不变，但候选条本身同时跟随候选字号，桌面或 MCP 把字号设大后候选条变大、展开面板不变，两处又不一致，因此两者都跟。
- **格高随候选栏大小一起缩小**：与 50% 时 21 dp 的候选行一致，但展开面板占的是按键的位置，按键不随候选栏大小变化，缩到 22 dp 的格子只是让触控目标变小、面板空出一截，iOS 的格子也不低于 44pt，因此设下限。
- **比一行还宽的候选在格子里换行并加高格子**：能读到整句，但要同时改三端的网格测高和行高分配，超出本 issue 的字号范围，留作后续。
- **新增 Android 专用高度偏好**：能单独控制高度，但会产生第二套设置协议和移动端专属落盘字段；现有整体大小契约已经表达了字号与几何同步缩放，复用它更一致。

## Consequences

共享设置页在 Android 上把这一行叫「候选栏大小」，发布包的原生设置页在「键盘 → 工具栏」给出同名滑块（[三端候选栏大小](2026-10-29-touch-candidate-bar-size.md)）；拖动后候选字号、预编辑字号和候选栏高度同步变化，旧配置缺失或越界时仍回到 100%。其他平台的能力位和行为不变。候选字号乘缩放后可能大于原来的 12–32 设置范围，这是整体大小控件的有意效果，候选行仍按相同倍率保留文字空间。

## Verification

新增 `CandidateScalePolicySmoke` 覆盖默认回退、50/200% 边界和 DIP 缩放；更新 Android JVM smoke 编译会自动发现该测试。同步更新 host capability 与设置行测试、Rust host capability 测试。当前工作区没有 Android SDK，无法本机运行 `platforms/android/check-host.sh` 或模拟器验收；需由 CI 和维护者设备验证。展开面板的换算由 `CandidateScalePolicySmoke` 和鸿蒙 `tests/keyboard-logic.test.ts` 覆盖默认值、50/75/150/200%、字号 12/24/32 和 44 dp 下限的取值，鸿蒙视图的接线由 `tests/run.sh` 钉住；200% 加长候选的真机效果还要在 Android 和鸿蒙设备上验收。
