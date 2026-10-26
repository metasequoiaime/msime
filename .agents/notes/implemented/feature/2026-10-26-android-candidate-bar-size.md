# Agent Note: Android 候选栏支持整体字号与高度调整

Status: implemented

## Problem

#9816 希望候选栏字号和高度可调。Android 已经读取 `candidate_font_size`，但候选行高度固定为 42 DIP，且共享设置页把整体大小控件隐藏在 Android 上，导致字号变大时用户仍无法调整候选栏占用的高度。

## Decision

复用已有共享偏好 `candidate_scale_percent`，将 Android 宿主声明为支持候选栏整体大小。Android 对候选字号、预编辑字号和候选行（包括释义附加行）使用同一 50–200% 缩放策略，并在偏好热更新时立即重排候选栏。共享字段的范围和默认值保持不变，不新增 Android 私有配置。

## Alternatives considered

- **只扩大 `candidate_font_size` 的范围**：只能改变文字，不能保证候选 chip、释义行和候选栏容器一起变高，字号变大时仍可能裁切，因此不选。
- **新增 Android 专用高度偏好**：能单独控制高度，但会产生第二套设置协议和移动端专属落盘字段；现有整体大小契约已经表达了字号与几何同步缩放，复用它更一致。

## Consequences

Android 设置页会显示「整体大小」滑块；拖动后候选字号、预编辑字号和候选栏高度同步变化，旧配置缺失或越界时仍回到 100%。其他平台的能力位和行为不变。候选字号乘缩放后可能大于原来的 12–32 设置范围，这是整体大小控件的有意效果，候选行仍按相同倍率保留文字空间。

## Verification

新增 `CandidateScalePolicySmoke` 覆盖默认回退、50/200% 边界和 DIP 缩放；更新 Android JVM smoke 编译会自动发现该测试。同步更新 host capability 与设置行测试、Rust host capability 测试。当前工作区没有 Android SDK，无法本机运行 `platforms/android/check-host.sh` 或模拟器验收；需由 CI 和维护者设备验证。
