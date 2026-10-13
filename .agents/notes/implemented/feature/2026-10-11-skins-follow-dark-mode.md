# Agent Note: 固定明暗的主题只在自己那种明暗下提供

Status: implemented

## Problem

鸿蒙、Android 和 iOS 的键盘皮肤面板和设置里的皮肤页，不管系统是浅色还是深色，都把五个内置主题全部列出来；Android 和 iOS 还让固定明暗的主题盖过系统明暗，把整个键盘拉到主题自己的明暗。浅色模式下选了「水杉」「夜」「墨」这类深色主题，键盘是黑的，周围的应用、工具栏、候选栏和设置页是白的，风格很割裂；深色模式下选「浅色」「纸白」也一样。

Android 输入法窗口可能在系统深浅切换时保留。原来配置回调只重算键盘皮肤，候选栏、表情、手写仍保留旧颜色；已经打开的皮肤选择列表也没有重新按当前明暗筛选。实时偏好更新同样漏掉表情与手写，因此在应用里修改颜色模式时，打开的表情面板仍停在旧配色。

## Decision

两种明暗对称处理：

- 主题目录里带 `appearance` 的主题只在对应的明暗下提供：浅色模式只列 `light`、`paper`，深色模式只列 `shuishan`、`night`、`ink`。`system` 和 `custom` 的 `appearance` 为空，两种模式都列。
- 已选的主题属于另一种明暗时，按 `system` 画，面板和网格里也是「跟随系统」显示为选中。所选的值不改写：切回原来的明暗，原来的主题照旧生效。
- 只过滤目录里的内置主题。社区和本机的皮肤包是用户自己装的，两种明暗下都列出：最初也按 `skinDrawsIn(base, dark)` 过滤过，结果浅色模式下 base 是深色主题的社区皮肤全不见了，用户以为皮肤丢了，所以撤掉。
- 鸿蒙键盘侧由 `GlobalTheme.fitsMode` / `GlobalTheme.effectiveId` 决定，`KeyboardSession.resolveTheme`、`keyboardSkin`、`themeSkins` 都经过它；设置页侧在 `skinGridCards` 里做同样的判断。
- Android 的实时偏好应用 `applyPreferencesSnapshot` 同时重算表情与手写，偏好轮询也比较这两个表面的实际配色键；只改面板自己的深浅时，即使键盘和候选栏没有变化，也会重画。颜色模式与面板覆盖实时生效。
- Android 的 `onConfigurationChanged` 用回调的 `Configuration.uiMode` 同时重算键盘、候选栏、表情和手写皮肤，重新着色现有视图；皮肤面板打开时重新构建选择列表。用户显式指定的面板 `light` / `dark` 仍优先于系统配置。
- Android 由 `KeyboardSkin.fitsMode` / `KeyboardSkin.effectiveTheme` 决定：键盘的 `MSIMEInputService.surfaceSkin`（键盘、候选条、表情和手写面板都经过它，各按自己那块面板的明暗）、键盘里的皮肤面板 `ImePanels.renderSkinPicker`、设置侧的 `HostStore.keyboardSkin` 和皮肤页 `SkinsPage`。
- iOS 由 `GlobalThemeCatalog.fits` / `ids(offeredIn:)` / `effectiveId` 决定。键盘把自己的明暗交给 `KeyboardTheme.reload(_:dark:)`，于是 `KeyboardTheme.current` 本身就是实际生效的主题，七十多处读它的地方不用改；明暗变化时（`traitCollectionDidChange`）重新解析。键盘的 `overrideUserInterfaceStyle` 只按「键盘明暗」偏好设置，不再按主题的固定明暗设置：否则选了「夜间」键盘先被拉成深色，再读到深色，「夜间」就永远合适，规则形同虚设。候选条配色 `CandidatePalette.resolveTheme` 和 App 的皮肤页 `SkinSettingsView` 也按同样规则过滤和选中；App 里没有键盘自己的明暗，「键盘明暗」跟随时取应用外观，与 `CandidatePreviewCard` 相同。

## Alternatives considered

- **只在浅色模式下隐藏深色主题，深色模式照旧全列。** 最强的理由是深色模式下用浅色键盘的人少，改动面更小。不用它，是因为深色模式下的白键盘同样割裂，用户要的是两边一致。
- **选中另一种明暗的主题时，把所选值改写成 `system`。** 状态最简单。不用它，是因为系统明暗会自动切换：用户白天选了「纸白」，晚上切到深色就被永久改掉，第二天早上回不来。

## Consequences

- **收益**：键盘和周围界面的明暗始终一致；切换系统明暗不丢失所选主题。
- **代价**：想在浅色系统里用深色键盘的人做不到了，只能把「键盘明暗」设成深色，或者用「自定义」。鸿蒙在模拟器上验证了浅色模式（面板只列跟随系统、浅色、纸白、自定义），深色模式只有单元测试覆盖；Android 的 `ThemeModeDeviceSmoke` 在专用 AVD 上用 light → dark → light 检查键盘与首选候选的实际像素颜色，并检查切换后选择列表只提供对应的「纸白」或「夜青」，以及表情面板实时跟随应用颜色模式、独立的表情明暗覆盖。旧版在实时改成深色时表情面板仍为浅色，新版整套通过。API 35 专用 AVD、原生 APK 构建、Android 宿主检查和 quick 均通过；截图只统计合成控件，不落盘。iOS 仍只有模拟器单元测试，鸿蒙深色仍只有单元测试。
