import * as settings from "./settings-style";

/** Where macOS users open the cloud panels: they need the input method's own session, so the settings window points at the entry the input method offers instead of opening them itself. */
export const CLOUD_PANEL_SESSION_NOTE =
  "云剪贴板和云词库需要当前输入法进程提供输入会话；请从输入法菜单中的「云剪贴板…」打开云剪贴板。";

/** Explains where macOS users can open cloud panels that need an input session. */
export function CloudPanelSessionNotice() {
  return <p className={settings.panelPreviewLabel}>{CLOUD_PANEL_SESSION_NOTE}</p>;
}
