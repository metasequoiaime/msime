# Agent Note: 关掉动画时 Android 应用内提示只显示一帧

Status: implemented

## Problem

Android 设置页的应用内提示 `MsToast` 用两段属性动画实现「淡入、停 1.6 秒、淡出」，停留靠第二段动画的 `setStartDelay(Ui.TOAST_MILLIS)`。属性动画的开始延迟和时长一样要乘系统的动画时长缩放（`animator_duration_scale`），用户在无障碍里打开「移除动画」或在开发者选项里关掉动画时缩放是 0：淡入立刻结束，淡出不等就开始并立刻结束，提示从内容层移除，只显示一帧。所有用 `MsToast` 说明原因的地方（无锁屏凭据时拒绝导出、设置失败等）对这些用户都看不到，读屏的 live region 也来不及读。

设备验收里这个缺陷表现为 API 35 的 `ClipboardExportDeviceSmoke` 在「export without device credential is refused」等不到「这台设备没有可用的系统认证」：日志里认证查询确实返回了无凭据（`canAuthenticate … result: 11`），导出按钮仍在，诊断读到的 `settings get global animator_duration_scale` 却是 null。原因是同一轮里更早的 `ChineseLanguageReturnDeviceSmoke`（只在 API 35 的完整验收里跑）把三项动画缩放设成 0，结束时原值是 null 就 `settings delete`。窗口管理器读这几项设置时以它内存里的当前值作默认值，删掉设置后它继续用 0，于是之后的套件全在没有动画的系统上跑，`settings get` 却报 null。本机 API 35 模拟器上先 `settings put … 0` 再 `settings delete`，导出验收稳定停在同一步；只用 Android 9 的 `--core` 验收不跑那个套件，所以一直没出现。

## Decision

`MsToast` 的停留按真实时间计：淡入照旧用属性动画，停留改用 `View.postDelayed(…, FADE_MILLIS + TOAST_MILLIS)`，到点后再开始淡出动画并在结束时移除。关掉动画时淡入淡出瞬间完成，提示照样停 1.6 秒左右。被新提示替换后旧提示已不在内容层里，到点的回调发现父视图不是内容层就直接返回。

`ChineseLanguageReturnDeviceSmoke` 恢复动画设置时，原值为 null 的项先写回默认值 1 让窗口管理器真正恢复，再删除设置，回到没设过的状态；原来有值的照旧写回原值。

## Alternatives considered

- 测试改为监听无障碍事件或每拍清掉 UiAutomation 缓存再找提示：#9671 试过前者，CI 上仍然失败。提示在动画缩放为 0 时只存在一帧，任何轮询或事件匹配都抓不到，问题在产品和前面套件留下的系统状态，不在取样方式。
- 验收开始时强制把动画缩放设成 1：能让这一条通过，但掩盖了「移除动画」用户看不到提示的产品缺陷，也不解决前一个套件把系统状态留坏的问题。
- 用系统 `Toast` 代替：系统 Toast 的停留不受动画缩放影响，但 `MsToast` 选择自绘是为了跟设计一致、不带 Android 12 起的应用图标，见类注释，不为这个缺陷推翻。

## Consequences

关掉动画的用户能看到应用内提示，停留时间与开着动画时一致。其他用到 `setStartDelay` 的地方只有首页启动画面的装饰动画，那里没有需要停留阅读的内容，不改。设备验收后续套件不再继承被改成 0 的动画缩放。

验证：本机 API 35 模拟器在动画缩放实际为 0（先设 0 再删除设置）时，修复前 `clipboard-export.sh` 停在「export without device credential is refused」，修复后 select、verify 两段都通过；动画缩放正常时也通过。
