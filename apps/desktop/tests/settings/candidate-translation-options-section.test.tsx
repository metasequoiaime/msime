// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateTranslationOptionsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const languages = [
  ["en", "英语"],
  ["ja", "日语"],
] as const;

const secondaryLanguages = [
  ["", "不显示"],
  ["en", "英语"],
] as const;

test("updates translation enablement and language choices", () => {
  const onEnabledChange = vi.fn();
  const onTargetLanguageChange = vi.fn();
  const onSecondaryLanguageChange = vi.fn();
  render(
    <CandidateTranslationOptionsSection
      enabled
      targetLanguage="en"
      secondaryLanguage={null}
      candidateGlossLanguagesEnabled
      visibleLanguages={languages}
      visibleSecondaryLanguages={secondaryLanguages}
      showSecondaryLanguage
      showAccountTranslation={false}
      accountTranslation={false}
      onEnabledChange={onEnabledChange}
      onTargetLanguageChange={onTargetLanguageChange}
      onSecondaryLanguageChange={onSecondaryLanguageChange}
      onAccountTranslationChange={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "候选词翻译" }));
  fireEvent.change(screen.getByRole("combobox", { name: "候选词翻译目标语言" }), {
    target: { value: "ja" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "候选词翻译第二种语言" }), {
    target: { value: "en" },
  });

  expect(onEnabledChange).toHaveBeenCalledWith(false);
  expect(onTargetLanguageChange).toHaveBeenCalledWith("ja");
  expect(onSecondaryLanguageChange).toHaveBeenCalledWith("en");
});

test("shows the Android account translation switch only when requested", () => {
  const onAccountTranslationChange = vi.fn();
  const { rerender } = render(
    <CandidateTranslationOptionsSection
      enabled
      targetLanguage="en"
      secondaryLanguage={null}
      candidateGlossLanguagesEnabled
      visibleLanguages={languages}
      visibleSecondaryLanguages={secondaryLanguages}
      showSecondaryLanguage={false}
      showAccountTranslation
      accountTranslation={false}
      onEnabledChange={vi.fn()}
      onTargetLanguageChange={vi.fn()}
      onSecondaryLanguageChange={vi.fn()}
      onAccountTranslationChange={onAccountTranslationChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "使用水杉账号翻译候选词" }));
  expect(onAccountTranslationChange).toHaveBeenCalledWith(true);

  rerender(
    <CandidateTranslationOptionsSection
      enabled
      targetLanguage="en"
      secondaryLanguage={null}
      candidateGlossLanguagesEnabled
      visibleLanguages={languages}
      visibleSecondaryLanguages={secondaryLanguages}
      showSecondaryLanguage={false}
      showAccountTranslation={false}
      accountTranslation={false}
      onEnabledChange={vi.fn()}
      onTargetLanguageChange={vi.fn()}
      onSecondaryLanguageChange={vi.fn()}
      onAccountTranslationChange={onAccountTranslationChange}
    />,
  );
  expect(screen.queryByRole("checkbox", { name: "使用水杉账号翻译候选词" })).toBeNull();
});
