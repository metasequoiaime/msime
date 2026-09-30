// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SettingsPage, type HostCapabilities, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    global_theme: "shuishan",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

async function shellFor(host: Partial<HostCapabilities> | undefined) {
  const { container } = render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: host as HostCapabilities | undefined,
      }}
    />,
  );
  await settingsFormReady();
  return container.querySelector("[data-settings-shell]")!;
}

function viewportWide(wide: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query === "(min-width: 601px)" ? wide : false,
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

test("the settings root names the desktop host's platform", async () => {
  for (const [platform, expected] of [
    ["windows", "win"],
    ["macos", "mac"],
    ["linux", "linux"],
  ] as const) {
    const shell = await shellFor({ platform });
    expect(shell.getAttribute("data-platform"), platform).toBe(expected);
    cleanup();
  }
});

test("HarmonyOS takes the 2-in-1 look only where the host turns the phone navigation off", async () => {
  expect(
    (await shellFor({ platform: "harmony", mobile_settings: false })).getAttribute("data-platform"),
  ).toBe("hm2");
  cleanup();
  expect(
    (await shellFor({ platform: "harmony", mobile_settings: true })).getAttribute("data-platform"),
  ).toBe("harmony");
});

test("an iOS host is an iPad above the phone breakpoint", async () => {
  viewportWide(false);
  expect((await shellFor({ platform: "ios" })).getAttribute("data-platform")).toBe("ios");
  cleanup();
  viewportWide(true);
  expect((await shellFor({ platform: "ios" })).getAttribute("data-platform")).toBe("ipad");
});
