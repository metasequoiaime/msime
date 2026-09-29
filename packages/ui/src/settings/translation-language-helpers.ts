export type TranslationLanguage = "en" | "fr" | "ja" | "es" | "ru" | "de" | "ko";

export const translationLanguages: [TranslationLanguage, string][] = [
  ["en", "英语"],
  ["fr", "法语"],
  ["ja", "日语"],
  ["es", "西班牙语"],
  ["ru", "俄语"],
  ["de", "德语"],
  ["ko", "韩语"],
];

export const translationSecondaryLanguages: ["" | TranslationLanguage, string][] = [
  ["", "不显示第二种语言"],
  ...translationLanguages,
];

export const mobileTranslationLanguages = translationLanguages.filter(([value]) => value !== "ru");
