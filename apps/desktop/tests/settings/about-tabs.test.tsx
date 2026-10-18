// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";

const snapshot: Snapshot = {
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
afterEach(cleanup);
const client = () => ({
  load: vi.fn().mockResolvedValue(snapshot),
  save: vi.fn(),
  host: testHost({ platform: "macos" }),
});

test("关于默认显示版本，切换分类保留反馈草稿，帮助返回原标签", async () => {
  render(<SettingsPage initialPage="about" client={client()} />);
  const tabs = await screen.findByRole("tablist", { name: "关于设置分类" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["版本更新", "帮助反馈", "系统信息", "许可隐私"]);
  expect(screen.getByRole("tabpanel").getAttribute("aria-labelledby")).toBe(
    within(tabs).getByRole("tab", { name: "版本更新" }).id,
  );
  expect(screen.queryByRole("textbox", { name: "反馈描述" })).toBeNull();
  fireEvent.click(within(tabs).getByRole("tab", { name: "帮助反馈" }));
  fireEvent.change(screen.getByRole("textbox", { name: "反馈描述" }), {
    target: { value: "synthetic feedback draft" },
  });
  fireEvent.click(within(tabs).getByRole("tab", { name: "系统信息" }));
  expect(screen.getByRole("region", { name: "系统信息" })).toBeTruthy();
  fireEvent.click(within(tabs).getByRole("tab", { name: "许可隐私" }));
  expect(screen.getByRole("button", { name: "隐私政策" })).toBeTruthy();
  fireEvent.click(within(tabs).getByRole("tab", { name: "帮助反馈" }));
  expect((screen.getByRole("textbox", { name: "反馈描述" }) as HTMLTextAreaElement).value).toBe(
    "synthetic feedback draft",
  );
  fireEvent.click(screen.getByRole("button", { name: "帮助" }));
  fireEvent.click(await screen.findByRole("button", { name: "返回关于水杉" }));
  expect(screen.getByRole("tab", { name: "帮助反馈" }).getAttribute("aria-selected")).toBe("true");
});

test("反馈路由初次与再次打开都定位帮助与反馈", async () => {
  const settingsClient = client();
  const view = render(
    <SettingsPage
      initialPage="feedback"
      route={{ page: "feedback", nonce: 1 }}
      client={settingsClient}
    />,
  );
  expect((await screen.findByRole("tab", { name: "帮助反馈" })).getAttribute("aria-selected")).toBe(
    "true",
  );
  fireEvent.click(screen.getByRole("tab", { name: "版本更新" }));
  view.rerender(
    <SettingsPage
      initialPage="feedback"
      route={{ page: "feedback", nonce: 2 }}
      client={settingsClient}
    />,
  );
  expect(screen.getByRole("tab", { name: "帮助反馈" }).getAttribute("aria-selected")).toBe("true");
});

test.each([
  ["ios", {}, []],
  ["harmony", {}, []],
  [
    "harmony",
    { mobile_settings: false, panel_windows: true },
    ["版本更新", "帮助反馈", "许可隐私"],
  ],
])("%s 按宿主形态显示关于标签", async (platform, capabilities, expected) => {
  render(
    <SettingsPage
      initialPage="about"
      client={{ ...client(), host: testHost({ platform, ...capabilities }) }}
    />,
  );
  const about = await screen.findByRole("group", { name: "关于" });
  expect(
    within(about)
      .queryAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(expected);
});
