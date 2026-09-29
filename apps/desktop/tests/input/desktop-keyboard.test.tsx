// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { themeEntry, type Snapshot } from "@msime/ui";
import { DesktopKeyboard } from "../../src/input/desktop-keyboard";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const snapshot = (
  revision: number,
  theme: "light" | "dark" | "system",
  surface: "follow" | "light" | "dark" = "follow",
): Snapshot => ({
  format_version: 1,
  revision,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    theme,
    screen_keyboard_theme: surface,
  },
});
const panel = { close: async () => {} };
const theme = () =>
  screen.getByRole("main", { name: "屏幕键盘" }).getAttribute("data-keyboard-theme");

test("keyboard subscribes before load and rejects stale snapshots without remounting", async () => {
  let emit!: (value: Snapshot) => void;
  let resolve!: (value: Snapshot) => void;
  const stop = vi.fn();
  const order: string[] = [];
  const rememberInputTarget = vi.fn().mockResolvedValue(undefined);
  const preferences = {
    onPreferencesChanged: async (listener: typeof emit) => {
      order.push("subscribe");
      emit = listener;
      return stop;
    },
    load: () => {
      order.push("load");
      return new Promise<Snapshot>((done) => {
        resolve = done;
      });
    },
  };
  const view = render(
    <DesktopKeyboard client={{ ...panel, rememberInputTarget }} preferences={preferences} />,
  );
  await waitFor(() => expect(order).toEqual(["subscribe", "load"]));
  act(() => emit(snapshot(3, "dark", "light")));
  expect(theme()).toBe("light");
  await act(async () => resolve(snapshot(2, "dark")));
  expect(theme()).toBe("light");
  act(() => emit(snapshot(1, "dark")));
  expect(theme()).toBe("light");
  act(() => emit(snapshot(4, "light", "dark")));
  expect(theme()).toBe("dark");
  expect(rememberInputTarget).toHaveBeenCalledTimes(1);
  view.unmount();
  expect(stop).toHaveBeenCalledTimes(1);
});

test("late subscription is cleaned up and never loads after unmount", async () => {
  let finish!: (stop: () => void) => void;
  const load = vi.fn();
  const view = render(
    <DesktopKeyboard
      client={panel}
      preferences={{
        load,
        onPreferencesChanged: () =>
          new Promise((done) => {
            finish = done;
          }),
      }}
    />,
  );
  view.unmount();
  const stop = vi.fn();
  await act(async () => finish(stop));
  expect(stop).toHaveBeenCalledOnce();
  expect(load).not.toHaveBeenCalled();
});

