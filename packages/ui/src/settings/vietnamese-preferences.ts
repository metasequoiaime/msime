/** 越南语输入页和各宿主共同使用的偏好模型。缺省字段表示用户还没有选择过。 */
export type VietnamesePreferences = {
  input_method?: "telex" | "vni";
  tone_style?: "modern" | "classic";
};

/** 把可选的落盘偏好投影为设置控件和输入宿主使用的有效值。 */
export function resolveVietnamesePreferences(preferences?: VietnamesePreferences) {
  return {
    input_method: preferences?.input_method ?? "telex",
    tone_style: preferences?.tone_style ?? "modern",
  } as const;
}
