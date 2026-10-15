/** 繁体输出用的 OpenCC 配置，与偏好 `traditional_chinese_variant` 的取值相同。 */
export type TraditionalChineseVariant = "s2t" | "s2tw" | "s2twp";

/** 「中文输出」单选的取值：简体，或某一种繁体配置。 */
export type ChineseOutputForm = "simplified" | TraditionalChineseVariant;

export interface TraditionalChineseOutputValue {
  traditional_chinese_output: boolean;
  traditional_chinese_variant: TraditionalChineseVariant;
}

/** 宿主不支持变体时按标准繁体显示，保留文档里的原选择。 */
export function chineseOutputForm(
  value: boolean | undefined,
  variant: TraditionalChineseVariant | undefined,
  variants: boolean | undefined,
): ChineseOutputForm {
  return value ? (variants ? (variant ?? "s2t") : "s2t") : "simplified";
}

/** 只写本次选择改变的偏好；切到简体或不支持变体时不覆盖已有变体。 */
export function chineseOutputPatch(
  next: ChineseOutputForm,
  variant: TraditionalChineseVariant | undefined,
  variants: boolean | undefined,
): Partial<TraditionalChineseOutputValue> {
  if (next === "simplified") return { traditional_chinese_output: false };
  if (!variants || next === (variant ?? "s2t")) return { traditional_chinese_output: true };
  return { traditional_chinese_output: true, traditional_chinese_variant: next };
}
