export type TranslationLanguage = "en" | "fr" | "ja" | "es" | "ru" | "de" | "ko";
export type TranslationSecondaryLanguage = TranslationLanguage | "";
export type TranslationLanguageOption = readonly [TranslationLanguage, string];
export type TranslationSecondaryLanguageOption = readonly [TranslationSecondaryLanguage, string];

export const translationLanguages: [TranslationLanguage, string][] = [
  ["en", "英语"],
  ["fr", "法语"],
  ["ja", "日语"],
  ["es", "西班牙语"],
  ["ru", "俄语"],
  ["de", "德语"],
  ["ko", "韩语"],
];

export const translationSecondaryLanguages: [TranslationSecondaryLanguage, string][] = [
  ["", "不显示第二种语言"],
  ...translationLanguages,
];

export const mobileTranslationLanguages = translationLanguages.filter(([value]) => value !== "ru");

export function translationLanguageChoices(
  mobile: boolean,
  translationTargetLanguage: TranslationLanguage,
  translationSecondaryLanguage: TranslationSecondaryLanguage,
) {
  const visibleTranslationLanguages = mobile
    ? [...mobileTranslationLanguages]
    : translationLanguages;
  if (
    mobile &&
    translationTargetLanguage === "ru" &&
    !visibleTranslationLanguages.some(([value]) => value === "ru")
  ) {
    visibleTranslationLanguages.push(["ru", "俄语（已保存）"]);
  }
  const visibleSecondaryLanguages = mobile
    ? [
        ["", "不显示第二种语言"] as ["", string],
        ...mobileTranslationLanguages,
        ...(translationSecondaryLanguage === "ru"
          ? [["ru", "俄语（已保存）"] as ["ru", string]]
          : []),
      ]
    : translationSecondaryLanguages;
  return { visibleTranslationLanguages, visibleSecondaryLanguages };
}
