# Agent Note: Android 设置应用的底部面板打开即展开

Status: implemented

## Problem

维护者反馈「底部弹窗的高度还是有问题，所有的都要修」。在 Pixel 3 XL（Android 0.6.0 发布包）上逐个打开设置应用的底部面板：选择面板（`OptionSheet`）、应用主题、App 图标这些内容少的面板本来就贴着内容，没有多余的空白；问题出在用 `SettingsSheet` 搭的自定义面板上。按钮顺序、九键左侧符号、添加符号、符号分类顺序、文本编辑按键顺序和社区皮肤详情都没有设打开状态，Material 的 `BottomSheetDialog` 于是先停在折叠高度（屏高减去屏宽的 9/16，这台机器上约为屏高的 73%）。按钮顺序面板的内容比这个高、又放得下一屏，打开后「恢复默认顺序 / 重置」被切在导航栏下面，「保存」完全看不见，要先往上拖才能保存。

这不是第一次：`OptionSheet.show()` 和 `AppIconSheet.show()` 早就各自加了 `STATE_EXPANDED`，`LoginSheet` 为了底部的同意勾选又加了 `setSkipCollapsed(true)`（见 [登录前的明示同意](2026-10-27-login-privacy-consent.md)），每次都只修了出问题的那一个面板，`SettingsSheet` 这一路一直漏着。

## Decision

设置应用的底部面板窗口一律由 `home/SheetDialog.create(Context)` 创建，`OptionSheet`、`SettingsSheet`、`AppThemeSheet`、`AppIconSheet` 和 `LoginSheet` 都改为调用它。它做三件事：

- 经 `ScreenCaptureProtection.protect` 加「防止截屏」的窗口标志（`check-host.sh` 按 `new BottomSheetDialog(` 出现的次数核对，现在只剩这一处）；
- `setSkipCollapsed(true)`：往下拖就是关掉，不会停在折叠高度把末尾的操作行藏回屏幕下面；
- `setState(STATE_EXPANDED)`：打开就是内容的高度；内容高过一屏时面板撑到屏幕高度，里面的 `NestedScrollView` 滚动，末尾的操作行滚到底就能点。

内容少的面板本来就比折叠高度矮，展开状态和折叠状态对它们是同一个位置，所以选择面板、应用主题、App 图标的样子不变。各处原来单独写的 `STATE_EXPANDED` 和 `setSkipCollapsed` 删掉，以 `SheetDialog` 为准；`LoginSheet` 在没勾同意时重新展开的那一处保留，它是用户拖动之后的再展开，不是打开状态。

导航栏留白不在这里处理：M3 的 modal bottom sheet 样式默认按导航栏 inset 给面板加底部内边距，选择面板的「取消」一直在导航栏上方，展开后的自定义面板同样如此（真机截图见下）。

## Alternatives considered

- **把 `peekHeight` 设成量出来的内容高度** — 好处是保留折叠状态，长内容时用户还能先看到一部分页面；但要在布局后再量一次、内容变化（九键符号增删、登录面板展开邮箱输入）时还要重算，而折叠状态对这些面板没有用处：它们都是要做完一件事再关掉的表单，停在一半只会藏住操作行。
- **只在 `SettingsSheet` 里补 `STATE_EXPANDED`** — 改动最小，但正是前面三次各修一个面板的做法，下一个直接 `new BottomSheetDialog` 的面板还会再漏一次；集中到一个工厂后，截屏保护的计数检查同时保证了新面板只能走这一处。
- **把操作行固定在面板底部、不随内容滚动** — 内容高过一屏时操作行也不用滚就能点；但现有面板把操作行放在内容卡片里（`GroupCard`），改成固定底栏要动每个面板的布局，超出这次「高度不对」的范围。目前这些面板在常见字号下都放得下一屏，滚到底可达已经满足要求。

## Consequences

- **收益**：所有底部面板打开时，内容放得下一屏就完整显示，末尾的「保存」「重置」「取消」不用拖就在导航栏上方；新加的底部面板经 `SheetDialog` 自动得到同样的行为。
- **代价与已知上限**：面板不再有折叠状态，往下拖一次就关掉；编辑到一半的排序面板被拖掉时按原样不保存（这些面板本来就是「点击保存后生效」）。内容高过一屏时操作行要滚到底才看得见，若以后出现常态下就超过一屏的表单面板，再考虑固定底栏。

## Verification

- `platforms/android/tests/device/settings/OptionSheetDeviceSmoke.java` 新增 `checkSettingsSheetOpensFullHeight`：用 13 行、每行屏高 1/20 的合成内容打开 `SettingsSheet`（比折叠高度高、放得下一屏），不拖动就断言最后一行完整地在导航栏上方。它随 `smoke.sh` 的 `OptionSheetDeviceSmoke` 在模拟器上跑，CI 的 `android-device.yml` 在合进 develop 后运行。
- `bash platforms/android/check-host.sh` 编译设备套件并核对截屏保护的面板计数；`MSIME_EDITION=pinyin bash platforms/android/build-apk.sh` 经 Gradle 编出 APK。
- 真机：Pixel 3 XL（Android 12）上与 0.6.0 发布包并装 `app.msime.android.pinyin` 对比。九键左侧符号打开即显示到「放弃本次调整」，添加符号打开即显示到「关闭选择」，修改前两者末尾的操作行都在导航栏下面；文本编辑按键顺序、按钮顺序（develop 上多了「语音」一项，共 10 项）和符号分类顺序的内容高过一屏，打开即撑满、滚到底操作行在导航栏上方；选择面板、应用主题、App 图标、登录和社区皮肤详情的外观与修改前相同。
