/** Explains why font controls are unavailable on hosts that draw the panel themselves. */
export function CandidateFontUnsupportedNotice() {
  return (
    <div className="section">
      <small>当前宿主的候选面板不支持自定义字体或字号。</small>
    </div>
  );
}
