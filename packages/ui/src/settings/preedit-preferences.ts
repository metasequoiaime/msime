export type CandidatePreeditStyle = "pinyin" | "empty";
export type PreeditStyle = "raw" | CandidatePreeditStyle;
export type TsfPreeditStyle = PreeditStyle;
/** 「双拼预编辑」的三档：原始按键（`nimf`）、双拼分隔（按音节切开的原始按键 `ni'mf`）、拼音分词（展开的全拼 `ni'men`）。 */
export type ShuangpinPreeditStyle = "raw" | "separated" | "pinyin";

export type PreeditPreferences = {
  /** 双拼预编辑是否保留原始按键；缺省为保留，设置入口由宿主能力决定。 */
  shuangpin_preedit_uses_raw?: boolean;
  /** 保留原始按键时是否按音节插 `'`（「双拼分隔」）；缺省为否，等于缺省时文档里没有这一项。拼音分词时不读。 */
  shuangpin_preedit_separated?: boolean;
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
    shuangpinStyle: shuangpinPreeditStyle(preferences),
    inlineStyle: preferences.tsf_preedit_style ?? "raw",
  };
}

function shuangpinPreeditStyle(preferences: PreeditPreferences): ShuangpinPreeditStyle {
  if (preferences.shuangpin_preedit_uses_raw === false) return "pinyin";
  return preferences.shuangpin_preedit_separated === true ? "separated" : "raw";
}

/** 选中的双拼预编辑样式写回共享偏好的两个键：拼音分词时清掉分隔，免得以后改回原始按键时它又悄悄生效。 */
export function shuangpinPreeditPatch(
  style: ShuangpinPreeditStyle,
): Pick<PreeditPreferences, "shuangpin_preedit_uses_raw" | "shuangpin_preedit_separated"> {
  return {
    shuangpin_preedit_uses_raw: style !== "pinyin",
    shuangpin_preedit_separated: style === "separated",
  };
}
