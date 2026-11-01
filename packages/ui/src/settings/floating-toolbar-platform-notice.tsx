import { SettingsGroupNote } from "./settings-group-note";

/** 只有菜单、不画悬浮工具栏窗口的 Linux 宿主（不报告 `floating_toolbar_appearance`），「悬浮工具栏」页首用这段说明代替预览：工具栏在 IBus 与 Fcitx5 下是输入法菜单里的「工具栏」子菜单，「工具栏大小」没有对应的面；Fcitx5 的中英文切换提示有自己的固定大小（`platforms/linux/src/overlay/ModeBadgeStyle.h`），也不读共享文档里的工具栏大小，所以说明写「工具栏大小不适用」是照实。`buttons` 为假时宿主没有按钮勾选项，只提显示开关。`gnomeShell` 是宿主报告 GNOME Shell 面板（`candidate_panel_limit` 为 `gnome_shell`，只有 IBus 宿主会报）的情形：IBus 在那里只发布输入模式和设置两项，没有工具栏子菜单，这一页的设置都不生效，所以照实说，不再说开关仍然生效。 */
export function FloatingToolbarPlatformNotice({
  buttons = true,
  gnomeShell = false,
}: {
  buttons?: boolean;
  gnomeShell?: boolean;
}) {
  if (gnomeShell)
    return (
      <SettingsGroupNote inline>
        {
          "Linux 不显示悬浮工具栏窗口。当前是 GNOME 桌面，IBus 在输入源菜单里只列出输入模式和设置，没有「工具栏」子菜单，这一页的设置在当前桌面不生效。"
        }
      </SettingsGroupNote>
    );
  const stillApplies = buttons ? "下方的按钮选择和显示开关仍然生效" : "下方的显示开关仍然生效";
  return (
    <SettingsGroupNote inline>
      {`Linux 不显示悬浮工具栏窗口，工具栏以输入法菜单里的「工具栏」子菜单呈现，工具栏大小不适用；${stillApplies}。GNOME 桌面下的 IBus 只在输入源菜单列出输入模式和设置，没有这个子菜单。`}
    </SettingsGroupNote>
  );
}

/** 画悬浮工具栏的 Linux 宿主放在预览下的说明：窗口画在 X11 上，Wayland 桌面经 Xwayland 显示；Wayland 桌面上看不出原生应用是否全屏，所以只在那里输入框失焦时工具栏也收起（`FloatingToolbar.h` 的 `hide_on_focus_out`）。没有 X 服务器可连或使用不支持独立浮层的 xwayland-satellite 时画不出来，工具栏退回输入法菜单里的「工具栏」子菜单，按钮选择和显示开关两处都照样生效（`platforms/linux/src/overlay/FloatingToolbarFactory.cpp`）。`gnomeShell` 是宿主报告 GNOME Shell 面板（`candidate_panel_limit` 为 `gnome_shell`）的情形：IBus 在那里的输入源菜单只发布输入模式和设置两项（`ClientEngine.cpp` 的 `publish_mode`），没有「工具栏」子菜单可退，也没有「显示工具栏」开关，所以照实说工具栏要靠 Xwayland、只能在这一页开关（或用工具栏右键菜单里的「隐藏悬浮状态栏」关掉）。 */
export function FloatingToolbarLinuxWindowNotice({ gnomeShell = false }: { gnomeShell?: boolean }) {
  const drawn =
    "Linux 在 X11 上显示悬浮工具栏，Wayland 桌面经 Xwayland 显示，在 Wayland 桌面上离开输入框时工具栏随之收起。使用 xwayland-satellite 的桌面不显示悬浮窗口。";
  return (
    <SettingsGroupNote inline>
      {gnomeShell
        ? `${drawn}当前是 GNOME 桌面，IBus 的输入源菜单里没有「工具栏」子菜单，没有 Xwayland 时就没有工具栏；工具栏只能在这一页打开或关闭，也可以用设置按钮右键菜单里的「隐藏悬浮状态栏」关掉。`
        : `${drawn}显示不出来时，工具栏以输入法菜单里的「工具栏」子菜单呈现，下方的按钮选择和显示开关同样生效。`}
    </SettingsGroupNote>
  );
}
