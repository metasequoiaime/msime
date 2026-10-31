# Agent Note: GNOME Classic 使用 Shell 精简菜单

Status: implemented

## Problem

#10358 报告 GNOME 顶部输入源菜单过长且子菜单打不开。当前宿主在 GNOME Shell 下已只发布模式与设置两项，但 `candidate_desktop_is_gnome_shell` 把 `GNOME-Classic` 放进排除列表。GNOME Classic 是 Shell 的扩展组合，其正式会话文件的 `DesktopNames` 为 `GNOME-Classic;GNOME;`，排除会使它回到完整属性树，候选外观的能力报告也随之错误。报告者尚未提供具体会话，不能断言此处就是截图根因。

## Decision

`candidate_desktop_is_gnome_shell` 把 `GNOME-Classic` 与 `GNOME` 一样识别为 Shell 会话，继续由既有 `org.gnome.Shell` 总线名称确认正在运行的 Shell。Flashback、Budgie、Unity、Pantheon 等自带面板的会话仍排除。覆盖 Classic 单独标识、正常和反序组合、以及与排除桌面混合时的优先级；不更改菜单内容或其他桌面默认值。

## 既有笔记审计

- [Linux 模式徽标与菜单](../../implemented/feature/2026-10-20-linux-mode-badge-and-menu-mac-parity.md)：部分重叠，继续持有完整桌面菜单和 GNOME 精简分支的行为；本篇只修桌面分类。
- [IBus 面板限制](../../implemented/bug-fix/2026-10-13-linux-ibus-panel-limit.md)：部分重叠，同一分类还决定 `gnome_shell` 和 `ibus_panel`，补齐 Classic 后报告与实际绘制方一致。
- 其余 `Classic` 命中属于 Fcitx classicui 主题和 Android 别名，不涉及 GNOME 会话，均无关。

## Alternatives considered

- **保留 Classic 排除，只在用户环境里改桌面名**：不用改产品，但正式会话就是这个标识，要求用户伪装环境绕过确定错误没有必要。
- **删除所有排除项，仅根据名称里出现 GNOME 判断**：规则更短，但 Flashback 和 Budgie 也可能带 GNOME 标识，它们的面板并非 Shell；保留既有排除优先级。

## Verification

- 新增 `GNOME-Classic:GNOME` 断言在原实现上失败（退出 134）；修复后完整 `candidate_panel_status.cpp` 在 macOS Clang 和已有 Linux 构建门禁容器内的 g++ 下通过，均使用 C++17、`-Wall -Wextra -Werror`，未关断言。
- Linux 容器复用已有镜像，只编译本专项，没有创建完整构建树。
- 最终 `bash scripts/verify-local.sh --quick` exit 0；首次因共用 Apple bridge 缓存指向另一工作树失败，在原目录 `cmake --fresh` 后完整重跑通过。远端缺少 Node，笔记三项门禁在本机通过；`git diff --check` 通过。

## Consequences

Classic 会使用既有模式／设置两项的菜单分支，同时外观能力准确报告为 `gnome_shell`；其余桌面分类不变。代价是 Classic 与普通 Shell 一样不再在顶栏暴露完整设置树，完整功能由设置应用承载。

## 验证缺口

尚未在 GNOME 桌面和报告者环境里验收，也没有证据说明其会话一定是 Classic。若报告者使用普通 GNOME，仍需继续核查运行版本、启动环境与 Shell 名称所有权，合并后仍请报告者复验；普通 GNOME 若仍出现完整菜单，继续核查而不把 Classic 的通过当作其环境验收。

## Sources

- [GNOME 官方 Classic 说明](https://help.gnome.org/gnome-help/gnome-classic.html)
- 远端实际默认分支 `main`，固定提交 `d3ca60dc974fd9fd1d1aa889163d8954fcf842f9`：[GNOME 官方会话文件](https://github.com/GNOME/gnome-shell-extensions/blob/d3ca60dc974fd9fd1d1aa889163d8954fcf842f9/data/gnome-classic.desktop.in)
