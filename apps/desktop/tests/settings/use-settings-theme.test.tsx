// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useSettingsTheme } from "../../../../packages/ui/src/settings/use-settings-theme";
import type {
  SurfaceTheme,
  ThemeMode,
} from "../../../../packages/ui/src/settings/theme-preferences";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.settingsAppearance;
});

test("宿主收到原始外观意图，跟随系统时继续响应系统变化", () => {
  let light = true;
  const changes = new EventTarget();
  vi.stubGlobal("matchMedia", () => ({
    get matches() {
      return light;
    },
    addEventListener: changes.addEventListener.bind(changes),
    removeEventListener: changes.removeEventListener.bind(changes),
  }));
  const { result, rerender } = renderHook(
    ({ mode, surface }: { mode: ThemeMode; surface: SurfaceTheme }) =>
      useSettingsTheme(mode, surface),
    { initialProps: { mode: "dark" as ThemeMode, surface: "follow" as SurfaceTheme } },
  );
  expect(result.current).toBe(true);
  expect(document.documentElement.dataset.settingsAppearance).toBe("dark");
  rerender({ mode: "dark", surface: "light" });
  expect(document.documentElement.dataset.settingsAppearance).toBe("light");
  expect(result.current).toBe(false);
  rerender({ mode: "system", surface: "follow" });
  expect(document.documentElement.dataset.settingsAppearance).toBe("system");
  expect(document.documentElement.dataset.theme).toBe("light");
  act(() => {
    light = false;
    changes.dispatchEvent(new Event("change"));
  });
  expect(result.current).toBe(true);
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(document.documentElement.dataset.settingsAppearance).toBe("system");
});
