import { SettingsGroupNote } from "./settings-group-note";

/** Linux 宿主不画悬浮工具栏窗口，「悬浮工具栏」页首用这段说明代替预览：工具栏在 IBus 与 Fcitx5 下是输入法菜单里的「工具栏」子菜单，缩放和图标尺寸没有对应的面。`buttons` 为假时宿主没有按钮勾选项，只提显示开关。`gnomeShell` 是宿主报告 GNOME Shell 面板（`candidate_panel_limit` 为 `gnome_shell`，只有 IBus 宿主会报）的情形：IBus 在那里只发布输入模式和设置两项，没有工具栏子菜单，这一页的设置都不生效，所以照实说，不再说开关仍然生效。 */
export function FloatingToolbarPlatformNotice({
  buttons = true,
  gnomeShell = false,
}: {
  buttons?: boolean;
  gnomeShell?: boolean;
}) {
  if (gnomeShell)
    return (
      <SettingsGroupNote>
        {
          "Linux 不显示悬浮工具栏窗口。当前是 GNOME 桌面，IBus 在输入源菜单里只列出输入模式和设置，没有「工具栏」子菜单，这一页的设置在当前桌面不生效。"
        }
      </SettingsGroupNote>
    );
  const stillApplies = buttons ? "下方的按钮选择和显示开关仍然生效" : "下方的显示开关仍然生效";
  return (
    <SettingsGroupNote>
      {`Linux 不显示悬浮工具栏窗口，工具栏以输入法菜单里的「工具栏」子菜单呈现，缩放和图标尺寸不适用；${stillApplies}。GNOME 桌面下的 IBus 只在输入源菜单列出输入模式和设置，没有这个子菜单。`}
    </SettingsGroupNote>
  );
}
