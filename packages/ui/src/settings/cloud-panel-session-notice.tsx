import * as settings from "./settings-style";

/** Where macOS users open the cloud clipboard: pasting needs the input method's own session, so the settings window points at the entry the input method offers instead of opening it itself. The cloud dictionary only manages words and opens normally from settings. */
export const CLOUD_PANEL_SESSION_NOTE =
  "云剪贴板需要当前输入法进程提供输入会话才能直接粘贴；请从输入法菜单中的「云剪贴板…」打开。";

/** Explains where macOS users can open the cloud clipboard, which needs an input session. */
export function CloudPanelSessionNotice() {
  return <p className={settings.panelPreviewLabel}>{CLOUD_PANEL_SESSION_NOTE}</p>;
}
