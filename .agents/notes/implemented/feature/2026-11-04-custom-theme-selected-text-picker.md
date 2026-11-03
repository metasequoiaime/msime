# Agent Note: 自定义主题的候选选中文字色取色器

Status: implemented

## Problem

#10740：macOS 0.57.0 的用户把「候选文字颜色」设成紫色，高亮的第一个候选仍是主题自带的绿色。自定义主题的取色器里没有「选中候选的文字颜色」这一项，选中行的文字色只能从别的颜色推出来：设了「候选选中色」时按那个底色的亮度取黑或白，否则取皮肤包的 `selected_text`，再没有时在内置底上取强调色。`CustomCandidateColors` 没有这个槽位，`Overrides::from_pickers` 把 `selected_text` 写死成 `None`，`preferences/tests.rs` 还断言这个键会被当成未知字段丢掉。

维护者 2026-11-03 拍板：加「候选选中文字色」取色器，字段放在共享 Rust 的 `custom_theme.candidate_colors.selected_text`，缺字段时照现在推导；所有画候选选中行的宿主都要照它画；Windows、Android、iOS、鸿蒙手机和 macOS 原生回退页都给取色器；`packages/ui` 这一行是迁移期规则的明确例外。

## Decision

`CustomCandidateColors` 加 `selected_text: Option<String>`，与其余取色器同样只接受 `#RRGGBB`，非法值报 `PreferencesError::InvalidCandidateSelectedTextColor`。没设时文档不写这个键，解析结果和现在逐项相同，出厂行为不变。

### 取值顺序

`skin::theme::resolve` 给自定义主题算 `selected_text` 时按下面的顺序取第一个有的：

1. 选中文字取色器 `candidate_colors.selected_text`（本篇新增）。
2. 选中底色取色器 `candidate_colors.selected` 按亮度推出的黑或白（`readable_text`）。
3. 画出来的皮肤包自己的 `selected_text`。
4. 内置底上取最终的强调色；`system` 底上没有，交给宿主画原生的。

`selected_number` 跟着同一条链：前两档时是那个颜色加 `PICKED_NUMBER_ALPHA`（`9D`），否则是皮肤包的 `selected_number`，再否则内置底上取编号色。皮肤包的选中释义细分槽位（`selected_translation` 及其 `_pos`、`_separator`）在前两档时也取那个颜色加 `9D`。选中行的底色 `selected` 不受影响，照旧是「选中底色取色器 > 皮肤包 > 内置底上强调色的 `24` 透明度」。前两档不看底是不是 `system`：只设选中文字取色器、底是 `system` 时，解析结果只带 `selected_text` 和 `selected_number`，选中底仍由宿主画原生的。

这个顺序和已有的推导一致：用户显式选的颜色压过推出来的颜色，推出来的颜色压过皮肤包和底。设置页镜像 `packages/ui/src/theme/global-theme.ts` 的 `customCandidatePalette`、网页引擎移植 `packages/web-engine/src/skin.js` 同序，`apps/desktop/tests/candidate/custom-theme-parity.json` 加了四条用例钉住三处一致。

### 各宿主

- **Windows**：Server 照解析结果画；`CandidateSkin.h` 的 `apply_candidate_skin_extra_slots` 设了选中底色或选中文字取色器时都不再叠皮肤包的三个选中细分槽位。WinUI 和 Win7 设置共用的 `ThemeSkinModel.h` `color_slots` 在「候选选中色」后面加一项，没设时色块显示原生选中行的白字。
- **macOS**：IMK 新存储键 `MSIMEClientCandidateSelectedTextColor`，读写、账号推送和恢复默认都走 `CandidateColorControls()` 那张表；解析请求带上 `selected_text`。解析结果给了 `selected_text` 时 macOS 本来就不再按选中底推导前景。原生回退窗口「配色」里多一个取色器；主路径的 Tauri 设置页靠 `packages/ui` 那一行。
- **Linux**：IBus 和 Fcitx5 经典界面原来在主题不给选中底也不给强调色时一律画原生白字，`system` 底上只设这个取色器会不生效；`candidate_theme_colors` 现在在这种情况下也读解析结果的 `selected_text`、`selected_number`，序号的透明度合成在原生选中底上。X11 原生画布本来就读解析结果。设置在共享页。
- **HarmonyOS**：`GlobalTheme.hostDrawnCustomTheme` 的白名单加上 `selected_text`，2in1 候选窗经解析器取到它；手机和平板的触屏候选条照旧用键盘配色（其余取色器也不作用于它），只在自定义主题设了这个取色器时把首选候选的字换成它（`GlobalTheme.touchSelectedText`）。设置是 ArkWeb 加载的共享页。
- **Android**：候选栏同样照旧用键盘配色，同一条例外（`CandidateSelectedTextPolicy.stripColour`）；用了候选皮肤包时由解析器给出。键盘页「键盘工具栏」组加「候选选中文字色」一行，Android 没有系统取色器，给跟随主题、七种常用颜色和「自定义…」输入 `#RRGGBB`。
- **iOS**：「外部皮肤与候选颜色」在「候选栏使用主题配色」打开时多一个「首选文字」，候选条首选候选和展开面板选中项本来就画解析结果的 `selectedText`；「恢复皮肤颜色」一并清掉它。
- **共享设置页**：「自定义配色」里「候选选中色」后面加「候选选中文字色」，和强调色、选中色一样只在 `candidate_row_colors` 为真时出现。没有新加 `HostCapabilities` 位：所有给出行颜色的宿主都画它，一个处处为真的能力位不带任何信息。

