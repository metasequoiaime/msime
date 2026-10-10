// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import {
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

test("without a clipboard the fields are still listed but there is no 复制 button", async () => {
  renderAbout();
  const about = await screen.findByRole("group", { name: "关于" });
  const group = within(about).getByRole("region", { name: "系统信息" });
  expect(await within(group).findByText("x86_64")).toBeTruthy();
  expect(within(group).queryByRole("button", { name: "复制系统信息" })).toBeNull();
});
