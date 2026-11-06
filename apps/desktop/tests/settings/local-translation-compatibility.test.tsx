// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useTranslationSettings } from "../../../../packages/ui/src/settings/use-translation-settings";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";

test("显式切换云服务关闭本地翻译并保留目标语言", () => {
  const onChange = vi.fn();
  const { result } = renderHook(() =>
    useTranslationSettings({
      preferences: {
        candidate_translations: true,
        local_translation: { enabled: true, target_language: "bo" },
      } as Preferences,
      macos: true,
      linux: false,
      mobile: false,
      candidateEnglishGlossAvailable: true,
      onDeviceDownloadable: ["en"],
      onChange,
    }),
  );
  expect(result.current.onDeviceMissingLanguages).toEqual([]);
  act(() => result.current.setTranslationProvider("account"));
  expect(onChange).toHaveBeenCalledWith(
    expect.objectContaining({
      translation_account: true,
      local_translation: { enabled: false, target_language: "bo" },
    }),
  );
});

test("旧偏好不增加本地翻译字段", () => {
  const onChange = vi.fn();
  const { result } = renderHook(() =>
    useTranslationSettings({
      preferences: {} as Preferences,
      macos: false,
      linux: true,
      mobile: false,
      candidateEnglishGlossAvailable: true,
      onDeviceDownloadable: [],
      onChange,
    }),
  );
  act(() => result.current.setTranslationProvider("none"));
  expect(onChange.mock.calls[0][0]).not.toHaveProperty("local_translation");
});
