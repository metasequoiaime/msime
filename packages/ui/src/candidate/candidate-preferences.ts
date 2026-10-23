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
  /** 窗口分轴内边距（0–16pt）；缺省时沿用旧的统一内边距。 */
  candidate_padding_horizontal?: number | null;
  candidate_padding_vertical?: number | null;
  /** 候选项每侧内边距（0–16pt），缺省时跟随皮肤与宿主。 */
  candidate_item_padding_horizontal?: number | null;
  candidate_item_padding_vertical?: number | null;
  /** 选中与悬停背景的独立圆角（0–32pt）。 */
  candidate_item_corner_radius?: number | null;
  /** 横排列距或竖排行距（0–16pt）。 */
  candidate_item_spacing?: number | null;
  /** 候选窗口严格按文字所需的宽度收紧，不垫出厂最小宽度（Windows `appearance.candidate_window_fit_content`，出厂关）；与皮肤 `fit_content` 是或关系。关着时不写入文档。 */
  candidate_window_fit_content?: boolean;
};
