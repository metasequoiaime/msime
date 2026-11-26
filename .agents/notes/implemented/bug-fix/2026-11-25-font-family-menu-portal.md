# Agent Note: 字体下拉菜单挂到设置外壳上，不被所在卡片裁切

Status: implemented

## Problem

#11889：macOS 设置应用「候选窗口 › 字体与大小」里，「英文字体」的下拉菜单点开后只露出一两项，其余被挡住。

原因在共享设置页：`FontFamilyInput`（`packages/ui/src/candidate/font-family-input.tsx`）的 `.font-family-menu` 以 `position: absolute` 挂在输入框下面，而每组设置行的卡片 `groupRows`（`packages/ui/src/core/platform-controls-style.ts`）带 `overflow-hidden`，圆角和分隔线都靠这层裁切。「英文字体」是「字体与大小」组的最后几行，菜单超出卡片底边的部分全被裁掉。主字体同样会被裁，只是它在组的上部，下面还有几行能露出来。

## Decision

菜单经 `createPortal` 挂到最近的 `[data-settings-shell]` 上（没有外壳时挂到 `document.body`），用 `position: fixed` 按输入框在视口里的位置定位，宽度与输入框相同，最大高度仍是 240px。输入框下方放不下而上方更宽裕时，菜单贴着输入框上沿往上弹（`translate: 0 -100%`），最大高度取那一侧的可用空间。做法与同一套设置页里的 AI 服务商菜单（`settings/ai-provider-select.tsx`）一致。

菜单不再随页面滚动，所以页面或任一外层滚动容器滚动、窗口改变大小时收起菜单；菜单自身滚动不收起。键盘移动高亮时只滚动菜单自身，不再用 `scrollIntoView`：它会连外层页面一起滚，正好触发上面的收起。

失焦收起、点选项前 `onPointerDown` 阻止失焦、组合输入按键交给输入法（[字体下拉导航让组合输入先处理按键](2026-10-14-font-family-ime-navigation.md)）都不变。菜单仍在设置外壳里，[设置浮层厚玻璃](2026-10-19-settings-glass-overlay-surface.md) 对 `.font-family-menu` 的豁免理由（近乎不透明的 `--dropdown-menu-bg`）照旧成立。

## Alternatives considered

- **去掉 `groupRows` 的 `overflow-hidden`**：改动最小，所有挂在卡片里的弹层一次解决。不用它：卡片的圆角、液态玻璃背景和 HarmonyOS 的分隔线都靠这层裁切，所有设置页的卡片都会受影响，范围远大于这个缺陷。
- **只对「英文字体」所在的组放开裁切**：同样能让菜单露出来，但菜单仍会被更外层的滚动容器和窗口底边截断；窗口矮时依旧看不全，也解决不了往上弹。

## Consequences

- 菜单打开时滚动页面会收起菜单，与 AI 服务商菜单、「?」说明气泡的行为一致。
- 菜单位置只在打开和选项数变化时量一次；输入框在菜单开着时被别的布局变化挪动（不经滚动或改窗口大小）的话，菜单不会跟着动。

## Verification

- `apps/desktop/tests/candidate/font-family-input.test.tsx` 新增三条：菜单不在所在卡片里、挂在设置外壳上并按输入框位置 fixed 定位；下方放不下时往上弹；页面滚动收起、菜单自身滚动不收起。三条在改动前的实现上都失败。
- `pnpm --filter @msime/desktop test` 全部通过，`typecheck`、`lint` 通过。
- 用 `scripts/build-settings-browser.mjs` 的合成设置页（macOS 外观、30 个合成字体）在 Chrome 无头模式下截了改前改后：改前菜单只露出卡片底边下的一条细边，改后完整显示并向上弹出。没有在打包的 `MSIME.app` 里验收。
