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

// 拒绝授权后鸿蒙手机没有账号客户端；授权弹窗和键盘都让用户去「我的 › 隐私」重新同意，所以「我的」和那一行都还要在。
test("a HarmonyOS phone that refused consent still reaches 我的 › 隐私 to agree later", async () => {
  const review = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="account"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: true, panel_windows: false }),
        privacyConsent: { granted: false, review },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: /^隐私/ }));
  const page = await screen.findByRole("group", { name: "隐私" });
  fireEvent.click(within(page).getByRole("button", { name: "查看并同意隐私政策" }));
  await waitFor(() => expect(review).toHaveBeenCalledTimes(1));
});

// 2in1 的授权在「关于 › 法律信息」里，没有账号客户端时不因此多出账号页。
test("a HarmonyOS 2in1 without an account client gets no account page from consent alone", async () => {
  render(
    <SettingsPage
      initialPage="account"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: false }),
        privacyConsent: { granted: false, review: vi.fn() },
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("heading", { name: /^(我的|账号与同步)$/ })).toBeNull();
  expect(screen.queryByRole("button", { name: /^隐私/ })).toBeNull();
});

// 「防止截屏」（#10194）：宿主提供时「隐私」页多一组「截屏与录屏」；切换交给宿主，成功后开关跟着宿主的回答走。
test("a HarmonyOS phone switches screen capture protection through the host", async () => {
  const load = vi.fn().mockResolvedValue(false);
  const save = vi.fn().mockResolvedValue(true);
  render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: true, panel_windows: false }),
        screenCaptureProtection: { load, save },
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "隐私" });
  const titles = [...page.querySelectorAll("[data-group-title]")].map((node) => node.textContent);
  expect(titles).toEqual(["你的数据", "匿名使用统计", "截屏与录屏"]);
  const toggle = within(page).getByRole("switch", { name: /防止截屏/ }) as HTMLInputElement;
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  await waitFor(() => expect(save).toHaveBeenCalledWith(true));
  await waitFor(() => expect(toggle.checked).toBe(true));
});

// 宿主拒绝（没有系统认证、认证没通过）时开关保持原值，并按原因提示；没设锁屏密码时提示先设置。
test("a refused switch keeps its value and says why", async () => {
  const save = vi
    .fn()
    .mockRejectedValueOnce({ code: "screen_capture_authentication_unavailable" })
    .mockRejectedValueOnce({ code: "screen_capture_authentication_failed" });
  render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: true, panel_windows: false }),
        screenCaptureProtection: { load: vi.fn().mockResolvedValue(false), save },
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "隐私" });
  const toggle = within(page).getByRole("switch", { name: /防止截屏/ }) as HTMLInputElement;
  await waitFor(() => expect(toggle.disabled).toBe(false));
  fireEvent.click(toggle);
  expect(
    await screen.findByText("这台设备没有可用的系统认证，设置锁屏密码后才能打开防止截屏"),
  ).toBeTruthy();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  expect(await screen.findByText("未通过系统认证，防止截屏保持关闭")).toBeTruthy();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(false);
});

// 2in1 没有「我的 › 隐私」，开关在「关于 › 许可与隐私」；那一页常驻，所以搜「录屏」能找到它。没有提供这项能力的宿主（Android 用原生页）不出现。
test("a HarmonyOS 2in1 finds screen capture protection under 关于 by searching 录屏", async () => {
  Element.prototype.scrollIntoView = vi.fn();
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: false }),
        screenCaptureProtection: { load: vi.fn().mockResolvedValue(true), save: vi.fn() },
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.change(screen.getByRole("searchbox", { name: "搜索设置" }), {
    target: { value: "录屏" },
  });
  const result = within(screen.getByRole("listbox", { name: "设置搜索结果" })).getByRole("option", {
    name: /截屏与录屏/,
  });
  expect(result.textContent).toContain("关于");
  fireEvent.change(screen.getByRole("searchbox", { name: "搜索设置" }), {
    target: { value: "防止截屏" },
  });
  expect(
    within(screen.getByRole("listbox", { name: "设置搜索结果" })).getByRole("option", {
      name: /防止截屏/,
    }),
  ).toBeTruthy();
});

test("hosts without screen capture protection show no such switch", async () => {
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
  expect(screen.queryByText("防止截屏")).toBeNull();
  expect(screen.queryByText("截屏与录屏")).toBeNull();
});

// 鸿蒙手机上「我的 › 隐私」和常驻的「关于 › 许可与隐私」同时挂着一个开关：在任一处切换成功后，另一处也跟着变，不能停在旧值上。
test("both copies of the switch on a HarmonyOS phone follow a toggle in either one", async () => {
  const save = vi.fn().mockResolvedValueOnce(true).mockResolvedValueOnce(false);
  render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: true, panel_windows: false }),
        screenCaptureProtection: { load: vi.fn().mockResolvedValue(false), save },
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "隐私" });
  const privacyToggle = within(page).getByRole("switch", { name: /防止截屏/ }) as HTMLInputElement;
  const toggles = screen.getAllByRole("switch", { name: /防止截屏/, hidden: true });
  expect(toggles).toHaveLength(2);
  const aboutToggle = toggles.find((toggle) => toggle !== privacyToggle) as HTMLInputElement;
  await waitFor(() => expect(privacyToggle.disabled).toBe(false));
  await waitFor(() => expect(aboutToggle.disabled).toBe(false));
  fireEvent.click(privacyToggle);
  await waitFor(() => expect(aboutToggle.checked).toBe(true));
  expect(privacyToggle.checked).toBe(true);
  fireEvent.click(aboutToggle);
  await waitFor(() => expect(privacyToggle.checked).toBe(false));
  expect(aboutToggle.checked).toBe(false);
  expect(save.mock.calls).toEqual([[true], [false]]);
});

// 上一次切换还没完成时宿主回答 `screen_capture_busy`：开关保持原值，提示重试。
test("a switch refused while the last change is still running keeps its value", async () => {
  const save = vi.fn().mockRejectedValueOnce({ code: "screen_capture_busy" });
  render(
    <SettingsPage
      initialPage="privacy"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: true, panel_windows: false }),
        screenCaptureProtection: { load: vi.fn().mockResolvedValue(true), save },
      }}
    />,
  );
  await settingsFormReady();
  const page = screen.getByRole("group", { name: "隐私" });
  const toggle = within(page).getByRole("switch", { name: /防止截屏/ }) as HTMLInputElement;
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(await screen.findByText("没有保存，请重试")).toBeTruthy();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(true);
  expect(save).toHaveBeenCalledWith(false);
});