选颜色时文档跟着改什么与其余取色器相同（共享页 `updateCandidateColor`、Windows `candidate_color_change`、iOS `pickingCandidateColor`、Android `CandidateSelectedTextPolicy.change`）：选了颜色就选上自定义主题，原来不是自定义主题时把当时的主题留作底并清掉两个皮肤槽位；交还给主题只清这一项。

## Alternatives considered

- **不加取色器，回复报告者用「候选强调色」或「候选选中色」绕过** — 最强的理由是两种办法今天就能用，而且 11-03 刚定下迁移期 `packages/ui` 不加新设置。没有采用：强调色会连带把选中底变成它的 `24` 透明度，在 `system` 底上还会变成实心选中底；选中色只能让文字变黑或白。两条都给不了「底不动、只换字色」，维护者决定加。
- **只做共享 Rust 和已有原生设置的宿主，`packages/ui` 不破例** — 最强的理由是守住迁移期规则，冻结只靠评审守。没有采用：报告者在 macOS 主路径（Tauri 设置页）上，鸿蒙的设置也是同一份共享页，不破例的话这两个平台的用户都改不了；设置页的预览本来就要和 Rust 对齐，镜像那一处无论如何要改。维护者明确给了这一行例外，规则本身不变。
- **加一个 `HostCapabilities::candidate_selected_text_color` 位** — 最强的理由是和编号色、边框色一样按宿主开关，以后某个宿主不画时可以单独藏起来。没有采用：这次所有给出行颜色的宿主都画它，新位在每个平台都是真，只多一处要同步的契约；真有宿主不画时再加。
- **触屏候选条（Android、鸿蒙手机）完全照旧，只在桌面候选窗和皮肤包里生效** — 最强的理由是其余取色器都不作用于触屏候选条，保持一致。没有采用：维护者要求 Android 也给取色器，候选条不照它画就是一个背后什么都没有的控件；只放开首选候选的字色这一项，候选条的底、其余候选和工具栏照旧跟随键盘配色。

## Consequences

- 收益：用户可以只换高亮候选的字色而不动底色，六个宿主画出来一致；没设时解析结果、文档和各宿主的画法都与以前逐项相同。
- 代价：自定义主题从七个取色器变成八个，每加一个槽位要改共享 Rust、两处 TS 镜像和六个宿主；macOS 多一个 NSUserDefaults 键。`packages/ui` 在迁移期多了一行，迁到原生后随整页删除。
- Linux 的修改也让「皮肤包在 `system` 底上只给 `selected_text`、不给选中底和强调色」这种包的选中文字色开始生效，以前被原生白字盖住；macOS、Windows 一直照画，这里是补齐。
- Android 和鸿蒙在候选栏上选颜色会切到自定义主题；键盘还没设过独立键盘皮肤、靠全局主题画的用户，键盘换成自定义主题的底（就是原来的主题），文档里若有旧的键盘设计会画那个设计，与其余宿主选取色器时相同。
- 验证缺口：iOS 的 XCTest、Android 设置页的 Gradle 编译和设备、鸿蒙 `.ets` 编译、Windows 的 WinUI 与 Win7 设置窗口都没在本机跑，由 PR 上的 CI 和之后的设备验收补上。

## Related

- [macOS 主题页与候选窗口页精简](../simplification/2026-10-16-macos-appearance-settings-prune.md)：部分重叠。那篇定下桌面宿主不给编号色和边框色取色器、取色器收进「自定义配色」；本篇在同一个折叠区里加一项，编号色和边框色的决定不变。
- [全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)：本篇的 `packages/ui` 一行是它迁移期规则的维护者特许例外，规则本身不变。
