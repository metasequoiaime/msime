export type CandidatePanelLimit = "gnome_shell" | "fcitx_theme" | "kimpanel";

const notes: Record<CandidatePanelLimit, string> = {
  gnome_shell:
    "GNOME Shell 自己绘制 IBus 候选窗并跟随 Shell 主题，这里的候选字体、颜色和皮肤在当前桌面不会生效。",
  fcitx_theme:
    "Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题，这里的候选颜色和皮肤不会覆盖它；字体仍然生效。改回 Fcitx5 默认主题后即可使用这里的设置。",
  kimpanel:
    "Fcitx5 的候选窗由桌面的 Kimpanel 绘制，使用桌面自己的字体和主题，这里的候选字体、颜色和皮肤不会生效。",
};

export interface CandidatePanelLimitSectionProps {
  limit: CandidatePanelLimit;
}

/** Explains which Linux desktop surface owns the candidate panel appearance. */
export function CandidatePanelLimitSection({ limit }: CandidatePanelLimitSectionProps) {
  return (
    <div className="section">
      <small>{notes[limit]}</small>
    </div>
  );
}