test("keyboard follows system changes and loads despite subscription failure", async () => {
  let change!: () => void;
  const media = {
    matches: true,
    addEventListener: vi.fn((_name, listener) => {
      change = listener;
    }),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("matchMedia", () => media);
  const view = render(
    <DesktopKeyboard
      client={panel}
      preferences={{
        load: async () => snapshot(1, "system"),
        onPreferencesChanged: async () => {
          throw new Error("synthetic");
        },
      }}
    />,
  );
  await waitFor(() => expect(theme()).toBe("light"));
  act(() => {
    media.matches = false;
    change();
  });
  expect(theme()).toBe("dark");
  view.unmount();
  expect(media.removeEventListener).toHaveBeenCalledWith("change", change);
});

test("failed initial load retains default dark theme", async () => {
  const load = vi.fn().mockRejectedValue(new Error("synthetic"));
  render(<DesktopKeyboard client={panel} preferences={{ load }} />);
  await waitFor(() => expect(load).toHaveBeenCalledOnce());
  expect(theme()).toBe("dark");
});
test("keyboard applies selected built-in and custom skin preferences", async () => {
  let emit!: (value: Snapshot) => void;
  const custom = {
    background: 0x102438,
    keyBackground: 0x17354f,
    keyForeground: 0xffffff,
    accent: 0xa2d8fa,
    actionBackground: 0x285d84,
    cornerRadius: 2,
    borderWidth: 1,
    shadow: 0,
    pattern: 2 as const,
    patternOpacity: 0.1,
    keyShape: "pebble" as const,
    keyMaterial: "raised" as const,
    keyOpacity: 0.45,
    gradientEnd: 0x203040,
    gradientHorizontal: true,
    photo:
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
    photoShade: 0.2,
    photoPosition: 0.75,
    monospaced: true,
  };
  const first = snapshot(1, "dark");
  first.preferences.global_theme = "night";
  const preferences = {
    load: async () => first,
    onPreferencesChanged: async (listener: (value: Snapshot) => void) => {
      emit = listener;
      return () => {};
    },
  };
  render(<DesktopKeyboard client={panel} preferences={preferences} />);
  await waitFor(() =>
    expect(screen.getByRole("main", { name: "屏幕键盘" }).getAttribute("data-keyboard-skin")).toBe(
      "night",
    ),
  );
  const main = screen.getByRole("main", { name: "屏幕键盘" });
  expect(main.style.getPropertyValue("--kb-background")).toBe(
    themeEntry("night").keyboard!.background,
  );
  expect(main.style.getPropertyValue("--kb-font-family")).toBe("inherit");
  act(() =>
    emit({
      ...first,
      revision: 2,
      preferences: {
        ...first.preferences,
        global_theme: "custom",
        custom_theme: { keyboard: custom },
      },
    }),
  );
  await waitFor(() => expect(main.getAttribute("data-keyboard-skin")).toBe("custom"));
  expect(main.style.getPropertyValue("--kb-background")).toBe("#102438");
  expect(main.style.getPropertyValue("--kb-font-family")).toContain("ui-monospace");
  expect(main.style.getPropertyValue("--kb-key-radius")).toContain("42%");
  expect(main.style.getPropertyValue("--kb-border-width")).toBe("1px");
  expect(main.style.getPropertyValue("--kb-action")).toBe("#285d84");
  expect(main.getAttribute("data-keyboard-material")).toBe("raised");
  expect(main.style.getPropertyValue("--kb-key-radius")).toContain("42%");
  expect(main.style.getPropertyValue("--kb-key-fill")).toContain("0.45");
  // The material is a property of the key, not a class name: it decides the fill, the lift and the
  // shadow together, so the key carries it as data rather than as styling a test has to decode.
  expect(main.querySelector(".keyboard-key")?.getAttribute("data-key-material")).toBe("raised");
  expect(main.style.getPropertyValue("background-image")).toContain("data:image/jpeg;base64,");
  expect(main.style.getPropertyValue("background-image")).toContain("linear-gradient");
  expect(main.style.getPropertyValue("background-position")).toContain("75% 75%");
  act(() =>
    emit({
      ...first,
      revision: 3,
      preferences: {
        ...first.preferences,
        global_theme: "custom",
        custom_theme: { base: "paper", keyboard: null },
      },
    }),
  );
  // A custom theme without a keyboard design draws its base's keyboard.
  await waitFor(() => expect(main.getAttribute("data-keyboard-skin")).toBe("paper"));
  expect(main.style.getPropertyValue("--kb-background")).toBe(
    themeEntry("paper").keyboard!.background,
  );
});

test("macOS standalone keyboard does not expose an unauthenticated voice panel", async () => {
  const value = snapshot(1, "dark");
  value.preferences.touch_voice_shortcut = true;
  render(
    <DesktopKeyboard
      client={{ ...panel, openVoice: vi.fn() }}
      preferences={{
        host: { platform: "macos" } as never,
        load: async () => value,
      }}
    />,
  );
  await waitFor(() => expect(screen.getByRole("main", { name: "屏幕键盘" })).toBeTruthy());
  expect(screen.queryByRole("button", { name: "打开语音输入" })).toBeNull();
});
