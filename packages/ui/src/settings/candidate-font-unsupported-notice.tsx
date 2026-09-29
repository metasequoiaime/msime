import * as settings from "./settings-style";

/** Explains why font controls are unavailable on hosts that draw the panel themselves. */
export function CandidateFontUnsupportedNotice() {
  return (
    <p className={settings.groupNote}>当前宿主的候选面板不支持自定义字体或字号。</p>
  );
}
