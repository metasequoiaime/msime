# Agent Note: Fcitx5 跟随系统保留水杉候选样式

Status: implemented

## Problem

PR #6466 的审查指出，全局主题缺省为 `system`，但 [主题所有权决定](../../implemented/bug-fix/2026-10-07-fcitx5-candidate-theme-ownership.md) 把共享层的空调色板解释成没有候选覆盖：新装不接管 classicui，仍使用 `msime` 的旧用户升级也立即恢复成原值或自带主题。保护第三方主题因此附带改变了默认品牌外观。

维护者确认保留水杉样式，并明确选择：用户主动从内置主题切回「跟随系统」也继续使用水杉样式；`system` 只表示颜色跟随系统，不再表示退出接管。

## Decision

- 有效共享解析结果 `id=system` 也是候选外观覆盖：使用现成的 Linux 原生 token 绘制水杉的候选卡片、品牌标志和菜单，不修改全局默认值或共享解析 ABI。
- 自带 classicui 主题与仍属于水杉的主题可以应用默认样式；第三方主题仍只在用户主动选择时接管，后台同步、窗口切换、系统明暗与重启不抢回。
- 无覆盖自定义主题、失效皮肤与解析失败继续退出并按原记录恢复；不把这些失败或空结果误判成有效 `system`。
- 恢复记录仍用于退出无覆盖主题与卸载，不因切回 `system` 删除；不新增配置项或所有权状态。

## Historical note audit

- [主题所有权决定](../../implemented/bug-fix/2026-10-07-fcitx5-candidate-theme-ownership.md)：部分取代，仅翻转 `system` 的默认外观和切回语义；第三方保护、共享选择基线、恢复安全与 CI 门禁仍成立，两篇互链保留。
- [每拍同步](../../implemented/bug-fix/2026-10-08-fcitx5-theme-sync-tick.md)：缓存与重试约束不变，无需替代。
- [模式提示宽度](../../implemented/bug-fix/2026-10-07-fcitx5-mode-hint-width.md)：装饰宽度与活动主题判据不变；退出接管测试改用无覆盖自定义主题。

## Alternatives considered

- **沿用 system 退出接管**：与 #4371「切回 system 恢复第三方主题」的审查建议一致，也不用改变现有判据；但新装与升级默认丢失水杉样式，维护者明确不接受该产品行为。
- **把全局默认改成内置水杉主题**：可以复用现成配色并立即显示品牌外观；但改变其他平台的默认策略和固定配色，且不能解决用户主动选回 system 后退出的语义，不采用。
- **区分初始 system 与主动切回 system**：兼顾默认品牌外观和原审查的退出操作；但同一个设置值会表现出不同含义，并需要维护额外生命周期状态，维护者选择统一保留样式。

## Verification

复用 `fcitx5-candidate-theme-priority` 的真实 classicui 与 Host API：缺省新装接管、旧版本 `msime` 在恢复记录缺失/损坏时保留、主动切回 system 保留及重新接管、原生浅深色；第三方主题在后台、窗口切换、重启与明暗变化中不被夺回。菜单主动选择 system 后，旧窗口补读同一存储选择也不接管。既有无覆盖恢复、提示缓存与每拍扫描断言保持执行。

`linux-candidate-palette` 区分有效 system、无覆盖 custom 与失败空结果；`fcitx5-candidate-theme-hint` 验证 system 仍是活动水杉主题但不携带皮肤装饰，并用无覆盖 custom 保留真正退出后的缓存重建测试。门禁命令为 `ctest --test-dir target/linux-gate --output-on-failure` 与 `bash platforms/linux/build-container.sh`。

本机真实隔离 Fcitx5 daemon 与两个 GTK3/X11 编辑器核对新插件的进程映射，执行缺省 system、第三方选择、旧窗口补读、主动切回 system 与再次外部选择；完整候选窗截图确认品牌标志、圆角、细边框、浅色卡片与蓝色选中行均显示，未上屏文本。脚本与截图只留本地 `target/`，不提交。

## Consequences

- **收益**：缺省与主动切回 system 的含义一致，水杉候选样式不会因新装或升级消失；复用原生取色与既有所有权判断，不新增持久化状态、设置项或平台默认值。
- **取舍**：`system` 不再兼任「恢复接管前的 Fcitx5 主题」。用户需要在 fcitx5-configtool 中选择第三方或其他主题，水杉尊重该外部选择；这是对原 #4371 审查约定的明确调整。
- **边界**：classicui 是进程级资源，接管仍影响同一进程的其他输入法。原生 Wayland、多屏与发布包升级验证仍需另行执行。
