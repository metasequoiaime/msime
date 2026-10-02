// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

// The card appears on every mobile host, because importPersonal is wired for all of them.
// Naming one platform in shared copy tells the users of the others something untrue.
async function openDictionary(platform: string) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform } as never,
        dictionary: {
          list: vi.fn().mockResolvedValue({ entries: [], total: 0 }),
          edit: vi.fn(),
          import: vi.fn(),
          export: vi.fn(),
          retry: vi.fn(),
          dismissFailure: vi.fn(),
          importPersonal: vi.fn(),
        },
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  // 个人词库文件是词库页「导入与导出」组里带名字的一块。
  return screen.getByRole("group", { name: "个人词库文件" });
}

test("iOS is told its own keyboard drains the queue", async () => {
  const card = await openDictionary("ios");

  expect(card.textContent).toContain("iOS 键盘同步队列");
  expect(card.textContent).not.toContain("Android");
});

test("Android keeps its own wording", async () => {
  const card = await openDictionary("android");

  expect(card.textContent).toContain("Android 键盘同步队列");
  expect(card.textContent).not.toContain("iOS");
});

// A host that reports no platform still gets a sentence that reads correctly.
test("an unknown host names no platform at all", async () => {
  const card = await openDictionary("harmony");

  expect(card.textContent).toContain("键盘同步队列");
  expect(card.textContent).not.toContain("Android");
  expect(card.textContent).not.toContain("iOS");
});
