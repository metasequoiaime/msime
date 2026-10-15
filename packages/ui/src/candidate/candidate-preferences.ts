export type CandidateSizingPreferences = {
  candidate_font_size?: number;
  candidate_preedit_font_size?: number;
};

export type CandidateFontPreferences = {
  candidate_font_family?: string;
  candidate_english_font?: string | null;
  candidate_fallback_fonts?: string[];
};

/** 候选窗口样式；范围由 `client-core` 的 `Preferences::validate` 校验。 */
export type CandidateWindowStylePreferences = {
  /** 整窗缩放百分比（50–200，缺省为 100），同时缩放字号与几何尺寸；核心不写入默认值。 */
  candidate_scale_percent?: number;
  /** 卡片填充、边框和皮肤背景的不透明度（50–100，缺省为 100），文字和焦点高亮保持不透明。 */
  candidate_opacity_percent?: number;
  /** 卡片圆角（0–32pt）；缺省或 `null` 时先跟随皮肤，再跟随宿主。 */
  candidate_corner_radius?: number | null;
  /** 缩放前的卡片内边距（0–16pt）；缺省或 `null` 时先跟随皮肤，再跟随宿主。 */
  candidate_padding?: number | null;
};
