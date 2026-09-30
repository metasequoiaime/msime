// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateTranslationSettingsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const props = {
  enabled: true,
  targetLanguage: "en" as const,
  secondaryLanguage: "" as const,
  candidateGlossLanguagesEnabled: true,
  visibleLanguages: [["en", "英语"]] as const,
  visibleSecondaryLanguages: [["", "不显示"]] as const,
  showSecondaryLanguage: false,
  showAccountTranslation: false,
  accountTranslation: false,
  onEnabledChange: vi.fn(),
  onTargetLanguageChange: vi.fn(),
  onSecondaryLanguageChange: vi.fn(),
  onAccountTranslationChange: vi.fn(),
};

test("grouped mode combines options, offline notice, and provider selector", () => {
  const onProviderChange = vi.fn();
  render(
    <CandidateTranslationSettingsSection
      {...props}
      grouped
      onDeviceMissingLanguages={["英语"]}
      showTranslationService
      translationProvider="none"
      onTranslationProviderChange={onProviderChange}
    />,
  );

  expect(screen.getByRole("heading", { name: "候选词翻译" })).toBeTruthy();
  expect(screen.getByRole("status", { name: "系统翻译语言未下载" })).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "候选词翻译服务" }), {
    target: { value: "tencent" },
  });
  expect(onProviderChange).toHaveBeenCalledWith("tencent");
});
