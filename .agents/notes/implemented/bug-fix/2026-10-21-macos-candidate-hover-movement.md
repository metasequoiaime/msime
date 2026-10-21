# Agent Note: macOS 候选悬停由指针移动激活

Status: implemented

## Problem

#9086 报告打字隐藏指针后，新候选窗出现在原指针位置时仍有悬停底色。产品 `CandidateChrome.h` 与保留面板 `CandidatePanel.mm` 都在 `mouseEntered:` 中无条件激活悬停，且没有注册区域内的移动消息，无法区分指针进入与跟踪区域出现在静止指针下。

## Decision

两条按钮路径都注册 `NSTrackingMouseMoved`，只在 `mouseMoved:` 中激活悬停；进入、离开以及重建跟踪区域时清除悬停。实际移动进按钮时，同一跟踪区域会接着收到移动消息；候选窗出现或重排本身不再激活底色。保留鼠标点击、指针样式与键盘选中背景。移动订阅遵循 [AppKit 跟踪区域文档](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TrackingAreaObjects/TrackingAreaObjects.html)，由同一跟踪区域把区域内移动交给 owner。

## Related decisions

检索 `candidateHovered`、`mouseEntered`、`mouseMoved`、`NSTrackingArea`、悬停与隐藏指针后，没有现有笔记规定悬停事件来源。[候选卡片定位](../../implemented/bug-fix/2026-10-16-macos-candidate-card-anchor.md)部分重叠于跟踪区域与窗口移动，定位规则保持；[外观设置收口](../../implemented/simplification/2026-10-16-macos-appearance-settings-prune.md)部分重叠于保留悬停配色，颜色规则保持。其余命中为其他平台或皮肤布局，与事件来源无关。

## Alternatives considered

- **查询鼠标移动计数并在进入时比较**：可以在进入回调就判断此前是否有过移动。但计数变化也可能发生在窗口重排以前，需要额外处理基线和复用；区域已经支持独立的移动回调，不必引入另一套状态。
- **完全关闭悬停底色**：可以覆盖所有隐藏指针情况。但会移除用户正在使用的鼠标定位反馈和悬停颜色偏好。

## Verification

真实 AppKit 产品按钮的新回归在旧实现上失败：仅进入消息就激活了悬停。修复后按钮检查通过，覆盖进入、区域内移动、离开、跟踪区域重建后再次移动，并检查 `NSTrackingMouseMoved` 与非激活窗口需要的 `NSTrackingActiveAlways` 订阅。保留面板增加相同事件序列回归，原有点击选词检查继续保留。远端完整 `bash scripts/verify-local.sh --quick` 通过，macOS 全套 154/154 项 CTest 通过，本机 `pnpm run verify-notes` 与 `git diff --check` 通过。

首次全套为 153/154，未改动的 `settings-glass` 出现一次设置明暗同步等待超时；该测试及其直接包含的实现均与基线一致、不引用本次候选按钮代码。单独复跑以及连续五次检查通过，随后未改代码的整套重跑 154/154 通过（182.68 秒）。保留首次失败与复跑日志，没有新增已知失败豁免或放宽测试超时。

在远端显示合成候选面板时未捕获自然产生的进入消息；直接向 `NSWindow` 或本进程注入移动也未经过系统跟踪区域的分发。随后将探针以原生应用包经 LaunchServices 放到本机图形会话：合成非激活面板使用产品的真实按钮，出现在静止指针下；旧实现得到 `enters=1 moves=0 hovered=1 pointerMoved=0`，修复后为 `enters=1 moves=0 hovered=0 pointerMoved=0`。指针是否移动以公开的 Core Graphics 移动事件计数作交叉检查，不记录位置或用户输入。此结果直接复现并修复窗口出现导致的合成进入；仍不代表在报告人的系统输入法窗口里完成了指针隐藏和物理移动验收。

## Consequences

收益是窗口出现、移到指针下或重排时不再凭位置激活悬停，鼠标移动时的底色与用户配色继续保留。代价是从进入到区域内移动消息之间不画悬停。跟踪区域重建会先清除旧悬停，再等下一次移动。消息注入不覆盖 WindowServer 与物理指针链路，真实系统输入法里的隐藏与移动仍需验收。
