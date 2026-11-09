# Agent Note: 主题轮播卡片统一高度

Status: implemented

## Problem

「主题」页的轮播从 #8837 起让高度跟随当前卡片的自然高度（见[本地皮肤统一](../../implemented/simplification/2026-10-18-local-skins-unified.md)）。皮肤之间的预览高度差别很大：带装饰图的皮肤多出一条装饰带，竖排候选、画布皮肤又比内置主题高。于是每翻一张，卡片底边和下面的导航行（‹、圆点、n / N、›、「回到当前」）都跟着上下跳，连续点 › 时按钮从指针下面跑开。维护者 2026-11-09 在会话里贴了三张截图指出这个问题，并明确否定了「高度跟随当前卡片」这个决定。Windows WinUI 主题页的 FlipView 照着共享页做，同样按当前卡片设高度，问题相同。

## Decision

轮播里所有卡片等高，取最高那张在当前宽度下的自然高度；多出来的高度给卡片内部的预览舞台，卡片外面不留空白。

- 共享页（macOS、Linux 的 Tauri 设置和 HarmonyOS PC 的 ArkWeb 设置）：`settings-style.ts` 的 `themeCarouselTrack` 去掉 `items-start`，flex 默认的 stretch 让每个 slide 拉到最高那张的高度；`themeCarouselSlide` 是纵向 flex，把直接子元素 `article` 撑满，`article` 里带 `data-skin-preview` 的预览面 `flex-1` 吃掉剩余高度。`theme-carousel.tsx` 删掉逐张测量高度的 state 和 effect，不再写内联高度；宽度变化时按当前卡片重新定位的 effect 保留。样式只经 slide 的子选择器生效，`skinCard` 本身不变，屏幕键盘皮肤区等其他用到它的地方不受影响。
- Windows（`platforms/windows/settings/main.cpp`）：`make_skin_card_frame` 的卡片改成 `VerticalAlignment::Stretch`，卡体从 StackPanel 换成 Auto / Star / Auto 三行的 Grid，预览面放在 Star 行；`add_theme_carousel` 的 `sync` 在这个宽度下量全部卡片，FlipView 高度取最大值。两处卡片构造函数（`make_theme_card`、`make_skin_card`）只用于轮播。

导航行留在卡片下方。它的位置只取决于最高那张卡片，装上或删掉皮肤、窗口改宽时才会变。

## Alternatives considered

- **保持跟随当前卡片，把导航行移到卡片上方**：导航行的位置只由上面固定高度的「明暗」分组决定，翻页时不动，也没有任何留白，改动更小。但卡片本身和下面的设置仍然每翻一张就伸缩，维护者要的是高度一致，不只是按钮不跑。
- **所有卡片等高，但卡片保持自然高度、空白留在卡片外**：#8837 之前就是这样，短预览下面出现一块与卡片分离的空白，像布局坏了，这正是 #8837 要修的问题。把空白放进卡片内部的预览舞台，看起来就是统一尺寸的卡片。
- **只挂载当前卡片**：[本地皮肤统一](../../implemented/simplification/2026-10-18-local-skins-unified.md)已否决，会丢掉横向滑动和其他卡片的预览状态。

## Consequences

- 收益：翻页时卡片外框、导航行和下面的设置都不再移动，「回到当前」与箭头一直停在同一处（[回到当前](../../implemented/feature/2026-11-03-settings-skin-delete-and-back-to-current.md)要求它始终占位、不左右跳，这次补上了上下方向）。共享页去掉了逐张测量和对应的尺寸订阅，少一个 effect。
- 代价：内置主题这类矮卡片的预览舞台下面多出一段空的舞台底色；装了一款特别高的皮肤（比如大尺寸画布皮肤）会把所有卡片一起撑高，后面的设置被推远。Windows 每次选中项或宽度变化都量一遍全部卡片，卡片数在几十张以内，量的是已建好的元素，开销可以接受。
- 验证：`apps/desktop/tests/settings/theme-carousel.test.tsx` 原来断言轨道高度跟随当前卡片，改为断言切换和滑动都不写内联高度、轨道不是 `items-start`、slide 把卡片撑满；轮播的 12 项测试通过。用 `scripts/build-settings-browser.mjs` 的合成夹具在 Chromium（1000×1400 视口）里逐张翻 8 张卡片：改动前导航行的 y 在 742–816 px 之间跳，卡片高 432–506；改动后 8 张卡片都是 506，导航行固定在 816。Windows 由 PR 上的 Windows 设置编译作业编译，没有在 Windows 上目测，也没有在真实的 macOS 设置应用里截图。
