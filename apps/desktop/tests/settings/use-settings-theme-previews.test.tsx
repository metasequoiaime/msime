// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import {
  useSettingsThemePreviews,
  type SettingsThemePreviewSource,
} from "../../../../packages/ui/src/settings/use-settings-theme-previews";

afterEach(cleanup);

const draft: SettingsThemePreviewSource = {
  theme: "light",
  settings_theme: "light",
  candidate_theme: "dark",
  toolbar_theme: "light",
  screen_keyboard_theme: "follow",
  global_theme: "custom",
  custom_theme: {
    candidate_colors: { number: "#123456", border: "#654321" },
    keyboard: null,
  },
};

test("returns surface previews and host-drawn custom theme", () => {
  const { result } = renderHook(() =>
    useSettingsThemePreviews({
      draft,
      platform: "macos",
      surfaceThemesFollow: false,
      showCandidateNumberColor: true,
      showCandidateBorderColor: false,
    }),
  );

  expect(result.current.themeMode).toBe("light");
  expect(result.current.settingsTheme).toBe("light");
  expect(result.current.globalTheme).toBe("custom");
  expect(result.current.candidatePreviewTheme).toBe("dark");
  expect(result.current.toolbarPreviewTheme).toBe("light");
  expect(result.current.keyboardPreviewTheme).toBe("light");
  expect(result.current.previewCustomTheme?.candidate_colors).toEqual({ number: "#123456" });
  expect(result.current.customKeyboardSelected).toBe(false);
});

test("resets per-skin preview overrides when candidate appearance changes", () => {
  const { result, rerender } = renderHook(
    ({ candidateTheme }: { candidateTheme: "light" | "dark" }) =>
      useSettingsThemePreviews({
        draft: { ...draft, candidate_theme: candidateTheme },
        surfaceThemesFollow: false,
        showCandidateNumberColor: true,
        showCandidateBorderColor: true,
      }),
    { initialProps: { candidateTheme: "light" as "light" | "dark" } },
  );

  act(() => result.current.setSkinPreviewThemes({ custom: "dark" }));
  expect(result.current.skinPreviewThemes).toEqual({ custom: "dark" });

  rerender({ candidateTheme: "dark" });

  expect(result.current.skinPreviewThemes).toEqual({});
});

test("marks a custom keyboard theme only when a keyboard design exists", () => {
  const { result } = renderHook(() =>
    useSettingsThemePreviews({
      draft: {
        global_theme: "custom",
        custom_theme: { keyboard: defaultTouchKeyboardSkinDesign },
      },
      surfaceThemesFollow: false,
      showCandidateNumberColor: true,
      showCandidateBorderColor: true,
    }),
  );

  expect(result.current.customKeyboardSelected).toBe(true);
});
