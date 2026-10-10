// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
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
