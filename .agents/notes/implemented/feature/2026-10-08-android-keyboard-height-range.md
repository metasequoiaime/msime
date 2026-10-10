# Agent Note: Android 键盘高度放宽到 160% 并显示实际高度

Status: implemented

「123 / #+= 层的底栏固定 46 dp、不分摊高度调整」这一条由 [底行默认与字母行同高](2026-10-10-android-equal-bottom-row.md) 部分取代：底行现在和键行同高（52 dp 加一份行距），分到四行均分后的那一份调整。当时防的压扁来自底栏拿了整份调整（75% 时压成 0 dp），不是来自均分。75%–160% 的范围、窗口封顶、实际高度读数仍按本篇。

## Problem

#5564：竖屏手机上把键盘高度调到上限 130% 后，整块键盘只占屏幕的四成多，单手握持时拇指要往下够，报告人希望能调到屏幕一半；评论里另建议调整时看到实际高度，而不是只有百分比。

追查时发现上限其实比 130% 还低：设置存的是设计范围的 dp 调整量（−46…55），但 `KeyboardGeometry.adjustedRowHeight` 按共享偏好 `touch_keyboard_height_adjustment` 的 −12…48 钳制，126%–130% 与 75%–93% 画出来都一样高，滑块两端是死区。另外 123 / #+= 层自带的底栏挂了高度角色，整份调整量在三行键之外又加了一遍，调高时这一层比字母层高出一截；范围放宽到 −46 后，75% 会把这条底栏压成 0 高。

## Decision

- 百分比范围是 75%–160%，存为 dp 调整量 −46…110（`KeyboardGeometry.MAX_HEIGHT_PERCENT` / `MAX_DESIGN_HEIGHT_ADJUSTMENT_DP`，`AndroidLocalSettings.HEIGHT_ADJUSTMENT_MAX` 引用同一常量），仍只在本地设置 `platform.android.keyboard_height_adjustment`，共享偏好的 −12…48 不变。
- `adjustedRowHeight` 按设计范围钳制，设置里的每一档都画出来。
- 130%（55 dp）以内在任何窗口里照画；更高的部分由 `KeyboardGeometry.windowHeightAdjustment` 封顶到窗口高度（`Configuration.screenHeightDp`）的 14%，且不低于 55 dp：约 800 dp 高的竖屏手机能用满 160%，横屏手机止于 130%。封顶只影响绘制，存着的值不变，旋转回来自动恢复。`ImeStyler.applyKeyboardHeight` 整棵树用同一个封顶后的值。
- 内联高度条的说明是「上下拖动调整 · N% · H px」，H 是键盘外框（`keyboardSurface`）此刻的布局高度，由外框的布局回调传给 `InlineHeightBar.setHeightPixels`，拖动预览、旋转和封顶都会反映出来；说明比拖动柄宽时整体缩小字号。
- 123 / #+= 层的底栏不挂高度角色，与功能行一样固定 46 dp。

## Alternatives considered

- **按测得的键盘自然高度精确封顶到「窗口一半」**：最贴近报告人的说法，但自然高度随布局、工具栏是否隐藏、候选释义行而变，只能在布局后量出来再回头改行高，多一轮布局，切换布局时会闪一帧，还要防止来回振荡。按窗口高度的固定比例封顶是确定的，竖屏手机上效果相同。
- **只放宽上限、不封顶**：最简单；但横屏手机窗口只有 360–410 dp，130% 时键盘已经快顶满，再高会把工具栏顶出窗口。
- **把百分比换成像素（评论原话）**：像素随屏幕密度变，同一份设置在不同设备上意义不同，设置页滑块也量不到键盘窗口；保留百分比作为设置的单位，像素只作为调整时的实测读数。

## Consequences

- 收益：竖屏手机上能把键盘调到屏幕一半以上；滑块每一档都有效；调整时看得到实际高度。
- 代价：设置页（宿主设置 App 的「键盘」页）的滑块只显示百分比，因为键盘窗口不在那个进程里，量不到实际高度；窗口不够高时 130% 以上的档位画出来一样高，内联条上的像素读数会停住。降级到旧版本时，本地存的 56…110 超出旧范围会被丢弃，回到共享偏好或 100%。
- 验证：`KeyboardGeometrySmoke` 钉住 75–160 的换算、窗口封顶和行高钳制；`ms_w2_kb_ViewLogicSmoke` 钉住内联条的范围、文案和缩字；`AndroidLocalSettingsSmoke` 钉住本地设置的新范围。均由 `platforms/android/check-host.sh` 运行。真机上的手感与 123 层底栏高度没有在设备上验收。
