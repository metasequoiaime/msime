import { expect, test } from "vitest";
import {
  mobileTranslationLanguages,
  translationLanguageChoices,
  translationLanguages,
  translationSecondaryLanguages,
} from "../../../../packages/ui/src/settings/translation-language-helpers";

test("桌面保留完整语言目录和原数组引用", () => {
  const choices = translationLanguageChoices(false, "ru", "ru");
  expect(choices.visibleTranslationLanguages).toBe(translationLanguages);
  expect(choices.visibleSecondaryLanguages).toBe(translationSecondaryLanguages);
  expect(choices.visibleTranslationLanguages.map(([code]) => code)).toEqual([
    "en",
    "fr",
    "ja",
    "es",
    "ru",
    "de",
    "ko",
  ]);
});

test("移动端普通选项不提供俄语，第二语言仍以关闭选项开头", () => {
  const choices = translationLanguageChoices(true, "en", "");
  expect(choices.visibleTranslationLanguages.map(([code]) => code)).toEqual([
    "en",
    "fr",
    "ja",
    "es",
    "de",
    "ko",
  ]);
  expect(choices.visibleSecondaryLanguages).toEqual([
    ["", "不显示第二种语言"],
    ...choices.visibleTranslationLanguages,
  ]);
});

test("仅主语言已保存俄语时只为主语言补回旧选项", () => {
  const choices = translationLanguageChoices(true, "ru", "en");
  expect(choices.visibleTranslationLanguages.at(-1)).toEqual(["ru", "俄语（已保存）"]);
  expect(choices.visibleSecondaryLanguages.some(([code]) => code === "ru")).toBe(false);
});

test("仅第二语言已保存俄语时只为第二语言补回旧选项", () => {
  const choices = translationLanguageChoices(true, "en", "ru");
  expect(choices.visibleSecondaryLanguages.at(-1)).toEqual(["ru", "俄语（已保存）"]);
  expect(choices.visibleTranslationLanguages.some(([code]) => code === "ru")).toBe(false);
});

test("两种语言均保存俄语时各自只增加一个旧选项", () => {
  const choices = translationLanguageChoices(true, "ru", "ru");
  expect(choices.visibleTranslationLanguages.filter(([code]) => code === "ru")).toEqual([
    ["ru", "俄语（已保存）"],
  ]);
  expect(choices.visibleSecondaryLanguages.filter(([code]) => code === "ru")).toEqual([
    ["ru", "俄语（已保存）"],
  ]);
});

test("保留旧语言不会改写共享目录或泄漏到后续页面", () => {
  const previous = structuredClone({
    translationLanguages,
    translationSecondaryLanguages,
    mobileTranslationLanguages,
  });
  translationLanguageChoices(true, "ru", "ru");
  const next = translationLanguageChoices(true, "ja", "");
  expect(next.visibleTranslationLanguages.some(([code]) => code === "ru")).toBe(false);
  expect(next.visibleSecondaryLanguages.some(([code]) => code === "ru")).toBe(false);
  expect({
    translationLanguages,
    translationSecondaryLanguages,
    mobileTranslationLanguages,
  }).toEqual(previous);
});

test("移动端每次创建独立目录数组并保留原选项元组引用", () => {
  const first = translationLanguageChoices(true, "en", "");
  const second = translationLanguageChoices(true, "en", "");
  expect(first.visibleTranslationLanguages).not.toBe(mobileTranslationLanguages);
  expect(first.visibleTranslationLanguages).not.toBe(second.visibleTranslationLanguages);
  expect(first.visibleSecondaryLanguages).not.toBe(second.visibleSecondaryLanguages);
  expect(first.visibleTranslationLanguages[0]).toBe(mobileTranslationLanguages[0]);
  expect(first.visibleSecondaryLanguages[1]).toBe(mobileTranslationLanguages[0]);
});
