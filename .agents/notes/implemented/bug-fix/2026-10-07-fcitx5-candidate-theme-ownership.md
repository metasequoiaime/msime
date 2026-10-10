# Agent Note: Fcitx5 候选主题只按用户主动选择接管，无覆盖时退出并恢复

Status: implemented

`system` 的退出接管语义由 [跟随系统保留水杉样式](../feature/2026-10-09-fcitx5-system-theme-keeps-brand-style.md) 部分取代：维护者确认缺省和主动切回 system 均保留水杉样式；本篇保留原审查理由，第三方保护、共享选择基线、无覆盖自定义/解析失败恢复与 CI 门禁仍成立。

## Problem

PR #4371#discussion_r4203158939 / r4203158947 指出，宿主的接管判据与生命周期都不对：

- `applyCandidatePanelTheme` 用「当前 classicui 主题是否可替换」（`fcitx_theme_replaceable`）决定接管，`refreshPreferences` 则用 `preferences.value("global_theme", "system") != "system"` 判断「用户选了主题」。启动、焦点进入、runtime options 刷新、系统明暗变化都走同一条路径，于是用户在 fcitx5-configtool 里选过第三方主题后，任意一次后台同步都可能把 `Theme` 强制写回 `msime`。
- 缓存 `theme == candidate_theme_applied_` 直接 `return`，跳过了所有权判断：classicui 被改回自带主题后，同一份配色不会重新接管。合并 develop 的每拍缓存后以 `chosen` 与落盘选择为准：输入没变而用户只是又选了一次水杉主题时仍接管，其余情况不追回用户自己改的值。
- 把全局主题切回「系统」后，水杉仍占着 classicui 的 `Theme`/`DarkTheme`，既不退出也不恢复。

PR #6466 的多窗口审查指出，偏好刷新若与各 `FcitxState` 自己的旧偏好比较，同一次选择会被重复当成主动选择：活动窗口先读到设置页的新主题并接管，用户随后选择第三方 classicui 主题，再聚焦尚未刷新偏好的旧窗口，就会被再次接管。原生门禁镜像只有 Fcitx5 开发库，没有 classicui 运行库和 pangocairo 开发包，接管与提示测试未注册，73 项通过不能覆盖它们。

装机后的设置页选择仍留下 Catppuccin：离开输入框后会话被关闭，设置页保存新主题，返回输入框时 `ensure()` 先把新选择记成已见，却不保留比较结果；此时 `session_` 尚未创建，主题同步也没有实际执行。后续同步只能以 `chosen=false` 看到第三方主题，刚才的主动选择因而被吃掉。只写偏好、不发布 runtime options 的最小真实 classicui 回归同样失败，排除了运行配置重载与文件写入权限作为根因。

PR #4371#discussion_r4203158943 指出解析边界：`candidate_dark_theme`、`surface_dark_theme`、`candidate_layout_id` 对 `preferences` 调 `.value()`，而 `preferences` 为 `null`、数组或数字时抛出 `nlohmann::json::type_error`；这三个 helper 由 IBus 宿主（`ClientEngine.cpp`）与 Fcitx5 宿主（`resolveThemeInMode`）共用，只修 `global_theme` 一行不够。

## Decision

