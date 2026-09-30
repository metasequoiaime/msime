// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 4,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function communitySkins() {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    finishTrial: vi.fn(),
  };
}

function renderSettings(platform: string, skins: unknown = communitySkins()) {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform } as never,
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        ...(skins ? { communitySkins: skins as never } : {}),
      }}
    />,
  );
}

// Apple's 皮肤 page carries this jump because the community is a separate tab there; the shared mobile navigation has the same shape, and its skins now live on 主题.
test("a mobile host can jump from the theme page to the community", async () => {
  renderSettings("android");
  await settingsFormReady();

  fireEvent.click(screen.getByRole("button", { name: "去社区发现皮肤" }));
  expect(await screen.findByRole("heading", { name: "社区" })).toBeTruthy();
});

test("iOS carries the same entry", async () => {
  renderSettings("ios");
  await settingsFormReady();

  expect(screen.getByRole("button", { name: "去社区发现皮肤" })).toBeTruthy();
});

// A host with no community client has nothing to open.
test("no community client means no entry", async () => {
  renderSettings("android", null);
  await settingsFormReady();

  expect(screen.queryByRole("button", { name: "去社区发现皮肤" })).toBeNull();
});

// The desktop sidebar already lists 社区; a second route to it is noise there.
test("the desktop keeps its sidebar route only", async () => {
  renderSettings("windows");
  await settingsFormReady();

  expect(screen.queryByRole("button", { name: "去社区发现皮肤" })).toBeNull();
});
