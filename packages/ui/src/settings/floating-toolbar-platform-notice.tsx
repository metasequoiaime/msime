import * as settings from "./settings-style";

/** 在把工具栏放进输入法菜单的宿主上，说明能调的工具栏控件有限，并且只列出该宿主仍提供的控件：「显示」里的开关总会列出，「按钮」里的勾选项在宿主有时才列出。 */
export function FloatingToolbarPlatformNotice({ buttons = true }: { buttons?: boolean }) {
  const stillApplies = buttons ? "上方的按钮选择和显示开关仍然生效" : "上方的显示开关仍然生效";
  return (
    <p className={settings.groupNote}>
      {`当前宿主以输入法菜单呈现工具栏，缩放和图标尺寸不适用；${stillApplies}。`}
    </p>
  );
}
