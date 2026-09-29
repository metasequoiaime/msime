// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useSettingsPreviewThemes, type SettingsSkinPreviewThemes } from "@msime/ui";
import type { Dispatch, SetStateAction } from "react";

test("resolves the three settings preview surfaces from the shared theme mode", () => {
  const setSkinPreviewThemes = vi.fn() as Dispatch<SetStateAction<SettingsSkinPreviewThemes>>;
  const { result } = renderHook(() =>
    useSettingsPreviewThemes({
      themeMode: "dark",
      candidateTheme: "follow",
      toolbarTheme: "light",
      screenKeyboardTheme: "follow",
      setSkinPreviewThemes,
    }),
  );

  expect(result.current).toEqual({
    candidatePreviewTheme: "dark",
    toolbarPreviewTheme: "light",
    keyboardPreviewTheme: "dark",
  });
  expect(setSkinPreviewThemes).toHaveBeenCalledWith({});
});
