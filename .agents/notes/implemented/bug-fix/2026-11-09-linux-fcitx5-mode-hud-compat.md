# Agent Note: Linux Fcitx5 低版本开发头保留模式提示

Status: implemented

## Problem

官方 Linux 包可能在旧版 Fcitx5 开发头上构建，运行时却安装在新版 Fcitx5 上。`showCustomInputMethodInformation` 只有 5.1.11 才出现在开发头中，原实现因此把整个模式提示函数编译掉；运行时的中英文切换提示和聚焦提示都没有任何文字回退。#11138 在 KDE Wayland、`show_app_logo=false` 的配置下复现了这个缺口。

## Decision

保留 5.1.11 及以上的自定义提示 API 和自绘徽章路径；开发头较旧时改用 Fcitx5 长期稳定的 `InputPanel::setAuxUp` 显示同一个「中」或「英」标签。回退提示沿用 IBus 的 600ms 生命周期：以递增编号丢弃过期定时器，只在辅助行仍属于这条提示且当前没有组字时清除，避免覆盖或收掉候选、语音等后续内容。清空输入面板时同时取消回退定时器。

这与[Linux 聚焦提示与中英文切换提示分开控制](2026-10-22-linux-focus-mode-hint.md)的两个偏好和光标等待规则一致；该篇原先只说明旧开发库不登记真实窗口测试，本篇补上旧库构建后的运行时行为。

## Alternatives considered

- **提高 Fcitx5 开发库最低版本**：可以保证 API 存在，但会让仍使用旧版 Fcitx5 的发行版失去插件，不符合包声明允许旧版框架的兼容范围。
- **只保留自绘徽章**：Wayland 客户端没有可靠的全局光标坐标，且没有 layer-shell 或图形依赖时仍会缺提示；辅助行是 Fcitx5 原生面板能覆盖的稳定回退。

## Consequences

旧开发头构建的插件在新版运行时也能显示模式提示，且提示不会无限留在辅助行中。低版本 Fcitx5 本身没有自定义弹出 API，回退外观由它的面板主题决定；自绘徽章和新 API 构建的行为保持不变。

## Verification

`fcitx5-mode-field-hints` 在没有 `MSIME_FCITX5_CUSTOM_IM_INFORMATION` 时断言两次切换分别写入「英」和「中」辅助行；Linux Fcitx5 构建门禁在旧开发库配置下会编译并运行这条用例。`fcitx5_contract.py` 继续检查新 API 路径和宿主调用顺序。
