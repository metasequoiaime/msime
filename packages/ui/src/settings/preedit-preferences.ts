export type CandidatePreeditStyle = "pinyin" | "empty";
export type ShuangpinPreeditStyle = "raw" | "raw_separator" | "pinyin";
export type PreeditStyle = ShuangpinPreeditStyle | CandidatePreeditStyle;
export type TsfPreeditStyle = PreeditStyle;

export type PreeditPreferences = {
  /** 双拼预编辑是否保留原始按键；缺省为保留，设置入口由宿主能力决定。 */
  shuangpin_preedit_uses_raw?: boolean;
  /** 双拼预编辑保留原始按键并在音节之间显示撇号；缺省关闭。 */
  shuangpin_preedit_shows_separator?: boolean;
  /** 行内（宿主绘制）预编辑；Linux 通过 `ClientEngine.preedit_style()` 应用。 */
  tsf_preedit_style?: TsfPreeditStyle;
  candidate_preedit_style?: CandidatePreeditStyle;
};

/** 设置控件与候选预览共用的有效显示状态；缺省值不写回偏好。 */
export function preeditControlState(preferences: PreeditPreferences): {
  candidateStyle: CandidatePreeditStyle;
  candidateVisible: boolean;
  shuangpinStyle: ShuangpinPreeditStyle;
  inlineStyle: TsfPreeditStyle;
} {
  const candidateStyle = preferences.candidate_preedit_style ?? "pinyin";
  return {
    candidateStyle,
    candidateVisible: candidateStyle !== "empty",
    shuangpinStyle:
      preferences.shuangpin_preedit_shows_separator === true
        ? "raw_separator"
        : preferences.shuangpin_preedit_uses_raw === false
          ? "pinyin"
          : "raw",
    inlineStyle: preferences.tsf_preedit_style ?? "raw",
  };
}
