/** 韩语实体键盘布局；缺字段或未来版本的未知值回退到两套式。 */
export type KoreanPreferences = {
  layout?: "dubeolsik" | "romaja";
};

export function resolveKoreanLayout(preferences?: KoreanPreferences) {
  return preferences?.layout === "romaja" ? "romaja" : "dubeolsik";
}

export const koreanLayoutDescription =
  "默认两套式，可切换为罗马字。罗马字用 x 输入声母 ㅇ（xa → 아，hangug → 한국），Shift+G/D/B/J/S 输入紧音；退格删除当前拼写的最后一个拉丁字母，标点为半角";
