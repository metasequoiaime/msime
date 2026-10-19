# Agent Note: 设置页浮层在液态玻璃下使用厚玻璃表面

Status: implemented

## Problem

macOS 设置窗口启用液态玻璃后，外壳把正文卡片用到的底色（`--surface-raised`、`--card-bg`）换成 7.5%–25% 的白色薄层，窗口背景也改为透明。对话框面板直接用这些底色（`bg-raised`）或本身就半透明的 `--dropdown-bg`，浮在页面上时，背后的皮肤预览、看板娘、候选条和分页按钮透进对话框，「发布候选窗口皮肤」这类表单几乎无法阅读。

#8837 新增 `settings-modal-surface` 与对应的厚玻璃规则（`--p-bg` 90% 混合、24px 模糊，减少透明度时退回实色，强制颜色时用 `Canvas`），但只在 `community.dialog` 与 `account.modal` 两个样式常量上加了这个类。问题的根源是约定靠逐个常量手工加入：没有任何检查告诉下一个浮层要带上它。用户截图里的设置应用是 0.52.0，早于 #8837，所以截图中所有对话框都透底。

## Decision

所有浮在设置页上的层都通过共享样式常量带上 `settings-modal-surface`，样式表里只有一条厚玻璃规则及其两种回退，由它决定 macOS 玻璃下的表面；其他平台和非玻璃窗口不受影响，因为规则限定在 `html[data-macos-liquid-glass="true"] [data-settings-shell][data-platform="mac"]` 之下。

逐个核对 `packages/ui/src` 里的浮层后：

- 已经带类、无需改动：`community.dialog`（社区对话框框架、候选皮肤发布、`core/confirm` 确认框、脚本插件、AI 皮肤抽卡的对话框形态、HarmonyOS 手机词库）与 `account.modal`（桌面编辑资料）。在 develop 上用 macOS 外壳的浏览器预览截图确认，发布对话框、确认框和编辑资料在深浅两色下都不透底。
- 本次补上：社区筛选范围的折叠菜单 `community.scopeMenuList`。窗口窄于 561px 时它取代三个按钮，用 85% 的 `--dropdown-bg` 浮在页面上，深色下背后的标签文字透进菜单；补类后不再透出。
- 本次一并补上的共享常量：`fullScreenSurface`（社区图库全屏子视图、HarmonyOS 手机的语音服务配置、触屏皮肤编辑器）和 `account.sheet`（登录与昵称底部面板）。这几处目前只在手机和 HarmonyOS 上出现，本身用不透明的 `--p-bg`，在 macOS 上不会渲染；加类让「浮层都带厚玻璃」成为不分平台的约定，将来接到桌面时不必再记得这一步。

不加厚玻璃的面板在测试里登记理由：

- 嵌在页面流或对话框内部的确认卡片（卸载确认、账号操作确认、社区删除与举报确认）。它们不浮在别的内容之上：在页面里时背后只有窗口材质，与同页的分组卡片同一层；在对话框里时已经落在对话框的厚玻璃上。改成厚玻璃会让页面里的卡片看起来像弹窗，却不是模态。卸载确认带了 `aria-modal="true"`，是唯一需要豁免的一个。
- 只在手机上出现的面板：`core/action-sheet`、`keyboard-switch-guide`；以及键盘面板窗口里的确认（`keyboard/panels.tsx`），它不在设置外壳里，玻璃规则本就不作用于它。
- 弹出列表：`.font-family-menu` 用近乎不透明的 `--dropdown-menu-bg`，玻璃外壳不改它；`.ai-provider-menu` 自带厚玻璃规则和两种回退；统计页的溢出菜单只在 HarmonyOS 和手机外观里出现；设置搜索结果就地替换侧栏列表，不浮在页面上。

`apps/desktop/tests/settings/glass-overlay-surface.test.tsx` 守住这条约定：

- 扫描 `packages/ui/src` 全部组件源码，每个 `aria-modal="true"` 的开始标签取出 `className`，把引用的样式常量解析成实际类名，要求含 `settings-modal-surface`；豁免文件按 `aria-modal` 个数登记，个数变了就重新判断。
- 所有带 `role="menu"` 或 `role="listbox"` 的文件必须与登记表一致，新增弹出列表要么带类，要么写明理由。
- 校验样式表里顶层规则、减少透明度和强制颜色三条规则都还在。
- 在 macOS 玻璃外壳里渲染候选皮肤发布对话框、确认框和筛选菜单，断言它们命中厚玻璃规则的选择器。

## Historical audit

- [设置玻璃](../feature/2026-10-19-macos-settings-liquid-glass.md)：部分重叠。外壳把正文底色换成薄层的决定不变，本篇只约束浮在页面上的层。
- [玻璃内容归属修正](2026-10-19-macos-glass-content-containment.md)：无冲突，那里管原生玻璃的视图归属，本篇只涉及 WebView 内的 CSS 表面。

## Alternatives considered

- **按 ARIA 语义写 CSS 选择器**（`[aria-modal="true"]`、`[role="menu"]`）：新浮层只要语义正确就自动得到厚玻璃，不需要记得加类，这是它最强的理由。不用它，是因为语义与外观并不一一对应：卸载确认是页面里的卡片却带 `aria-modal`；AI 皮肤抽卡的全屏形态把 `aria-modal` 放在居中的内容列上，背景在外层；筛选菜单是 `role="group"`。按语义选会给内联卡片加上弹窗外观，又漏掉真正的浮动菜单。显式的类加上扫描测试同样不会漏，且每个例外都有写明的理由。
- **只修用户截图里的对话框**：develop 上它已经由 #8837 修好，用户看到的是旧版本。只补这一处不能阻止下一个浮层再次透底，因此补上测试。

## Consequences

新增浮层时，测试会要求它要么用带 `settings-modal-surface` 的共享样式常量，要么在登记表里写理由；用了新的样式模块，需要在测试的 `styleModules` 里登记才能解析。代价是测试按源码文本解析 `className`，只认 `ns.member`、具名导入的常量和字面量，写法过于动态的类名会被报为缺失，需要改回共享常量。

`fullScreenSurface` 与 `account.sheet` 在 macOS 玻璃下会是 90% 的 `--p-bg` 加模糊，而不是完全不透明；与对话框保持同一种材质，背后内容经 24px 模糊后不可辨认。浅色下筛选菜单从纯白变为 90% 白色加模糊，同样不透出文字。

## Verification

- 用 `scripts/build-settings-browser.mjs` 的同一套构建，以合成数据渲染 macOS 外壳的主题页（`data-macos-liquid-glass="true"`，彩色渐变模拟窗口背后的桌面），在 Chromium 中对改动前后截图：发布候选窗口皮肤、确认框、编辑资料在 develop 上即为厚玻璃；520px 宽的筛选菜单在改动前深色下透出背后的「输出设置」等标签，改动后不透出，浅色保持可读。
- 新测试在去掉类名后失败（列出 7 处缺类的面板，渲染断言同时失败），恢复后通过。`apps/desktop` 的类型检查与全部 782 个测试文件、4812 项测试通过。
- 尚未在真实 macOS 设置应用里截图；本机安装由维护者从 develop 统一构建。
