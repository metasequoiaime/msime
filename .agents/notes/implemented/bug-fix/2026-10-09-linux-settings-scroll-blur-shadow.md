# Agent Note: Linux 设置页去掉分组与皮肤卡片的模糊阴影

Status: implemented

## Problem

issue #6403：deepin 25.2.2 上，Linux 设置窗口右侧内容区用鼠标滚轮上下滚动不跟手，任意设置页都这样。

设置启动器 `msime-linux-settings` 默认导出 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 和 `WEBKIT_DISABLE_DMABUF_RENDERER=1`（见 [Linux 设置窗口默认禁用 WebKit DMA-BUF 渲染器](2026-10-08-linux-webkit-dmabuf-renderer.md)）。两个变量各自都会让 WebKitGTK 退回非合成模式：后者让 `AcceleratedBackingStore::rendererBufferTransportMode()` 一种传输方式都不报，硬件加速随之不可用。非合成模式下，内容区 `#settings-content` 是 `overflow-y: auto` 的元素，不是整页滚动，WebKit 没法用整页滚动的位块平移，每滚一步就把整块可见内容在 CPU 上重画一遍。

Linux 的分组容器（`--p-group-shadow`）带一层 `0 1px 3px` 的模糊阴影，皮肤卡片带 `shadow-card`（两层模糊阴影）。带模糊的阴影每次重绘都要对整个分组的面积重新做一遍模糊，一页十几个分组，重绘成本就由它们决定。

在 Debian 12 容器里（WebKitGTK 2.50.6、Xvfb、两个变量都设成 1，内容区 860×760），用合成数据的设置页逐步滚动内容区，测量每一步到下一帧的耗时：

- 改动前，10 个高于一屏的页面 p50 都在 51–62ms。
- 只把分组阴影换成不带模糊的版本，「输入」页从约 100ms 降到 18ms；1px、2px 的模糊半径分别是 90ms、51ms，只要带模糊就慢。
- 再去掉皮肤卡片的模糊阴影，「主题」页从 23ms（p90 约 55ms）降到 17ms。
- 两处都改完，10 个页面的 p50 都在 9–17ms，被 60Hz 帧间隔封顶。

## Decision

Linux 设置页不用带模糊的大面积阴影：

- `platform-tokens.ts` 和 `styles.css` 里 Linux 浅色、深色两套 `--p-group-shadow` 的下沿阴影由 `0 1px 3px rgba(0,0,0,.07)` 改为 `0 1px 0 rgba(0,0,0,.07)`，外圈 1px 描边不变。
- `settings-style.ts` 的 `skinCard` 在 Linux 下未选中时 `linux:shadow-none`，选中时只保留强调色外圈 `linux:shadow-[0_0_0_1px_var(--accent-color)]`；卡片本身的边框不变。
- `platform-tokens.test.ts` 新增用例：Linux 分组阴影每一层的模糊半径都必须是 0。
- `platforms/linux/README.md` 在启动器默认变量那一段写明这条约束。

其他平台的阴影值不变：`--p-group-shadow` 只改了 `[data-platform="linux"]` 的两个块，`skinCard` 只加了 `linux:` 变体。

## Alternatives considered

- 去掉启动器的 `WEBKIT_DISABLE_COMPOSITING_MODE`，让 WebKit 回到合成模式。理由最强的一条：合成模式下滚动由合成器平移图层，页面里放什么阴影都不用每步重画，这是根上的解法。没采用：同一个启动器还设着 `WEBKIT_DISABLE_DMABUF_RENDERER=1`，当前 WebKit 里它单独就会让硬件加速不可用，只删前者不起作用；两个一起删，就把 #3273 和 #5834 规避的 NVIDIA 下启动即退出（Error 71、GBM 分配失败）带回来，而我们手上没有能复测的 NVIDIA 机器。
- 把 `WEBKIT_DISABLE_DMABUF_RENDERER` 换成 `WEBKIT_DMABUF_RENDERER_FORCE_SHM=1`，保留合成、只把缓冲改走共享内存。理由：可能同时保住 NVIDIA 下能启动和合成模式下的滚动。没采用：#5834 的崩溃在哪一步触发没有定位到，这个替换能否规避它只能在出问题的机器上验证，不能拿来猜。
- 关掉 WebKitGTK 的平滑滚动（`set_enable_smooth_scrolling(false)`）。理由：平滑滚动把一格滚轮拆成多帧，每帧都要重绘。没采用：每帧的重绘成本不变，只是帧数少了，每一步仍要 50ms 以上；量出来的大头是阴影的模糊，改掉它之后平滑滚动的每一帧都在一个帧间隔内画完。
- 把内容区改成整页滚动，让 WebKit 走位块平移。理由：非合成模式下整页滚动只需补画新露出的一条。没采用：要改共享设置页的整体布局（侧栏、标题栏都要改成固定定位），影响所有平台；改成固定定位之后，侧栏和标题栏在每一步整页滚动时也要重画。

## Consequences

- Linux 设置页分组的下沿由 3px 模糊的淡阴影变成 1px 不带模糊的淡阴影，皮肤卡片在 Linux 下只剩边框；截图对比肉眼几乎分不出来。换来的是十个高于一屏的页面每步滚动都在一帧内画完。
- 往 Linux 设置页加带模糊的 `box-shadow`、`filter: blur` 或 `backdrop-filter` 会把这个问题带回来。新用例只看分组阴影这一个令牌，其他元素靠 README 和这篇笔记提醒；判据是在关掉合成的 WebKitGTK 里量滚动每步的耗时。
- 皮肤预览里候选窗、悬浮条自带的阴影（`.container`、`.status-bar`）没动：它们是候选窗外观的一部分，量出来对滚动的影响在帧间隔以内。
- 如果以后去掉了启动器的两个变量，回到合成模式，这里的约束就可以放松，届时更新这篇笔记。

## Verification

- `apps/desktop` 下 `pnpm exec vitest run tests/core/platform-tokens.test.ts tests/core/platform-controls.test.tsx` 通过；把 Linux 分组阴影改回 `0 1px 3px`，新用例失败。
- 用 `scripts/build-settings-browser.mjs` 打出改动前后两份合成数据的设置页，在 Debian 12 容器（WebKitGTK 2.50.6、Xvfb、两个变量都设成 1）里逐页量滚动，数字见 Problem 一节。
- 没有在 deepin 25 上验证，也没有在真实 GPU 和高分屏上量过。
