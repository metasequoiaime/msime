# Agent Note: Windows 原生设置窗口自己画「屏幕键盘」页

Status: implemented

## Problem

原生设置窗口的「屏幕键盘」页只有两行：打开屏幕键盘面板，和「屏幕键盘设置」，后者用 `settings:screen-keyboard` 拉起共享 Tauri 应用。整项工作的目标是任何设置都在原生窗口里改，面板（屏幕键盘、手写、云词库）是输入工具，仍由共享应用显示。共享页（`packages/ui/src/settings/pages/screen-keyboard-page.tsx` 和 `touch-keyboard-geometry-section.tsx`）在 Windows 上画四组：「屏幕键盘」（打开面板和一张可拖动调间距的预览）、「尺寸」（键盘高度 -12 到 48、按键间距 30 到 60、行间距 40 到 100、恢复默认）、「工具栏」（只有顶部语音入口，Windows 不是 `touch_toolbar_components` 的宿主）和「外观」（指向「主题」页）。「布局」「手势」和触屏反馈只在手机宿主上画。

## Decision

- `build_osk_page` 按共享页重写，文案、范围、回落值（0、60、70、关）和显示格式（带符号的 dp、一位小数的 dp）照搬。滑块像其他滑块一样先改内存里的文档，停下 350 ms 后保存。
- 「恢复默认」照 `useTouchKeyboardSettingsReset`：确认框「恢复屏幕键盘默认值」，确认后从文档里删掉 `touch_key_spacing_tenths`、`touch_row_spacing_tenths`、`touch_keyboard_height_adjustment`、`touch_voice_shortcut` 和 `touch_toolbar`，不写入当前默认值，保存成功后提示「屏幕键盘设置已恢复默认。」。
- 预览照 `ScreenKeyboardPreview` 的桌面布局用 WinUI 的 `Canvas` 和 `Path` 画，1100 单位宽，经 `Viewbox` 最宽缩到 760。明暗跟「屏幕键盘主题」再跟颜色模式；配色取 `msime_client_theme_catalog` 的 `keyboard`（`load_catalog` 现在也读它），跟随系统和没有调色板的主题用共享预览的系统键盘色；自定义主题带键盘设计时按设计画颜色、圆角、键形（圆角、胶囊、票根、卵石，路径同 `keyboardKeyPath`）、材质、描边、渐变、三种底纹和照片。
- 拖动照 `useTouchKeyboardGeometryDrag`：按下时记住两个间距，先移动 4 像素决定方向，左右调键距、上下调行距，每 1.8 像素一个十分之一 dp，按 `Math.round` 取整并夹在范围内。拖动面上 `ManipulationMode` 设为平移，触控拖动不会被滚动视图当成翻页；拖动时对应的滑块跟着移动。
- 「外观」组的说明写「界面主题」而不是共享页的「高级」，因为本窗口「主题」页上同一组叫「界面主题」；「前往」直接落到「屏幕键盘主题」一行。
- 删掉 `shell_links::screen_keyboard`，`all` 从 8 项减为 7 项；`screen-keyboard` 路由仍打开原生「屏幕键盘」页。README 同步。

## Alternatives considered

- **用静态图片或只画键位轮廓做预览，不支持拖动** — 省事；但共享页的说明文字就写着可以在预览上拖动，静态图要么改文案、要么说一句做不到的话。
- **把共享预览的 SVG 交给 WebView2 画** — 和共享页逐像素一致；但设置窗口没有 WebView2 依赖，只为一张预览引进一个浏览器进程不值得。
- **在 host-api 里算好键位矩形再交给窗口画** — 布局只有一处；但这些数字只服务一张预览，共享页的布局也是 TypeScript 里写死的常量，Rust 里没有对应的东西可复用。

## Consequences

- 原生窗口不再为屏幕键盘设置拉起共享应用；打开屏幕键盘面板仍然拉起。
- 键位、配色常量和设计字段的读法写死在 `main.cpp`，`screen-keyboard-preview.tsx`、`keyboard-layouts.ts` 或 `keyboard-shape.ts` 改了要同步改这里。
- 键盘设计的投影（`shadow`）没有画：WinUI 的形状没有现成的模糊阴影，用合成层给每个键加阴影代价太大。底纹在背景的 8 单位圆角外也会画出来，共享预览会把它裁成圆角。在 Windows 上只有从手机同步来的自定义主题才带键盘设计，这两处影响有限。

## Verification

- 设置工程按 `/WX` 编译通过（`settings-test-build.ps1` 输出 `SETTINGS_BUILD_OK`）。
- `tests/ui/settings_navigation.cpp` 用 `cl /std:c++20 /W4 /WX` 单独编译并运行通过。
- 没有启动窗口：预览的实际样子、鼠标与触控拖动、滑块联动、恢复默认的确认框和写入效果都未在运行中的窗口里检查。
