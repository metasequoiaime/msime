# Agent Note: 鸿蒙电脑候选窗不透出触屏键盘背景

Status: implemented

## Problem

电脑候选窗为没有 canvas 的皮肤绘制了候选卡片，同时在卡片下面铺满 `skinBackdrop`；该层来自触屏键盘，可能包含键盘底色、渐变或自定义照片。卡片四角与边缘因此可能露出一块额外的方形底色或照片。已有 canvas 的候选窗已把这层设为透明，普通皮肤却没有。用户反馈候选窗出现方形背景和圆角卡片叠加，但没有做屏幕像素和设备事件验证，不能断言这一处解释所有反馈。

## Decision

通过 `CandidateDecorationLayout.keyboardBackdropVisible` 决定这一层的可见性：鸿蒙电脑只显示候选窗时，无论是否使用 canvas 都隐藏触屏键盘背景；手机键盘及电脑的表情、语音、手写、屏幕键盘面板仍保留这层。候选卡片、皮肤图片和 canvas 继续由原有的 `candidateStrip` 与 `CandidateCanvasFrame` 绘制，不改变其圆角、透明度或资源读取。

## Alternatives considered

保留背景可以继续让电脑候选窗继承键盘底色，但触屏皮肤照片和卡片配色可能不同，矩形底也会盖住窗口圆角外的透明区域。删除 `skinBackdrop` 或隐藏整个电脑面板能消除多余背景，但会影响电脑打开屏幕键盘等工具时的背景以及手机渲染，因此仅在电脑候选模式下隐藏该层。

## Consequences

这个改动只处理触屏键盘背景混进普通电脑候选窗，不能保证主题切换时跨进程设置已同步，也不修改 canvas 图片的圆角裁切或解决其他旧图片缓存问题。窗口圆角以及透明背景的实际显示仍需要设备验收。

## Verification

- `candidate-backdrop.test.ts` 经 TypeScript 6 严格编译和 Node 执行通过，覆盖电脑普通候选、四种电脑工具面板和手机场景的可见性规则。
- 使用同一编译器执行已有 `keyboard-logic.test.ts`：4791 项断言通过。
- 以上测试验证逻辑而不验证 ArkUI 合成与实际像素；没有构建 HAP、运行真实输入或获取设备截图。
