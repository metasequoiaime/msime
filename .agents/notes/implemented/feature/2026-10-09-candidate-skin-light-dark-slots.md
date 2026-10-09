# Agent Note: 候选窗皮肤按明暗分浅色、深色两个槽位

Status: implemented

## Problem

自定义主题只有一个皮肤槽位 `custom_theme.candidate_skin`。社区里绝大多数候选窗皮肤的清单 `base` 是固定明暗的内置主题（`light`、`paper` 为浅色，`shuishan`、`night`、`ink` 为深色），而 `theme::resolve` 原来的规则是「固定明暗的底决定模式、忽略宿主的 `dark`」，所以选了一款深色皮肤，系统处在浅色模式时候选窗仍是深色，反过来也一样，配色与系统主题不搭。用户的要求是：亮色模式只能用亮色皮肤，暗色模式只能用暗色皮肤。

2026-10-09 统计社区公开的 88 款候选窗皮肤，85 款的 `base` 固定了明暗，只有 3 款是 `system`。清单里给固定底皮肤另一种模式写的配色从来不会被画出来。

## Decision

`custom_theme` 有两个皮肤槽位：`candidate_skin` 是浅色槽位，`candidate_skin_dark` 是深色槽位。`candidate_skin_dark` 未设时深色模式也取 `candidate_skin`（`CustomTheme::candidate_skin_for`），只设过一款皮肤的旧文档不需要迁移。

皮肤包属于哪个槽位由它清单 `base` 对应内置主题的明暗决定（`theme::skin_appearance`）：浅色底进浅色槽位，深色底进深色槽位，`system` 底两个槽位都可以放。`resolve` 只在皮肤包所属的明暗下画它（`ThemePackage::draws_in`）；不符时不画，主题落回底。设过皮肤、当前模式却没有可画的包时，`custom.base` 也只在属于当前明暗时作底，否则按 `system`：应用皮肤时包的 `base` 会被写进 `custom.base`，不这样做，浅色模式会落回深色皮肤留下的 `night`。完全没设皮肤的自定义主题照旧用 `custom.base`。

槽位选择在 client-core 里完成：`msime_client_resolve_theme` 带 `skins_directory` 时按 `dark` 加载对应槽位的包，所以扫描皮肤根目录的宿主只需把 `custom_theme` 原样带上；Linux 宿主带 `package`，由宿主按同一规则挑当前模式槽位的那一项，Tauri 发布 `candidate_skin_catalog` 时两个槽位的皮肤都保留在上限之内。

应用皮肤的规则在设置页（`applyCandidateSkin` / `removeCandidateSkin`，packages/ui/src/theme/global-theme.ts）和各原生设置界面保持一致：深色皮肤写深色槽位，并清掉旧文档放在 `candidate_skin` 里的深色皮肤；浅色皮肤写浅色槽位，深色槽位空着而原来的 `candidate_skin` 不是浅色皮肤时，先把它挪进深色槽位；`system` 底的皮肤写两个槽位；`base` 照旧写成包的 base。取消使用只清放着它的槽位，清空皮肤的路径两个槽位一起清。

设置页的镜像 `customCandidatePalette` 与 `resolve` 由 `apps/desktop/tests/candidate/custom-theme-parity.json` 对齐，TS 侧用 `candidateSkinFor`、`skinDrawsIn`、`customDrawnBase` 复现同一套选择。

## 同步与兼容

- 按键同步的平台各加一个键：`platform.android.custom_candidate_skin_dark`、`platform.harmony.custom_candidate_skin_dark`、`platform.macos.custom_candidate_skin_dark`，空串表示未设，与浅色槽位的键同样校验。
- `CustomTheme` 带 `deny_unknown_fields`，早于这个字段的构建会拒读写过 `candidate_skin_dark` 的偏好文档和解析请求。正式发布里输入法和设置应用同包、版本一致；开发机单独换输入法时要一起换设置应用。处理方式与 [`show_app_logo`](2026-10-08-macos-app-logo-toggle.md) 相同。
- 键盘皮肤试用的持久记录 `TrialRecord` 多一个 `previous_candidate_skin_dark`，带 serde 缺省值，旧记录照常读取。

## Alternatives considered

- **只改皮肤包数据，把深色意象的皮肤改成只声明 `dark`** — 不用改任何客户端代码，2026-10-09 已对 12 款深色意象的社区皮肤这样做过（版本 1.1.0）。但 `base` 固定明暗时 `resolve` 本来就只画那一种配色，删掉另一种配色对候选窗没有可见变化，深色皮肤在浅色系统下仍是深色，问题没解决。
- **在当前模式下只列出同明暗的皮肤、仍只有一个槽位** — 选择界面最简单。但多数人开着系统自动切换明暗，白天选的浅色皮肤到了夜里只能落回默认主题，看起来像皮肤自己消失了；两个槽位让每种明暗各有一款。
- **用 `Option<Option<String>>` 区分「旧文档未设」和「明确不要深色皮肤」** — 能精确表达三种状态。但各宿主（Swift、ArkTS、Objective-C++、TS）都要区分缺省和 `null`，而按明暗绘制的规则已经让「浅色槽位里的皮肤到了深色模式不画」自然成立，第三种状态没有可见差别，不值得这层复杂度。

## Consequences

- **收益**：浅色模式只会画浅色皮肤，深色模式只会画深色皮肤；跟随系统切换明暗的用户可以白天、夜里各用一款。旧文档不迁移：原来那款皮肤在它自己的明暗下照常生效。
- **代价与已知上限**：
  - 只设过一款固定明暗皮肤的老用户，在另一种明暗下会看到候选窗变成跟随系统的默认样式，这是有意的行为变化。
  - 固定底皮肤清单里另一种模式的配色仍然不会被画；皮肤作者想两种模式都画，要把 `base` 设成 `system`。
  - 新字段让旧构建拒读偏好文档，见「同步与兼容」。

## Verification

- `crates/client-core/src/skin/theme/tests.rs`：`a_package_is_drawn_only_in_the_mode_of_its_base`、`the_dark_slot_is_used_in_dark_mode`、`a_skin_base_left_from_another_mode_follows_the_host`，以及 `web_custom_theme_mirror_cases_match_resolve` 写出的两条新用例（深色包在浅色模式不画、浅色包在深色模式不画）。
- `apps/desktop/tests/candidate/candidate-skin-slots.test.ts` 覆盖槽位归属、按模式取槽位、应用与取消的规则和旧深色皮肤的保留；`custom-theme-parity.test.ts` 让设置页镜像跑同一份用例。
- 各平台的宿主测试见对应 PR 说明。
