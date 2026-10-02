// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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

  fireEvent.click(screen.getByRole("button", { name: "去社区找皮肤" }));
  expect(await screen.findByRole("heading", { name: "社区" })).toBeTruthy();
});

test("iOS carries the same entry", async () => {
  renderSettings("ios");
  await settingsFormReady();

  expect(screen.getByRole("button", { name: "去社区找皮肤" })).toBeTruthy();
});

// A host with no community client has nothing to open.
test("no community client means no entry", async () => {
  renderSettings("android", null);
  await settingsFormReady();

  expect(screen.queryByRole("button", { name: "去社区找皮肤" })).toBeNull();
});

// The desktop has no 社区 page: its keyboard-skin row is a touch entry, and candidate-window skins are a tab of 主题 itself.
test("the desktop has no mobile community row", async () => {
  renderSettings("windows");
  await settingsFormReady();

  expect(screen.queryByRole("button", { name: "去社区找皮肤" })).toBeNull();
});

function candidateSkins() {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    preview: vi.fn(),
    install: vi.fn(),
    packPreview: vi.fn(),
    addPreview: vi.fn(),
    addLicense: vi.fn(),
    publish: vi.fn(),
    setVisibility: vi.fn(),
    setCategory: vi.fn(),
    sync: vi.fn().mockResolvedValue({ uploaded: [], downloaded: [], deleted_local: [] }),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
}

// Desktop browses candidate-window skins on 主题 behind 我的皮肤 / 社区皮肤, the way 插件 switches between 我的插件 and 社区插件.
test("the desktop theme page switches to the community candidate-window skins", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        loadDefaultPreferences: vi.fn().mockResolvedValue(initial),
        host: { platform: "windows" } as never,
        communityCandidateSkins: candidateSkins() as never,
      }}
    />,
  );
  await settingsFormReady();

  expect(screen.queryByRole("button", { name: "社区" })).toBeNull();
  const tabs = screen.getByRole("tablist", { name: "皮肤来源" });
  expect(screen.getByRole("group", { name: "主题" }).hidden).toBe(false);
  expect(screen.getByRole("button", { name: "恢复默认设置" })).toBeTruthy();

  fireEvent.click(within(tabs).getByRole("tab", { name: "社区皮肤" }));
  expect(await screen.findByRole("heading", { name: "社区皮肤" })).toBeTruthy();
  expect(screen.queryByRole("group", { name: "主题" })).toBeNull();
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();

  fireEvent.click(within(tabs).getByRole("tab", { name: "我的皮肤" }));
  expect(screen.getByRole("group", { name: "主题" })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: "社区皮肤" })).toBeNull();
});