- **接管依据是共享层解析出的实际候选覆盖**，不是 classicui 当前主题。`CandidateTheme::covers_candidates` / `candidate_theme_covers(resolved)`（`CandidateColors.h`）为真仅两种：共享层的 `candidate` 是非 null 对象（内置主题，或 custom 的皮肤槽位/取色器给了颜色），或 `candidate_skin` 非空（皮肤清单声明了当前布局与明暗）。`system`、基底 system 且无皮肤无颜色的 custom、未知/退役 id、解析失败落回的空文档都判为「没有覆盖」。它在 `candidate_theme_colors` 里**先于**空槽位被原生 palette 补齐前计算。
- **只有「用户主动选择」才允许从第三方主题手里接管。** `FcitxState::syncCandidatePanelTheme(bool chosen)` 的 `chosen` 只来自主题菜单 `setThemeChoice()`，或偏好监视/新会话初始化接受存储快照时 `candidate_theme_selection(preferences)`（`global_theme` + 整个 `custom_theme` 两个字段的可比较文档）相对整个插件已见的选择发生变化时。`FcitxEngine::candidate_theme_selection_` 共用一份观察状态，与 classicui 的进程级所有权一致，不随上下文关闭清空。`candidateThemeSelectionChanged()` 在真实存储快照被会话接受后比较并记录；菜单立即记录自己的选择，避免另一个窗口补读菜单保存后再次接管。新会话在 Host API 创建与焦点初始化成功后，用最新存储快照比较共享基线并同步主题：首次观察只建基线，同一份选择不重复接管，已有基线下尚未处理的新存储选择仍传 `true`。不把 runtime options 或上下文 override 的旧主题当成新选择。`refreshProviderSockets()`（runtime options 刷新）、`setSystemDark()`、以及指纹不变的偏好同步都传 `false`。
- **CI 必须实际运行接管与提示测试。** `Dockerfile.build-gate` 安装 Debian 的 `fcitx5-modules`（classicui 运行库与 addon 元数据）和 `libpango1.0-dev`（pangocairo 开发接口）。`build-container.sh` 保存 CTest JUnit 报告，并断言 `fcitx5-candidate-theme-priority` 与 `fcitx5-candidate-theme-hint` 各出现一次、状态为 `run` 且没有 `skipped` 节点；少注册一项或返回 77 都使门禁失败。普通开发环境仍可不安装 classicui，不把门禁要求变成所有构建的强制依赖。
- **无覆盖时退出接管并恢复。** `restore_classicui_theme(fcitx::AddonInstance&)`（`FcitxEngine.cpp`）在 `!resolved.covers_candidates` 时调用：只把当前值仍等于水杉自己主题名的 `Theme`/`DarkTheme` 放进 `held`，再用 `panel_restore_values(record, "fcitx5", held, kClassicuiStockThemes)`（`PanelRestoreRecord.h`）求恢复值——记录里的 `prior`，记录没留下可用 prior 时用自带主题 `default`/`default-dark`。恢复记录缺失或损坏时仍用空记录调用 `panel_restore_values`，不能跳过自带主题兜底。用户自己改过的项不在 `held` 里，一项都不动。
- **恢复不是一次新的接管**：走 `classicui.setConfig()`，不经 `set_classicui_config()`（后者会 `record_classicui_takeover`），否则会把用户自己的值换成被恢复的值；只写 `Theme`/`DarkTheme`，`Font`/`WheelForPaging` 等其他项不动。
- **缓存只决定是否重写主题文件与是否重读 classicui 的完整配置**，不影响所有权判断、退出接管或 `publishCandidatePanelStatus`。合并 develop 的每拍缓存（#5988，输入没变就直接返回）后，接管写好且不是新的一次主动选择、落盘的主题选择与提示配置（跟随深色、字体、Wayland 字体 DPI）也没变时才跳过：这些落盘项变化时只重读一次现值刷新提示，不重写主题或重新接管；用户自己改 classicui.conf 或卸载还原的主题不会被改回去；用户在主题菜单里重新选择水杉主题（`chosen`）仍然立即重新接管。退出接管时同时清空 `candidate_theme_applied_`，否则选回同一皮肤会命中旧 stamp，却留下已清零的提示宽度。
- **非对象 preferences 按空文档处理**：`candidate_dark_theme`/`surface_dark_theme` 跟随系统明暗，`candidate_layout_id` 取默认纵向，`candidate_theme_request` 用 `find()` 本就返回 `system`。入口 helper 覆盖两个宿主。
- 自带主题名唯一副本 `kClassicuiStockThemes{{"Theme","default"},{"DarkTheme","default-dark"}}`，与 `msime-linux-setup --unregister` 的 `FCITX5_STOCK_THEMES` 同一规则。

## Alternatives considered

- **继续使用每个上下文的旧偏好作基线** — 不增加插件状态，也能识别单窗口的设置变更；但上下文失焦就停止刷新，旧窗口无法知道这次选择已由另一窗口处理，真实多窗口回归会覆盖用户后来选的第三方主题。

- **只补镜像依赖，不检查执行报告** — 改动最少，当前环境中两项测试就能注册；但条件注册或返回 77 会使后续依赖变化再次产生假绿，必须按测试名确认实际执行，而不是固定总数。
- **沿用 `global_theme != "system"` 作为接管触发** — 改动最小，但把「重读同一份偏好」也当成了主动选择。PR #4371 的测试预期「外部切主题后再次强制写回 msime」正是这个判据的直接后果，被本轮改掉。
- **新建一份「上次是否水杉接管」的状态文件** — 能精确记录所有权，但和 `panel-restore.json`（`prior`/`written`）职责重叠，且退出接管与卸载恢复会分成两套逻辑；复用既有记录 + `written` 一致性检查更小。
- **改回自带主题时自动追回** — 能让先前选好的水杉配色自动恢复；但无法区分用户手改与 `--unregister` 的还原，会让仍在运行的插件撤销卸载操作。保留 develop #5988 的行为：已应用输入不变时只刷新提示，重新接管须主动选择。
- **把恢复也写进接管记录** — 会让记录里的值在退出接管后变成用户的值，下一次接管/卸载的语义全错。恢复只写 addon 配置，不碰记录。

