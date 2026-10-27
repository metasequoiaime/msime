// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    global_theme: "system",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

// 「我的 › 隐私」打开应用里的页面：先用几句话说数据怎么处理，再是匿名使用统计的开关，完整隐私政策的网页在最后一行，点了才去浏览器。
test("the phone privacy page explains the data in the app and links the policy last", async () => {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...initial,
    revision: 2,
    preferences: next,
  }));
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save,
        openExternalUrl,
        host: testHost({ platform: "harmony", mobile_settings: true, panel_windows: false }),
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "隐私" });
  const titles = [...page.querySelectorAll("[data-group-title]")].map((node) => node.textContent);
  expect(titles).toEqual(["你的数据", "匿名使用统计"]);
  expect(within(page).getByText("打的字留在手机上")).toBeTruthy();
  expect(openExternalUrl).not.toHaveBeenCalled();

  fireEvent.click(within(page).getByRole("switch", { name: "匿名使用统计" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls.at(-1)?.[1].usage_reporting).toBe(false);

  fireEvent.click(within(page).getByRole("button", { name: "完整隐私政策" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/privacy/");
});

// 桌面的隐私设置在「关于」里，没有这一页。
test("desktop hosts have no separate privacy page", async () => {
  render(
    <SettingsPage
      initialPage="privacy"
      client={{ load: async () => initial, save: vi.fn(), host: testHost({ platform: "windows" }) }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("group", { name: "隐私" })).toBeNull();
});

// HarmonyOS 首次启动时由原生弹窗征得隐私政策授权；没同意时「隐私」页说明联网功能关着，并能重新打开那个弹窗。
test("the privacy page shows the host's consent and reopens its dialog", async () => {
  const review = vi.fn().mockResolvedValue(undefined);
  const host = testHost({ platform: "harmony", mobile_settings: true, panel_windows: false });
  const { rerender } = render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host,
        privacyConsent: { granted: false, review },
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "隐私" });
  expect(within(page).getByText("隐私政策授权")).toBeTruthy();
  expect(
    within(page).getByText(/未同意：登录、云同步、社区、检查更新等联网功能已关闭/),
  ).toBeTruthy();
  fireEvent.click(within(page).getByRole("button", { name: "查看并同意隐私政策" }));
  await waitFor(() => expect(review).toHaveBeenCalledTimes(1));

  rerender(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host,
        privacyConsent: { granted: true, review },
      }}
    />,
  );
  await settingsFormReady();
  const granted = screen.getByRole("group", { name: "隐私" });
  expect(within(granted).getByText("已同意，联网功能已开启。")).toBeTruthy();
  expect(within(granted).getByRole("button", { name: "查看或撤回隐私政策授权" })).toBeTruthy();
});

// 其他宿主不提供授权，这一行不出现。
test("hosts without a consent dialog show no consent row", async () => {
  render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "android", mobile_settings: true, panel_windows: false }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByText("隐私政策授权")).toBeNull();
});
