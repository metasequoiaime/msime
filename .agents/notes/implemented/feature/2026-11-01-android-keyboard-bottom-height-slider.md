# Agent Note: Android 恢复单一的「键盘底部高度」滑块

Status: implemented

## Problem

#10447（#10456 是同一位用户隔 3 分钟提交的重复，平台误选成 Windows）要求按键高度不变、把整块键区抬离屏幕底边。运行时能力一直在：`ImeBottomBar` 读 `platform.android.bottom_padding` 和 `platform.android.bottom_padding_height`（0–160 dp），按 `KeyboardBottomBarPolicy.extraPaddingDp` 垫高键区。缺的是入口，[移除底部留白设置控件](2026-10-17-remove-redundant-bottom-padding-controls.md) 把它从设置页删了。

那次删除的理由是「和自动底栏是两套互相重叠的避让机制」，漏掉了一个缺口：[键盘底栏](2026-10-08-android-keyboard-bottom-bar.md) 只在 Android 12 起、导航栏低于 32 dp、系统不画输入法按钮、非浮动、非手机横屏时出现。三键导航的设备、系统自己在手势条上画输入法按钮的设备（Pixel、AOSP 13 起）、API < 31 的设备都没有底栏，删掉入口后完全没有办法抬高底行；底栏出现时也固定 40 dp，不能调。#10447 正是这个缺口造成的回归，与当年的 #6392、#7633 是同一个诉求。

本篇部分取代（supersedes）[移除底部留白设置控件](2026-10-17-remove-redundant-bottom-padding-controls.md)：「不给入口」这一点翻转；「保留旧字段和运行时读取」照旧成立。

## Decision

键盘页「布局」组的「键盘底栏」开关下面只有一个滑块「键盘底部高度」，说明「抬高整块键盘，按键大小不变。底栏显示时，40 dp 以内由底栏占用；浮动键盘、手机横屏和接实体键盘收起键区时不生效。」，范围 `KeyboardBottomBarPolicy.MIN_PADDING_DP..MAX_PADDING_DP`（0–160 dp），步进 `PADDING_STEP_DP`（4 dp），显示单位 dp。不恢复独立的「底部留白」开关。

数值是键区底边到系统导航区之间的目标总距离，底栏画着时它的 40 dp 计在里面，这就是 `extraPaddingDp` 已有的语义 `max(0, h − (底栏显示 ? 40 : 0))`。存储沿用两个旧字段，不新增键、不改默认值、不同步：

- 显示值 = `bottom_padding` 为真时取 `bottom_padding_height`，否则 0（`AndroidLocalSettings.bottomHeightDp`）。新用户显示 0，布局与现状一致；以前打开过开关的老用户直接看到原来的高度，不需要迁移。
- 拖到 v > 0：写 `bottom_padding = true`、`bottom_padding_height = v`；拖到 0：只写 `bottom_padding = false`，高度保留原值不清除（`AndroidLocalSettings.bottomHeightEdits`）。两项在同一次原子写里落盘（`KeyboardSheets.saveLocal(Fragment, Map, Runnable, Runnable)`）。保存成功后不重建页面，只在失败时刷新，与同页其他本地设置滑块一致：`MsSlider` 对方向键和读屏调整是每一格都提交，成功也重建的话旧的 SeekBar 节点被移除，读屏用户每调一格就丢一次焦点。

运行时不变：浮动键盘、手机横屏、外接键盘收起键区三种情况照旧不垫高。浮动键盘是本页的设置项，打开时滑块置灰（同页「浮动键盘」开关切换时同步置灰或恢复）；手机横屏和外接键盘收起键区是键盘弹出时的状态，设置页判断不了，写在说明里。保存后键盘经现有的本地设置重读路径重新应用 inset。设置页的滑块没有实时预览机制，这次不新增。设置搜索给键盘页加上「键盘底部高度」「底部高度」「底部间距」「底部留白」「键盘抬高」。

旧滑块步进是 1 dp，老用户可能存着 42 这类不在 4 dp 格上的值：运行时照样按 42 垫；设置页显示时 `MsSlider.setValue` 取最近一格（44），只是打开设置页不改写存储；但用户一旦点按或拖动滑块，松手时 `onStopTrackingTouch` 无条件提交当前格，存储就对齐到 4 dp 格（42 变 44，存着 1 dp 的显示 0，一碰就关掉开关）。差别不超过 2 dp，接受。

## Alternatives considered

- **原样恢复「底部留白」开关加「底部留白高度」滑块**：最强的理由是改动最小、能直接回滚 #8562，开关语义也最直白。不用它，是因为这正是当初被删的原因：用户要同时理解「底栏」和「留白开关」两套机制，还要理解开关关着时滑块无效。单一滑块只暴露一个「目标总距离」，0 就是关，与底栏合并计算，信息量相同而概念少一个。
- **只在底栏不显示时才显示入口**：能彻底避免「底栏和留白重叠」的观感。不用它，是因为底栏出不出现取决于导航模式、系统版本和横竖屏，入口会随之时有时无，用户换一次导航方式就找不到设置，也搜不到；底栏出现时用户同样可能想要比 40 dp 更高。
- **在内联高度调整条（`KeyboardLayoutAdjustPolicy`、功能面板里的拖动条）加一个下沿拖动柄**：所见即所得，不用进设置页。不用它，是因为交互改动大得多（两个拖动柄的命中区、与整体高度缩放的区分、读屏说明都要重新设计），留给以后另开 issue。因此本篇对 #10447 只算部分解决：用户参照 Gboard 找的是内联调整条的下沿拖动柄，本篇只让这个能力从设置页可达，PR 用 `Refs #10447`，不关闭 issue。

## Consequences

- 收益：底栏不出现的设备重新有了抬高底行的手段；底栏出现时也能调到 40 dp 以上。只改 Android 设置页、设置搜索和这篇笔记，不碰共享偏好 `touch_keyboard_height_adjustment`、云同步、iOS、HarmonyOS、Windows。
- 代价：「键盘底部高度」和「键盘高度」名字相近，靠说明文字区分「按键大小不变」；老用户存着的非 4 dp 倍数在设置页上显示为最近一格。
- 跨平台不失衡：iOS 键盘扩展的 Home 指示条区域由系统在扩展外处理，这个需求在 iOS 上不成立；HarmonyOS 面板目前不读避让区 inset，也没有同类反馈，以后有反馈再单独开 issue。
- 测试：`AndroidLocalSettingsSmoke.bottomHeightSlider` 覆盖读写映射（新用户 0、老用户 true/96 显示 96、v = 0 关开关且保留高度、v > 0 开开关并写高度、每一格往返）；`KeyboardBottomBarPolicySmoke` 确认步进整除范围；设备冒烟 `BottomPaddingSettingsDeviceSmoke` 改为验收滑块的范围、写盘、拖到 0 保留高度和重开恢复，并确认没有独立的「底部留白」开关；`KeyboardHeightDeviceSmoke` 里底部留白的设备回归保留不动。
