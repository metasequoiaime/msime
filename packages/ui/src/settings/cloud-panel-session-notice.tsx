import * as settings from "./settings-style";

/** Explains where macOS users can open cloud panels that need an input session. */
export function CloudPanelSessionNotice() {
  return (
    <p className={settings.panelPreviewLabel}>
      云剪贴板和云词典需要当前输入法进程提供输入会话；请从输入法悬浮工具栏或输入法菜单打开对应面板。
    </p>
  );
}
