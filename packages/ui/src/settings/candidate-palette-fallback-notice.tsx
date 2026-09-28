/** Explains why the candidate palette controls are inactive while the keyboard skin supplies colors. */
export function CandidatePaletteFallbackNotice() {
  return (
    <div className="section">
      <small>
        候选栏正在使用键盘皮肤的颜色，下面的候选颜色要在「皮肤」页打开「使用桌面候选皮肤」后才生效。
      </small>
    </div>
  );
}
