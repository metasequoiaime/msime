// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import {
  SystemInfoSection,
  systemInfoEntries,
  systemInfoText,
} from "../../../../packages/ui/src/settings/system-info-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const linuxHost = {
  platform: "linux" as const,
  os_version: "Synthetic Linux 1.0",
  arch: "x86_64",
  kernel_version: "7.0.0-synthetic",
  desktop_session: "GNOME (wayland)",
  input_method_framework: "IBus",
  device_model: "SyntheticVendor Model 14",
};

test("lists every field the host reports, in a fixed order, and copies them one per line", () => {
  const entries = systemInfoEntries({ appVersion: "0.11.0", host: linuxHost, scheme: "全拼" });
  expect(entries).toEqual([
    { label: "应用版本", value: "v0.11.0" },
    { label: "系统", value: "Linux Synthetic Linux 1.0" },
    { label: "内核", value: "7.0.0-synthetic" },
    { label: "桌面环境", value: "GNOME (wayland)" },
    { label: "输入法框架", value: "IBus" },
    { label: "设备型号", value: "SyntheticVendor Model 14" },
    { label: "处理器架构", value: "x86_64" },
    { label: "输入方案", value: "全拼" },
  ]);
  expect(systemInfoText(entries)).toBe(
    [
      "水杉 IME 系统信息",
      "应用版本：v0.11.0",
      "系统：Linux Synthetic Linux 1.0",
      "内核：7.0.0-synthetic",
      "桌面环境：GNOME (wayland)",
      "输入法框架：IBus",
      "设备型号：SyntheticVendor Model 14",
      "处理器架构：x86_64",
      "输入方案：全拼",
    ].join("\n"),
  );
});

test("leaves out what the host cannot report instead of writing 未知", () => {
  expect(
    systemInfoEntries({
      appVersion: "1.2.0",
      host: { platform: "macos", os_version: "15.4", arch: "aarch64" },
      scheme: "双拼",
    }),
  ).toEqual([
    { label: "应用版本", value: "v1.2.0" },
    { label: "系统", value: "macOS 15.4" },
    { label: "处理器架构", value: "aarch64" },
    { label: "输入方案", value: "双拼" },
  ]);
  // 没有系统版本时只写平台名。
  expect(systemInfoEntries({ appVersion: "1.2.0", host: { platform: "windows" } })).toEqual([
    { label: "应用版本", value: "v1.2.0" },
    { label: "系统", value: "Windows" },
  ]);
});

test("names the edition next to the version when it is not full", () => {
  expect(
    systemInfoEntries({
      appVersion: "0.11.0",
      host: { platform: "linux", edition: { display_name: "水杉五笔" } },
    })[0],
  ).toEqual({ label: "应用版本", value: "v0.11.0（水杉五笔）" });
});

test("writes the release date after the version, after the edition name when there is one", () => {
  expect(systemInfoEntries({ appVersion: "0.11.0", releaseDate: "2026-10-19" })[0]).toEqual({
    label: "应用版本",
    value: "v0.11.0（2026-10-19 发布）",
  });
  expect(
    systemInfoEntries({
      appVersion: "0.11.0",
      releaseDate: "2026-10-19",
      host: { platform: "linux", edition: { display_name: "水杉五笔" } },
    })[0],
  ).toEqual({ label: "应用版本", value: "v0.11.0（水杉五笔，2026-10-19 发布）" });
  // 不知道发布日期时与之前一样只写版本号。
  expect(systemInfoEntries({ appVersion: "0.11.0", releaseDate: "" })[0]).toEqual({
    label: "应用版本",
    value: "v0.11.0",
  });
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function renderAbout(copyText?: (text: string) => Promise<void>) {
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("0.11.0"),
        copyText,
        host: testHost(linuxHost),
      }}
    />,
  );
}

