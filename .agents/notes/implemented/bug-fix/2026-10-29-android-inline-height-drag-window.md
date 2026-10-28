# Agent Note: Android 拖动内联高度条时固定输入法窗口

Status: implemented

## Problem

#10025：Android 0.6.0 在功能面板「键盘高度」打开内联高度条后上下拖动，键区频繁闪成整块白色，输入法窗口和上方应用一起上下跳。报告人给的对照录屏里，另一款输入法拖动时应用完全不动，键盘窗口上方露出一块区域，键区在窗口里伸缩，没有空白帧。

根因在每次拖动都改输入法窗口的高度。`InlineHeightBar` 的拖动柄每收到一个 `ACTION_MOVE` 就更新百分比，`MSIMEInputService.previewInlineHeight` 随即调 `ImeStyler.applyKeyboardGeometry` 改行高并对根视图 `requestLayout`。停靠时输入法窗口按内容包裹高度贴底，键区一变高，窗口就跟着改高，系统按新的窗口上沿重算 insets，应用整体重排；新尺寸的窗口在键区画好之前先露出窗口底色，就是录屏里成串的白帧。

检索审计：[浮动键盘](../../implemented/feature/2026-10-08-android-floating-keyboard.md)部分重叠，它否掉了「停靠时也一直铺满」，本篇只在调整高度这个临时模式里铺满，不改那条决定；[全文预览](../../implemented/feature/2026-10-20-android-clipboard-card-preview.md)部分重叠，本篇复用它「记下打开前的屏幕上沿、按当前窗口原点换算」的 insets 做法；[Android 9 窗口 insets 修复](2026-10-24-android-api28-ime-window-insets.md)部分重叠，API 28 本来就是全高窗口，本篇只给它补上调整期间的固定边界；[键盘高度范围](../../implemented/feature/2026-10-08-android-keyboard-height-range.md)持有 75%–170% 的范围、封顶和内联条的读数，本篇不改。

## Decision

- 打开内联高度条（`showInlineHeight`）时，先记下键区此刻在屏幕上的上沿 `inlineHeightKeyboardTop`，再把 `inlineHeightActive` 计入 `applyFloatingLayout` 的全高条件：输入法窗口、系统 `inputArea` 与键盘根视图铺满，键盘外框包裹内容贴底。功能面板盖在键区上、与键区同高，记下的就是应用此刻正在避让的边界，所以打开时应用不动。
- 调整期间 `onComputeInsets` 把 `contentTopInsets` / `visibleTopInsets` 固定在记下的上沿（`KeyboardHeightPolicy.adjustingInsetTop` 换算成窗口坐标、不小于 0），触摸区域只算键区矩形。拖动只在窗口里伸缩键区：变高时盖住应用底部，变矮时露出下面的应用窗口，应用不重排、编辑器不丢焦点。
- 根视图用 API 28 已有的停靠背景，只在键区所在的底部横带画键盘底色，上方透明，平板居中键区两侧仍是键盘底色；键区每次布局后重画这条横带。结束后换回整块底色，不让 `setBackgroundColor` 就地改色把只画横带的背景留下。
- 「完成」「取消」或收起键盘（`finishInlineHeight`）时清掉记下的上沿，`render` 按最终高度恢复停靠窗口：整个调整只改一次窗口高度。
- 调整中转屏时旧的屏幕坐标作废，键区在新窗口里下一次布局时重新记下；记下之前边界退回键区此刻的上沿。
- 浮动键盘和跟随光标的候选栏本来就是全高窗口、应用不避让，`onComputeInsets` 先走它们的分支，本篇不改变它们。iOS 和鸿蒙的高度条是否有同样的闪烁尚未确认，不在本篇范围内。

## Alternatives considered

- **把 `ACTION_MOVE` 合并到下一帧再应用**：改动最小，能减少同一帧里的多次布局；但每一帧仍然改窗口高度、让应用重排，白帧只是变少，跳动还在，不解决根因。
- **只去掉 `applyKeyboardGeometry` 里对根视图的 `requestLayout`**：停靠窗口按内容包裹高度，键区变高时窗口照样要变高，只是晚一拍，同样会重算 insets。
- **所有系统都一直用全高窗口**：调整高度时天然不改窗口；但这正是浮动键盘笔记否掉的方案，停靠布局的风险会落到所有用户身上。只在调整高度这个临时模式里铺满。
- **调整期间让应用边界跟着键区实时走、只是不改窗口高度**：应用仍会每次拖动都重排，录屏里应用跟着跳的问题还在，所以固定在打开时的边界，完成后一次让出新高度。

## Consequences

- 收益：拖动高度条时应用不重排、不闪白，键区在窗口里连续伸缩；完成或取消时只发生一次窗口改高。
- 代价：调整期间键区变高会盖住应用底部，用户在完成前看不到应用按新高度让出的空间；变矮时键区上方露出的是应用窗口自己的底色。打开、结束各一次扩窗和缩窗与全文预览相同，依赖系统 `input_method.xml` 的结构。
- 验证：`KeyboardGeometrySmoke` 钉住 `adjustingInsetTop` 的换算与未记下时的退回，由 `platforms/android/check-host.sh` 运行。`KeyboardHeightDeviceSmoke` 新增一段：经合成编辑器探针读应用窗口可见区下沿，预览 130%、80%、120% 时它不变、编辑器保持焦点、键帽确实变高，点「完成」后下沿上移，「取消」后回到原边界，最后重置回 100%。设备套件需要模拟器或真机，本次只做了编译，没有在设备上运行，也没有录屏确认白帧消失。
