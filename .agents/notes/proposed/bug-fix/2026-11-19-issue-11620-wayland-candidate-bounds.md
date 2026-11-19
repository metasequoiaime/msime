# Agent Note: #11620 Wayland 候选框越界调查

Status: proposed

## Problem

#11620 报告 Deepin 25、DDE Wayland 和 Fcitx5 下候选框超出屏幕边缘。当前证据只有截图、版本和现象，没有 Fcitx5 UI、缩放比例、问题应用或 `fcitx5-diagnose` 的 Wayland/UI 信息。仓库的 Wayland 路径把候选列表交给 Fcitx5，由 Fcitx5 根据输入上下文定位，水杉的 X11 自绘候选窗夹紧工作区坐标，不能直接解释该报告。

## Proposal

将本 issue 作为候选定位调查 Draft PR 跟踪。先收集 Fcitx5 UI、显示缩放、问题应用、Classic UI 对照和 `fcitx5-diagnose` 摘要，再判断问题属于 Fcitx5、Wayland text-input 或水杉候选内容触发。确认水杉拥有可修改的定位边界后，另在本 PR 加入针对该路径的行为修复与测试；证据不足时保持 Draft，不改变当前候选定位行为。

## Alternatives considered

- 直接修改 X11 `NativeCanvasX11` 的工作区夹紧：报告环境是 Wayland，当前路径不经过该窗口，无法覆盖现象。
- 强制把候选窗改成水杉自绘浮层：会绕过 Fcitx5 的 Wayland 面板协议，改变所有 Wayland 桌面行为，缺少宿主证据。

## Acceptance criteria

- 得到报告者环境的 Fcitx5 UI、缩放、应用和 Wayland 诊断信息。
- 在最小合成编辑器中确认候选窗定位的实际宿主路径。
- 只有确认产品代码拥有定位责任后才加入行为修复和对应测试。

## Risks

在未确认宿主责任前修改坐标会影响所有 Linux 桌面，并可能重新引入 Wayland 合成器兼容问题。