test("the 关于 page shows the 系统信息 group and its 复制 button copies the text", async () => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  renderAbout(copyText);
  const about = await screen.findByRole("group", { name: "关于" });
  fireEvent.click(within(about).getByRole("tab", { name: "系统信息" }));
  const group = within(about).getByRole("region", { name: "系统信息" });

  expect(await within(group).findByText("v0.11.0")).toBeTruthy();
  expect(within(group).getByText("Linux Synthetic Linux 1.0")).toBeTruthy();
  expect(within(group).getByText("IBus")).toBeTruthy();
  expect(within(group).getByText("全拼")).toBeTruthy();

  fireEvent.click(within(group).getByRole("button", { name: "复制系统信息" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledTimes(1));
  const copied = copyText.mock.calls[0][0] as string;
  expect(copied.split("\n")[0]).toBe("水杉 IME 系统信息");
  expect(copied).toContain("输入法框架：IBus");
  expect(copied).toContain("应用版本：v0.11.0");
  expect(await within(group).findByText("已复制")).toBeTruthy();
});

test("the 系统信息 group and its copy carry the release date the host reports", async () => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("0.11.0"),
        readAppReleaseDate: vi.fn().mockResolvedValue("2026-10-19"),
        copyText,
        host: testHost(linuxHost),
      }}
    />,
  );
  const about = await screen.findByRole("group", { name: "关于" });
  fireEvent.click(within(about).getByRole("tab", { name: "系统信息" }));
  const group = within(about).getByRole("region", { name: "系统信息" });
  expect(await within(group).findByText("v0.11.0（2026-10-19 发布）")).toBeTruthy();
  fireEvent.click(within(group).getByRole("button", { name: "复制系统信息" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledTimes(1));
  expect(copyText.mock.calls[0][0]).toContain("应用版本：v0.11.0（2026-10-19 发布）");
});

test("without a clipboard the fields are still listed but there is no 复制 button", async () => {
  renderAbout();
  const about = await screen.findByRole("group", { name: "关于" });
  fireEvent.click(within(about).getByRole("tab", { name: "系统信息" }));
  const group = within(about).getByRole("region", { name: "系统信息" });
  expect(await within(group).findByText("x86_64")).toBeTruthy();
  expect(within(group).queryByRole("button", { name: "复制系统信息" })).toBeNull();
});

const copyFailure = "复制系统信息失败，请重试。";

async function openSystemInfo() {
  const about = await screen.findByRole("group", { name: "关于" });
  fireEvent.click(within(about).getByRole("tab", { name: "系统信息" }));
  return within(about).getByRole("region", { name: "系统信息" });
}

test.each(["异步拒绝", "同步异常"])("系统信息复制的%s显示可重试的错误", async (failure) => {
  const copyText = vi.fn<(text: string) => Promise<void>>().mockImplementation(() => {
    if (failure === "同步异常") throw new Error("synthetic clipboard failure");
    return Promise.reject({ code: "unavailable" });
  });
  renderAbout(copyText);
  const group = await openSystemInfo();
  fireEvent.click(within(group).getByRole("button", { name: "复制系统信息" }));

  await waitFor(() => expect(screen.queryByText(copyFailure)).not.toBeNull());
  expect(within(group).queryByText("已复制")).toBeNull();
});

test("系统信息复制重试开始时清除旧错误，成功后才显示已复制", async () => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const copyText = vi
    .fn<(text: string) => Promise<void>>()
    .mockRejectedValueOnce({ code: "unavailable" })
    .mockReturnValueOnce(pending);
  renderAbout(copyText);
  const group = await openSystemInfo();
  const button = within(group).getByRole("button", { name: "复制系统信息" });
  fireEvent.click(button);
  await waitFor(() => expect(screen.queryByText(copyFailure)).not.toBeNull());

  fireEvent.click(button);
  expect(screen.queryByText(copyFailure)).toBeNull();
  expect(within(group).queryByText("已复制")).toBeNull();
  await act(async () => finish());

  expect(within(group).getByText("已复制")).toBeTruthy();
  expect(screen.queryByText(copyFailure)).toBeNull();
  expect(copyText).toHaveBeenCalledTimes(2);
  expect(copyText.mock.calls[1][0]).toContain("水杉 IME 系统信息\n");
});

test.each(["成功", "失败"])("系统信息组件卸载后忽略迟到的复制%s", async (outcome) => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve, reject) => {
    finish = () => (outcome === "成功" ? resolve() : reject({ code: "unavailable" }));
  });
  const setError = vi.fn();
  const { unmount } = render(
    <SystemInfoSection appVersion="0.11.0" copyText={() => pending} setError={setError} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "复制系统信息" }));
  setError.mockClear();
  unmount();
  await act(async () => finish());

  expect(setError).not.toHaveBeenCalled();
});
