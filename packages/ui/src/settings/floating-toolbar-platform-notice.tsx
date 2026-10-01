import * as settings from "./settings-style";

/** Explains the limited toolbar controls on hosts that render it in the input method menu, naming only the controls this host still offers: the switch in 显示 always, the 按钮 checks when the host has them. */
export function FloatingToolbarPlatformNotice({ buttons = true }: { buttons?: boolean }) {
  const stillApplies = buttons ? "上方的按钮选择和显示开关仍然生效" : "上方的显示开关仍然生效";
  return (
    <p className={settings.groupNote}>
      {`当前宿主以输入法菜单呈现工具栏，缩放和图标尺寸不适用；${stillApplies}。`}
    </p>
  );
}
