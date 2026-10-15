// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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

// 保存状态原来只写在表单最后一行的小字里。偏好文档读不进时（例如输入法更新后写了设置应用不认识的键）每次保存都失败，用户在拨开关的地方什么也看不到，只觉得改了不生效。失败要在页面顶部说出来，并能就地重试。
test("a failed save is reported at the top of the page with a retry", async () => {
  const save = vi.fn().mockRejectedValue(new Error("合成的保存失败"));
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save, host: testHost({ platform: "macos" }) }}
    />,
  );
  const form = await settingsFormReady();
  // 哪一页、哪个开关都一样：任何一次保存失败都要在顶部看得见。
  const toggle = screen.getAllByRole("switch", { hidden: true })[0];
  fireEvent.click(toggle);
  saveSettingsNow();

  const banner = await screen.findByText(/设置没有保存/);
  expect(banner.textContent).toContain("合成的保存失败");
  // 在表单内容之前，不必滚到页面底部才看得见。
  expect(banner.compareDocumentPosition(toggle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(form.contains(banner)).toBe(false);

  const calls = save.mock.calls.length;
  fireEvent.click(screen.getAllByRole("button", { name: "重试" })[0]);
  await waitFor(() => expect(save.mock.calls.length).toBeGreaterThan(calls));
});

test("nothing is reported at the top while saves succeed", async () => {
  const save = vi.fn(async (revision: number, preferences: Snapshot["preferences"]) => ({
    format_version: 1,
    revision: revision + 1,
    preferences,
  }));
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save, host: testHost({ platform: "macos" }) }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getAllByRole("switch", { hidden: true })[0]);
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(screen.queryByText(/设置没有保存/)).toBeNull();
});
