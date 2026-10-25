# Agent Note: Linux 设置提示打开能改那份输入法列表的界面，首次配置结果卡片的长网址不再溢出

Status: implemented

## Problem

2026-10-25 在 Ubuntu 24.04（GNOME 46）上验收 nightly 0.13.0 时发现三处界面问题。

一是 GNOME + IBus 时设置页顶部提示「水杉输入法尚未加入系统输入法列表」，按钮「检查设置」打开的是 `ibus-setup`。`msime-linux-setup` 注册和查询时都按 `ibus_input_list` 选列表：`XDG_CURRENT_DESKTOP` 含 `GNOME` 且装有 `org.gnome.desktop.input-sources` 时用 GNOME 的输入源，否则用 IBus 的 `preload-engines`；GNOME Shell 只读前者。`ibus-setup` 改的是后者，所以在里面加完水杉，`--status --json` 仍是 `enabled:false`。根因是设置应用 `open_linux_input_method_settings` 只按 `host` 选工具（`ibus` 一律 `ibus-setup`），而状态查询没有把「该改哪份列表」报告出来，Rust 一侧无从区分。

二是偏好文件读不出来时，同一条提示上的按钮也叫「检查设置」，动作还是 `openSetup`，即打开系统的输入法配置工具，与偏好无关，按了让人以为在修复。

三是 Fcitx5 首次配置的结果卡片上，提示文字向两侧溢出，左边被滚动区裁掉并出现横向滚动条。卡片 `LinuxInputMethodNotice` 是 `items-center` 的纵向 flex，提示列表按内容的最小宽度排版；脚本的 GNOME Wayland 提示（`GNOME_WAYLAND_GTK_NOTE`）带着 README 的完整网址，网址不能断行，列表的最小宽度就是网址的宽度，比卡片宽，居中后向两侧溢出。用 WebKit 和 Chromium 按真实提示文字复现：500 宽的窗口里列表宽 571、左边到 -35。

## Decision

- `msime-linux-setup --status --json` 多报告一个 `list`：`fcitx5`（Fcitx5 当前输入法组）、`gnome`（GNOME 输入源）、`ibus`（IBus 预载引擎），查不出时为 `null`。IBus 下取值直接来自 `ibus_input_list` 选中的 schema，与注册、注销用的是同一个判定，不在 Rust 里另读 `XDG_CURRENT_DESKTOP`。
- 设置应用 `input_method_settings_command` 按 `host` 和 `list` 选工具：Fcitx5 是 `fcitx5-configtool`；`list` 为 `gnome` 时是 `gnome-control-center keyboard`，`gnome-control-center --version` 的主版本小于 40 时是 `gnome-control-center region`（GNOME 40.beta 的 NEWS 记着键盘面板改版、开始管理输入源，之前输入源在「区域和语言」面板），读不出版本按现行版本打开 `keyboard`；其余 IBus（非 GNOME 桌面，以及没有 `list` 的旧版脚本）仍是 `ibus-setup`。只有 GNOME 分支才去问版本。
- `OobeStatusNotice` 的按钮按提示给：资源未准备好是「修复运行资源」；偏好读不出来时不给宿主动作，只留「重新检查」；未加入输入法列表时叫「去添加」（与首页状态卡、鸿蒙警告条同一个词）；不是当前输入法时仍是「检查设置」。macOS 共用这个组件，偏好读不出来时同样不再给打开输入源设置的按钮。
- `LinuxInputMethodNotice` 的卡片加 `overflow-wrap:anywhere`，让长网址和路径在计算最小宽度时也能断开；提示列表 `self-stretch` 撑满卡片宽度、左对齐换行。不改脚本的提示文字，网址仍在，终端里照样可以复制。

## Alternatives considered

- 设置应用自己读 `XDG_CURRENT_DESKTOP` 判断 GNOME：不用改脚本的 JSON，改动只在 Rust。没有采用：注册写哪份列表由脚本决定（还要看 schema 是否装了），两处各判一次迟早会不一致，比如 GNOME 桌面上没装 `org.gnome.desktop.input-sources` 时脚本写的是 `preload-engines`，这时打开 GNOME 设置反而帮不上忙。
- 一律打开 `gnome-control-center keyboard`，不看版本：少一次子进程。没有采用：项目仍支持 GNOME 40 之前的发行版（Ubuntu 20.04 的 3.36、Debian 11 的 3.38），那里没有管输入源的键盘面板，打开的是不相干的页面。
- 偏好读不出来时让按钮去打开首次配置或修复页：看起来像是「能修」。没有采用：现有的修复事务只处理词库资源，不重写偏好；给一个名不副实的按钮正是这次要去掉的问题。
- 给结果页的滚动区加 `overflow-x-hidden`：一行就能去掉横向滚动条。没有采用：溢出的文字仍然被裁掉，只是看不见滚动条了。

## Consequences

GNOME + IBus 的用户点「去添加」进到 GNOME 设置的键盘面板，在「输入源」里加完，状态查询和提示立即一致；非 GNOME 桌面和 Fcitx5 打开的工具不变。代价是 GNOME 分支多跑一次 `gnome-control-center --version`（3 秒上限，读不出就按现行版本处理），`--status --json` 的消费者多了一个键，旧键不变。偏好读不出来时提示上不再有动作按钮，用户只能按提示文字重新打开设置；真正的偏好恢复入口仍然缺失，这里不新增。结果卡片里任何长网址或路径都会在卡片内折行，网址可能断在任意字符处。

验证：Python 脚本测试覆盖 GNOME 桌面上预载引擎里有水杉但输入源里没有时报告 `list:"gnome"`、`enabled:false`，KDE 上报告 `list:"ibus"`、`enabled:true`，在旧脚本上失败；Rust 单测 `input_method_settings_open_the_tool_that_edits_the_reported_list` 覆盖四种组合和 GNOME 版本解析；vitest 覆盖提示按钮和结果卡片的样式，均在旧实现上失败。布局用 WebKit 和 Chromium 按 448×640、500×640、1280×800 渲染核对，修复后滚动区 `scrollWidth` 等于 `clientWidth`。没有在 Ubuntu 实机上点过新的「去添加」，也没有在 GNOME 40 之前的系统上验证 `region` 面板。

## Related decisions

- [全平台 OOBE 与系统能力状态分离](../architecture/2026-10-25-cross-platform-oobe-capability-state.md) 定义了设置页提示所读的 OOBE 状态和资源修复、系统设置两个动作。
- [Linux 共享面板与首次配置的交互对齐 macOS](../feature/2026-10-20-linux-panels-and-setup-mac-parity.md) 记录了首次配置结果卡片和 `msime-setup-note:` 提示行。
