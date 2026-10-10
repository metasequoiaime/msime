# Agent Note: 手机设置页按宿主画键盘预览，并收起电脑上的皮肤选项

Status: implemented

## Problem

共享设置页在 Android、iOS、HarmonyOS 手机上显示了一批只属于电脑的东西。

- `ScreenKeyboardPreview` 没写 `layout` 时画 Windows 式屏幕键盘（数字行、Tab、Caps Lock、Ctrl、Win、Alt）。设置页里只有缩略图和桌面主页指定了布局，社区皮肤详情、发布对话框、社区卡片、键盘页预览、皮肤轮播、「我的皮肤」卡片、皮肤编辑器和 AI 设计皮肤的提案共八处，在手机上一直画电脑键盘。
- 皮肤页「更多选项」里有「外部皮肤」一行：它导入的是候选窗口的皮肤文件夹（`skin.toml` 加图片），导入型宿主还露出应用沙箱里的内部路径（如 `/data/storage/el2/...`）。
- 同一处的七个候选颜色取色器作用于候选窗配色，而手机的触屏候选栏大多画键盘皮肤的颜色，改了看不到效果。
- 从「我的 → 社区作品 → 我的设计」进入皮肤页时编辑器已经打开，但它在默认收起的「更多选项」里，用户只看到皮肤网格。

## Decision

- 屏幕键盘布局是宿主级的默认契约：`packages/ui/src/keyboard/screen-keyboard-preview.tsx` 导出 `ScreenKeyboardLayoutContext` 和 `screenKeyboardLayoutFor(platform)`，`SettingsPage` 在根部按 `client.host.platform` 提供它，`android`、`ios`、`harmony` 为 `touch`，其余为 `desktop`。`ScreenKeyboardPreview` 的 `layout` 省略时取这个上下文；显式传入的 `layout`（如桌面主页的 `layout="desktop"`）和 `thumbnail` 仍优先。HarmonyOS 2in1 的宿主平台同样是 `harmony`，它的输入法也画触屏键盘，所以按平台而不是按 `mobile_settings` 判定。
- 触屏布局的预览画布为 700 宽，单键接近手机上的竖长方形；桌面布局仍是 1100 宽，桌面插图不变。
- `mobile_settings` 宿主（手机）不显示 `ExternalSkinDirectoryRow`。HarmonyOS 2in1 等桌面形态不受影响，导入能力（HarmonyOS 的 `skin_directory_import`、iOS 的 `import_picked_skin`）的后端代码保留，2in1 仍在用。
- 能用「导入皮肤」的宿主即使显示这一行，也不再显示皮肤目录路径：那是沙箱内部路径，用户既看不到也无法手动复制文件进去。
- 手机上候选颜色取色器只在 `candidatePaletteFollowsDesktop === true` 时显示。逐个宿主核对过谁读这些颜色：HarmonyOS 手机的候选栏取键盘调色板，只有 2in1 用候选配色（`KeyboardView.ets` 的 `candidateColors()`）；Android 的候选栏不读解析出的候选配色；只有 iOS 在打开「候选栏使用主题配色」后才读，而这个字段也只有 iOS 宿主提供。桌面照旧显示。皮肤页因此不再渲染 `CandidatePaletteFallbackNotice`，组件本身仍从 `packages/ui` 导出。
- 打开了触屏皮肤编辑器时，「更多选项」以 `defaultOpen` 展开。从账户页进入时皮肤页会重新挂载，`defaultOpen` 在 `showTouchSkinEditor` 已为真时读取。

## Alternatives considered

- 在八个调用点各自传 `layout`：最直白，每处一眼可见。但新加的预览默认仍会画电脑键盘，同样的缺陷会随下一个调用点回来；宿主平台在设置页根部就已知，由上下文提供一次即可，个别需要固定布局的地方（桌面主页）继续显式传。
- 手机上保留外部皮肤一行，只在 iOS 打开「候选栏使用主题配色」时显示，与取色器同一个条件：iOS 键盘在这个开关打开时确实画外部候选皮肤。没有采用，因为这一行要求用户事先在文件系统里整理好含 `skin.toml` 的文件夹，手机用户几乎没有这样的来源，手机的皮肤入口是社区图库；需要的用户可以在电脑上做好后发布到社区再取用。
- 保留取色器并继续显示「候选颜色在这里不生效」的提示：用户仍会面对七个改了没效果的控件，提示只是解释它们为什么没用，不如在不生效时不显示。

## Consequences

- 手机设置页里所有未指定布局的键盘预览都画触屏键盘，新加的预览默认跟随宿主；桌面输出逐字节不变。所有 `ScreenKeyboardPreview` 都在 `SettingsPage` 之下，脱离它单独渲染时回到上下文默认的 `desktop`。
- 手机用户失去从文件夹导入候选窗口皮肤的入口；若日后要在手机上恢复，需要同时给出手机上可行的皮肤来源，而不只是把这一行放回来。
- Android 和 HarmonyOS 手机上不再出现候选颜色取色器；如果某个手机宿主日后让候选栏读候选配色，它要像 iOS 一样提供 `candidatePaletteFollowsDesktop`，取色器才会出现。

## Verification

`apps/desktop/tests` 下新增或改写的用例覆盖：鸿蒙与 Android 键盘页预览没有 Caps Lock、Win、Tab，Windows 仍有（去掉上下文后这三条失败）；触屏布局下社区皮肤详情没有 Caps Lock、Win；鸿蒙、Android、iOS 手机没有外部皮肤一行，鸿蒙 2in1 仍提供导入皮肤；手机上候选颜色只在开关打开时出现，Android 不显示；导入型宿主不显示目录。另在 HarmonyOS 模拟器上用本分支的 HAP 看过社区皮肤详情、键盘设置、我的皮肤卡片和编辑器。
