// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { DesktopPanelTheme } from "../../src/input/desktop-panel-theme";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: vi.fn(() => false), invoke: vi.fn() }));

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

function snapshot(revision = 1): Snapshot {
  return {
    format_version: 1,
    revision,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      theme: "dark",
      handwriting_theme: "light",
      voice_theme: "dark",
      emoji_theme: "light",
    },
  };
}

const showTheme = (theme: "dark" | "light") => <output>{theme}</output>;
const currentTheme = () => screen.getByRole("status").textContent;

test.each([
  ["handwriting", "light"],
  ["voice", "dark"],
  ["emoji", "light"],
] as const)("%s uses its own theme field", async (surface, expected) => {
  const value = snapshot();
  const opposite = expected === "light" ? "dark" : "light";
  value.preferences = {
    ...value.preferences,
    theme: opposite,
    handwriting_theme: opposite,
    voice_theme: opposite,
    emoji_theme: opposite,
    [`${surface}_theme`]: expected,
  };
  render(
    <DesktopPanelTheme preferences={{ load: async () => value }} surface={surface}>
      {showTheme}
    </DesktopPanelTheme>,
  );
  await act(async () => {});
  await waitFor(() => expect(currentTheme()).toBe(expected));
});

test("preference updates follow host policy and dispose the subscription", async () => {
  let emit!: (value: Snapshot) => void;
  const stop = vi.fn();
  const value = snapshot();
  const view = render(
    <DesktopPanelTheme
      preferences={{
        host: testHost({ platform: "macos", surface_themes_follow: true }),
        load: async () => value,
        onPreferencesChanged: async (listener) => {
          emit = listener;
          return stop;
        },
      }}
      surface="handwriting"
    >
      {showTheme}
    </DesktopPanelTheme>,
  );
  await act(async () => {});
  expect(currentTheme()).toBe("dark");
  act(() => emit({ ...value, revision: 2, preferences: { ...value.preferences, theme: "light" } }));
  expect(currentTheme()).toBe("light");
  act(() => emit(value));
  expect(currentTheme()).toBe("light");
  view.unmount();
  expect(stop).toHaveBeenCalledOnce();
});

test("pending host capabilities suppress the saved surface override", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  let resolveHost!: (value: unknown) => void;
  vi.mocked(invoke).mockReturnValue(new Promise((resolve) => (resolveHost = resolve)));
  const value = snapshot();
  const load = vi.fn(async () => value);
  render(
    <DesktopPanelTheme preferences={{ load }} surface="emoji">
      {showTheme}
    </DesktopPanelTheme>,
  );
  await act(async () => {});
  expect(load).toHaveBeenCalledOnce();
  expect(currentTheme()).toBe("dark");
  await act(async () => resolveHost({ platform: "windows", surface_themes_follow: false }));
  expect(currentTheme()).toBe("light");
});

test("system colour changes reach the child and release their listener", async () => {
  let change!: () => void;
  const media = {
    matches: false,
    addEventListener: vi.fn((_event, listener) => (change = listener)),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("matchMedia", () => media);
  const value = snapshot();
  value.preferences.theme = "system";
  value.preferences.voice_theme = "follow";
  const view = render(
    <DesktopPanelTheme preferences={{ load: async () => value }} surface="voice">
      {showTheme}
    </DesktopPanelTheme>,
  );
  await act(async () => {});
  act(() => {
    media.matches = true;
    change();
  });
  expect(currentTheme()).toBe("light");
  view.unmount();
  expect(media.removeEventListener).toHaveBeenCalledWith("change", change);
});
