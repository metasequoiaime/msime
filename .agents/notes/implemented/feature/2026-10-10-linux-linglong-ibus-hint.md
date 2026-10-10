# Agent Note: 带玲珑的系统上用 IBus 时提示改用 Fcitx5

Status: implemented

## Problem

#6410：deepin 25 上，玲珑（Linglong，linyaps）格式的应用（Chrome、Firefox、文本编辑器）里切到水杉的中文模式也只能打出字母，非玲珑应用正常。

读 linyaps 的源码得到的结论：玲珑应用跑在自己的运行时容器里，GTK/Qt 的输入法模块取自容器内的运行时，而运行时只带 Fcitx5 的 GTK2/3 和 Qt6 模块，没有任何 IBus 模块；会话 D-Bus 和 `~/.config` 都挂进了容器。于是框架是 Fcitx5 时玲珑应用能连上宿主的 Fcitx5，框架是 IBus 时应用里 `GTK_IM_MODULE=ibus` 找不到模块，退回 GTK 自带的简单输入。根因在玲珑运行时，水杉改不了容器里装了哪些模块。

## Decision

`msime-linux-setup` 在首次配置和 `--register` 结束时，若输入法注册进的是 IBus，且系统是 deepin/UOS（`os-release` 的 `ID` 或 `ID_LIKE`）或装了玲珑（`ll-cli` 在 PATH 里，或有 `/var/lib/linglong`），打印一句提示：玲珑应用里只能打出字母的原因，以及改用 Fcitx5 的命令（`im-config -n fcitx5`、重新登录、`msime-linux-setup --register`），并指向 README 新增的「deepin 玲珑应用里只能输入字母」一节。判定写成纯函数 `linglong_ibus_note(host, release, linglong)`，与 GNOME Wayland 的 `gnome_wayland_gtk_note` 同一个形状，紧随其后打印。只提示，不替用户切换框架、不改任何文件。

设置窗口的首次配置页运行的就是这个脚本，输出逐行显示，所以提示在那里同样可见，不必在 Tauri 侧另写一份检测。

非 deepin/UOS、也没装玲珑的系统，以及注册进 Fcitx5 的情况，输出与之前完全相同。

## Alternatives considered

- **在设置页加一个常驻的环境检测卡片**：用户每次打开设置都能看到，比只在配置时打印一次更显眼。但要新增一条 Tauri 命令去读 os-release、判断当前框架，再在共享 React 页面里按平台和能力位显示，跨 Rust、TypeScript 两层还要各自的测试，而这条限制只影响带玲珑的系统上用 IBus 的少数用户。首次配置页已经显示脚本输出，先用脚本这一处覆盖；若反馈表明用户是在配置完很久之后才遇到，再考虑做卡片。
- **检测到 deepin 时自动切到 Fcitx5（改 im-config、往 Fcitx5 里注册）**：省掉用户的手动步骤。但切换框架会改变全系统所有应用的输入方式，还可能让用户已经配好的其他 IBus 输入法失效，这是用户自己的决定；GNOME Wayland 的那条提示同样只提示不改文件。
- **只看 `ID=deepin`**：最贴近 issue。但统信 UOS 同样默认带玲珑，别的发行版也能另装 linyaps，按 `ll-cli`/`/var/lib/linglong` 判断才对得上真正的条件（容器里没有 IBus 模块），os-release 只是在玲珑尚未装上时补一层。

## Consequences

deepin/UOS 上的 IBus 用户在配置时就能知道玲珑应用的限制和绕过办法，不必等撞上了再来提 issue。代价是一条可能出现在「装了玲珑但从不用玲珑应用」的用户屏幕上的提示，它只在配置命令结束时打印一次。

这不修复根因：玲珑运行时补上 IBus 模块（或允许宿主的模块进容器）之前，IBus 用户在玲珑应用里仍然只能打出字母。改用 Fcitx5 后仍不能输入的，README 列出了需要用户补充的信息（玲珑应用里的 `GTK_IM_MODULE`/`QT_IM_MODULE`/`XMODIFIERS`、deepin 自带拼音在同一应用里是否可用、Fcitx5 `DebugInfo` 里有没有该应用的输入上下文），用来区分是模块缺失还是连接问题。

## Verification

`platforms/linux/tests/core/setup_linglong.py`（ctest 名 `linux-setup-linglong`）覆盖：deepin、UOS、`ID_LIKE` 含 deepin、其他发行版装了玲珑时提示；注册进 Fcitx5、没有运行的框架、没有玲珑的其他发行版时不提示；os-release 的引号去除与缺文件。没有在 deepin 真机和玲珑应用上验证。
