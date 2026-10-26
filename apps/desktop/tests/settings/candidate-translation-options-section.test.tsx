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

  fireEvent.click(screen.getByRole("switch", { name: "候选词翻译" }));
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

  fireEvent.click(screen.getByRole("switch", { name: "使用水杉账号翻译候选词" }));
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
  expect(screen.queryByRole("switch", { name: "使用水杉账号翻译候选词" })).toBeNull();
});

test("the default layout folds only the second language and draws beforeMore after the target", () => {
  render(
    <CandidateTranslationOptionsSection
      enabled
      targetLanguage="en"
      secondaryLanguage={null}
      candidateGlossLanguagesEnabled
      visibleLanguages={languages}
      visibleSecondaryLanguages={secondaryLanguages}
      showSecondaryLanguage
      showAccountTranslation
      accountTranslation={false}
      onEnabledChange={vi.fn()}
      onTargetLanguageChange={vi.fn()}
      onSecondaryLanguageChange={vi.fn()}
      onAccountTranslationChange={vi.fn()}
      beforeMore={<input type="checkbox" aria-label="sub-row" />}
    />,
  );
  const secondary = screen.getByRole("combobox", { name: "候选词翻译第二种语言" });
  expect(secondary.closest("details")?.textContent).toContain("更多选项");
  // Android 上账号开关是唯一的在线翻译来源，不收起来。
  expect(
    screen.getByRole("switch", { name: "使用水杉账号翻译候选词" }).closest("details"),
  ).toBeNull();
  const controls = Array.from(document.querySelectorAll("input, select"));
  expect(controls.indexOf(screen.getByRole("checkbox", { name: "sub-row" }))).toBe(
    controls.indexOf(screen.getByRole("combobox", { name: "候选词翻译目标语言" })) + 1,
  );
});

test("offers the translation scope and disables it while translation is off", () => {
  const onScopeChange = vi.fn();
  const props = {
    targetLanguage: "en",
    secondaryLanguage: null,
    candidateGlossLanguagesEnabled: true,
    visibleLanguages: languages,
    visibleSecondaryLanguages: secondaryLanguages,
    showSecondaryLanguage: false,
    showAccountTranslation: false,
    accountTranslation: false,
    onEnabledChange: vi.fn(),
    onTargetLanguageChange: vi.fn(),
    onSecondaryLanguageChange: vi.fn(),
    onAccountTranslationChange: vi.fn(),
    onScopeChange,
  } as const;
  const { rerender } = render(<CandidateTranslationOptionsSection enabled {...props} />);
  const scope = screen.getByRole("combobox", { name: "候选词翻译范围" }) as HTMLSelectElement;
  // 不传 scope 时按「仅当前高亮」。
  expect(scope.value).toBe("highlighted");
  expect(Array.from(scope.options).map((option) => [option.value, option.textContent])).toEqual([
    ["highlighted", "仅当前高亮"],
    ["all", "全部"],
  ]);
  expect(scope.disabled).toBe(false);
  fireEvent.change(scope, { target: { value: "all" } });
  expect(onScopeChange).toHaveBeenCalledWith("all");

  rerender(<CandidateTranslationOptionsSection enabled={false} scope="all" {...props} />);
  const disabled = screen.getByRole("combobox", { name: "候选词翻译范围" }) as HTMLSelectElement;
  expect(disabled.value).toBe("all");
  expect(disabled.disabled).toBe(true);

  // 手机布局同样有这一行，紧跟在开关后面。
  rerender(<CandidateTranslationOptionsSection enabled layout="phone" {...props} />);
  const rows = screen.getAllByRole("combobox").map((row) => row.getAttribute("aria-label"));
  expect(rows[0]).toBe("候选词翻译范围");
});

test("leaves the scope row out when the host does not bind it", () => {
  render(
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
      onAccountTranslationChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("combobox", { name: "候选词翻译范围" })).toBeNull();
});