## Consequences

- **收益**：用户选的第三方主题不再被后台同步夺回；切回「系统」或无覆盖 custom 时真的退出并把仍属于水杉的项原地放回；classicui 被改回自带主题后不自动追回，但重新主动选择同一份水杉配色仍能接管；畸形偏好不再让宿主抛异常。
- **门禁代价**：构建镜像多安装 classicui 及 Pango 相关包；测试加载真实 addon，但不启动系统输入法 daemon、不需要锁定词库或用户桌面。JUnit 报告只留在规定的 `target/linux-build-gate` 产物目录。
- **启动边界**：进程第一次观察偏好只建立基线，不从第三方主题接管；已有基线后，在没有会话时保存的新主题由下一个成功创建的会话处理一次。焦点进入本身不是主动选择，但不能吞掉用户尚未处理的存储选择。原生回归分别覆盖仅写偏好、同时发布 runtime options，以及处理后外部改第三方主题不被下一拍夺回。
- **代价与已知上限**：「退出接管」无法区分「用户在 fcitx5-configtool 里主动选了水杉主题（`Theme=msime`）而偏好是系统」和「上次残留」，前者也会被改回 prior/自带主题——与卸载脚本同一规则。同理，后台同步不读 `getConfig()` 的代价是：用户在 fcitx5-configtool 改主题后不做焦点变化就切中英，会用到上一次同步的判据（见模式提示笔记）。`panel_restore_values` 的 `written` 一致性检查在当前调用点上不会触发，保留是为了与 `--unregister` 的 `restorable` 同构。

## Verification

多窗口回归复用 `candidateThemePriority`，使用两个真实 Fcitx 输入上下文、真实 Host API 的空合成资源目录、偏好存储和 classicui。修复前 `ctest --test-dir target/linux-gate -R '^fcitx5-candidate-theme-priority$' --output-on-failure` 在「旧窗口补读已处理的偏好不能夺回第三方主题」处失败；修复后还覆盖旧窗口下一次设置页选择、菜单立即接管、另一个窗口晚读菜单保存时保持第三方主题，以及不带偏好存储的合法 runtime options。缺存储场景先复现对 null 快照调用 `value()` 的异常，再修复为回退到运行时偏好。真实 classicui 测试按运行库是否提供 `DarkTheme` 断言：Fcitx5 5.0.x 仍验证 `Theme` 接管、恢复与活动主题提示，支持独立深色项的版本同时验证 `DarkTheme`；不整项跳过。固定 Bookworm 镜像的 Fcitx5 5.0.21 两项定向测试通过。本机 Fcitx5 5.1.23 的 75/75 项通过；`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes`、Fcitx5 静态契约与差异检查通过。


CI 原镜像的 CTest 注册表只有 73 项，按名字断言两项主题测试存在时失败。JUnit 执行检查用合成报告自检：正常执行通过，缺失任一测试或包含 `skipped` 节点均失败。

固定 Bookworm 镜像（Fcitx5 5.0.21）完整 `bash platforms/linux/build-container.sh` 通过，CTest 报告为 75 项、0 失败、0 跳过，两项主题测试均为 `status=run`；五笔生产构建、两版本安装与 Host API 链接隔离、日文输入法登记检查通过。本机 Fcitx5 5.1.23 的 quick 门禁再次通过。以上为本地执行 CI 同一脚本的证据，未推送或触发新的 GitHub CI。

`platforms/linux/tests/candidate/candidate_palette.cpp`（非对象文档、覆盖信号、`candidate_theme_selection` 边界）、`panel_restore_record.cpp`（`panel_restore_values` 的 prior/兜底/单项/`written` 不匹配）、`platforms/linux/fcitx5/tests/native.cpp::candidateThemePriority`（`--theme-priority`，真实 classicui：启动/焦点不夺回第三方主题、主动选择接管并记录、`Nord-Dark -> paper -> system` 恢复、单项被用户改过只恢复另一项、恢复不动字体与记录、恢复记录缺失或损坏时兜底、进程重启后尊重用户选择）。
