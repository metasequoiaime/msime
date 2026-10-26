# Agent Note: Android 输入法弹出时隐藏详情页导航

Status: implemented

## Problem

Android 11 某些厂商设备在详情页搜索框获得焦点后，输入法已经占据底部区域，但 `WindowInsetsCompat.isVisible(Type.ime())` 仍可能返回 `false`。详情页固定导航因此继续显示，浮在输入法窗口上方。

## Decision

详情页导航的显示策略同时读取 IME 可见状态和 IME 底部 inset。只要可见状态为真，或底部 inset 大于零，就隐藏固定导航；两者都没有时才显示。现有只传布尔值的策略保留，避免改变其他调用方的语义。

## Alternatives considered

- **只依赖 `isVisible(Type.ime())`**：代码最简单，但无法覆盖报告设备的厂商实现差异，因此不采用。
- **按 Android 版本或厂商名称特判**：不能覆盖同版本的其他实现，也会把设备识别逻辑写进布局策略，因此不采用。

## Consequences

输入法显示但可见标记滞后的设备不会再把详情页导航画在键盘上方；输入法完全收起且 inset 清零时导航会恢复。新增策略参数和 JVM 冒烟断言，不改变主页底部导航或输入法自身窗口布局。真实 vivo Android 11 设备尚待报告者复验。
