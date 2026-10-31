// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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

const harmonyPhone = () =>
  testHost({ platform: "harmony", mobile_settings: true, panel_windows: false });

function deleteAuthenticationSwitch(): HTMLInputElement {
  return screen.getByRole("switch", { name: "剪贴板删除数据验证" }) as HTMLInputElement;
}

// 「剪贴板删除数据验证」（#10408）：宿主提供时「剪贴板」页的「剪贴板」组多这一个开关；打开和关闭都交给宿主（宿主先认证），成功后开关跟着宿主的回答走。
test("a HarmonyOS phone turns clipboard delete authentication on and off through the host", async () => {
  const load = vi.fn().mockResolvedValue(false);
  const save = vi.fn().mockImplementation(async (enabled: boolean) => enabled);
  render(
    <SettingsPage
      initialPage="tools"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: harmonyPhone(),
        clipboardDeleteAuthentication: { load, save },
      }}
    />,
  );
  await settingsFormReady();
  const toggle = deleteAuthenticationSwitch();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(load).toHaveBeenCalledTimes(1);
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  await waitFor(() => expect(save).toHaveBeenCalledWith(true));
  await waitFor(() => expect(toggle.checked).toBe(true));
  await waitFor(() => expect(toggle.disabled).toBe(false));
  fireEvent.click(toggle);
  await waitFor(() => expect(save).toHaveBeenCalledWith(false));
  await waitFor(() => expect(toggle.checked).toBe(false));
});

// 宿主拒绝（没有锁屏、认证没通过、上一次还在认证）时开关保持原值，并按原因提示；没有锁屏时提示先去系统设置开启。
test("a refused switch keeps its value and says why", async () => {
  const save = vi
    .fn()
    .mockRejectedValueOnce({ code: "clipboard_delete_authentication_unavailable" })
    .mockRejectedValueOnce({ code: "clipboard_delete_authentication_failed" })
    .mockRejectedValueOnce({ code: "clipboard_delete_authentication_busy" });
  render(
    <SettingsPage
      initialPage="tools"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: harmonyPhone(),
        clipboardDeleteAuthentication: { load: vi.fn().mockResolvedValue(true), save },
      }}
    />,
  );
  await settingsFormReady();
  const toggle = deleteAuthenticationSwitch();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(
    await screen.findByText("请先在系统设置中开启锁屏验证（指纹、图案、密码等）后再使用"),
  ).toBeTruthy();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(await screen.findByText("未通过系统认证，剪贴板删除数据验证保持开启")).toBeTruthy();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(await screen.findByText("正在验证身份，请先完成上一次验证")).toBeTruthy();
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(toggle.checked).toBe(true);
  expect(save).toHaveBeenCalledTimes(3);
});

// 没有提供这项能力的宿主（Android、iOS 用各自的原生页，桌面没有这项）不出现这个开关。
test("hosts without the client show no clipboard delete authentication switch", async () => {
  render(
    <SettingsPage
      initialPage="tools"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: harmonyPhone(),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("switch", { name: "剪贴板删除数据验证" })).toBeNull();
  expect(screen.queryByText("剪贴板删除数据验证")).toBeNull();
});
