// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

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

async function openAbout(platform: "windows" | "linux") {
  const openThirdPartyLicenses = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save: vi.fn(),
        openExternalUrl: vi.fn().mockResolvedValue(undefined),
        openThirdPartyLicenses,
        host: testHost({ platform }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(关于|关于水杉)$/ }));
  fireEvent.click(screen.getByRole("tab", { name: "许可与隐私" }));
  await screen.findByRole("button", { name: "开源许可协议" });
  return openThirdPartyLicenses;
}

// Windows 安装器把 THIRD_PARTY_NOTICES.txt 装在 server 目录的上一级，共享的关于页和 macOS 一样提供这一行。
test("the Windows about page opens the bundled third-party notices", async () => {
  const openThirdPartyLicenses = await openAbout("windows");
  fireEvent.click(screen.getByRole("button", { name: "第三方组件许可" }));
  await waitFor(() => expect(openThirdPartyLicenses).toHaveBeenCalledOnce());
});

test("a host without bundled notices does not offer the row", async () => {
  await openAbout("linux");
  expect(screen.queryByRole("button", { name: "第三方组件许可" })).toBeNull();
});

// 粤语双拼的键位方案由陳澍鋒设计（#9799）：致谢在「许可与隐私」里，点开是设计者、来源和独立实现的说明，「查看方案仓库」打开他的仓库。
test("the about page credits the 粤语双拼 designer beside the licences", async () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save: vi.fn(),
        openExternalUrl,
        host: testHost({ platform: "linux" }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(关于|关于水杉)$/ }));
  fireEvent.click(screen.getByRole("tab", { name: "许可与隐私" }));
  const row = await screen.findByRole("button", { name: "粤语双拼键位方案" });
  expect(row.textContent).toContain("陳澍鋒设计");

  fireEvent.click(row);
  const dialog = await screen.findByRole("alertdialog", { name: "粤语双拼键位方案" });
  expect(dialog.textContent).toContain("陳澍鋒（swithinchan）设计");
  expect(dialog.textContent).toContain("#9799");
  expect(dialog.textContent).toContain("独立编写");
  fireEvent.click(within(dialog).getByRole("button", { name: "查看方案仓库" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/swithinchan/rime-cantonese-shuangpin",
    ),
  );
  expect(screen.queryByRole("alertdialog")).toBeNull();
});
