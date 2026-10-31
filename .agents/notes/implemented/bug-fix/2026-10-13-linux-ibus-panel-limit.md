# Agent Note: IBus 宿主报告 ibus_panel，设置页说明圆角、边框颜色和皮肤装饰图在 IBus 下不生效

Status: implemented

## Problem

`HostCapabilities` 是按平台给的。`crates/client-core/src/host_surface.rs` 对整个 Linux 声明 `candidate_corner_radius` 和 `candidate_border_color`，设置页因此在 Linux 上显示圆角滑块和边框颜色。这两项只有 Fcitx5 的 classicui 主题画得出来（宿主把圆角、细边和皮肤的顶部装饰图生成进 `theme.conf` 与它引用的 PNG）。IBus 宿主交给桌面面板的只有候选文字和 RGB 前景、背景属性，`ClientEngine.cpp` 从不读圆角、边框或装饰图；`platforms/linux/README.md` 也写明了 IBus 的候选属性不能描边、外部皮肤的顶部装饰图无法经 panel 协议显示。于是在 IBus（GNOME Shell 以外，例如 ibus-ui-gtk3）下，设置页让用户调一个圆角、选一个边框颜色、装一个带装饰图的皮肤，预览里都看得到，候选窗上什么都不变，页面也不说为什么。

Linux 已经有一条按运行时说实话的通道：宿主把「当前画候选的面板忽略哪些设置」写进 `$XDG_RUNTIME_DIR/msime-client/candidate-panel.json`（`platforms/linux/src/candidates/CandidatePanelStatus.h`），设置应用读成 `HostCapabilities.candidate_panel_limit`（`apps/desktop/src-tauri/src/lib.rs`），外观页和主题页各显示一条说明（`packages/ui` 的 `CandidatePanelLimitSection`）。IBus 宿主过去只在 GNOME Shell 下报告 `gnome_shell`，其余情形写 `null`，即「全部生效」。

## Decision

- 新增面板限制 `ibus_panel`：C++ 的 `CandidatePanelLimit::IbusPanel`、Rust 的 `CandidatePanelLimit::IbusPanel`（serde 名 `ibus_panel`）、TypeScript 的 `candidate_panel_limit` 联合类型同步加上这一项。
- IBus 宿主的结论收进 `ibus_candidate_panel_limit(gnome_shell)`：GNOME Shell 下仍是 `gnome_shell`（全部不生效），其余一律 `ibus_panel`。IBus 没有全部生效的情形，所以 IBus 宿主不再写 `null`。Fcitx5 的判断不变：classicui 用水杉主题时写 `null`，用户自选主题时 `fcitx_theme`，Kimpanel 时 `kimpanel`。
- 设置页的说明文案：「IBus 的候选窗口由桌面的输入法面板绘制，只接收候选字体和颜色：这里的圆角、边框颜色和皮肤的装饰图在 IBus 下不会生效。」与另外三条一样，显示在「候选窗口」页的预览下方和「主题」页顶部。
- 能力位 `candidate_corner_radius`、`candidate_border_color` 保持按平台声明，控件照常显示：它们说的是 Fcitx5，IBus 那一半由运行时的限制说明。

## Alternatives considered

- **把 Linux 的 `candidate_corner_radius`、`candidate_border_color` 改成 false**：最强的理由是简单、静态，IBus 用户不会再看到没用的控件。但能力位按平台给，Fcitx5（classicui 主题真的画圆角、边框和装饰图）的用户会失去能用的设置；这正是审计里「改静态位」那条路的问题。
- **读到 `ibus_panel` 时在设置页隐藏这两个控件**：IBus 用户看不到死控件。没有这么做：偏好在两个框架之间共享，状态文件以最后写入的宿主为准，同一台机器换框架时控件会忽隐忽现，在 IBus 下调过的值也无处可看；现有的 `gnome_shell`（连字体和颜色都不生效）同样只给说明、不藏控件，保持一致。
- **维持 `null`，只在 README 里写**：README 本来就写了，但用户在设置页里看不到。

## Consequences

收益：在 IBus 面板下，外观页和主题页说清圆角、边框颜色和皮肤装饰图不会生效，字体和颜色仍然生效；Fcitx5 用户看到的一切不变。

代价与缺口：`ibus_panel` 只按「是不是 GNOME Shell」判断，不区分 Plasma 上的 kimpanel-ibus-panel 等其他 IBus 面板，那些面板可能连字体和颜色也不读，说明在那里偏乐观。旧版设置应用读到 `ibus_panel` 时按未知名字处理为没有限制（`from_host_status` 的既有约定），不会报错。说明不是按控件逐个标注，而是在页首一次说完，与现有三条限制相同。

## Verification

- `platforms/linux/tests/candidate/candidate_panel_status.cpp`（ctest `linux-candidate-panel-status`）：`ibus_panel` 的文档逐字节、`ibus_candidate_panel_limit` 在 GNOME Shell 与否时的结论。
- `platforms/linux/tests/runtime/engine_smoke.cpp` 在各模式共用的开头（获得焦点之后）断言运行中的 IBus 宿主把 `{"host":"ibus","limit":"ibus_panel"}` 写进运行目录下的 `candidate-panel.json`（测试先清掉 `XDG_CURRENT_DESKTOP`，结论与跑测试的桌面无关）。在 `build-container.sh` 的镜像里用已校验的词库跑 ctest 登记的五个模式：换回未改的 `ClientEngine.cpp` 时每个模式都停在这条断言上（读到的是 `"limit":null`），改后全部通过。
- `crates/client-core/src/host_surface/tests.rs` 的 `candidate_panel_limit_reads_only_what_the_host_reported`：宿主写出的那一行读成 `IbusPanel`，`HostCapabilities` 序列化为 `"ibus_panel"` 并能读回。
- `apps/desktop/src-tauri/src/tests.rs` 的 `candidate_panel_status_reader_reports_the_ibus_panel_limit`：设置应用经 `read_candidate_panel_status` 读到这份文件，得到 `IbusPanel`，「关于」页的输入法框架仍是 IBus。
- `apps/desktop/tests/input/host-capabilities.test.tsx`：`ibus_panel` 的说明在「主题」与「候选窗口」两页各出现一次，没有限制时不出现。

GNOME Classic 的 Shell 会话分类修复见 [GNOME Classic 使用 Shell 精简菜单](../../implemented/bug-fix/2026-10-31-linux-gnome-classic-menu.md)。
