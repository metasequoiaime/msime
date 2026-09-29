// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsContentIntro } from "@msime/ui";

test("renders the content header and loading status together", () => {
  render(
    <SettingsContentIntro
      page="appearance"
      mobile={false}
      pageTitle="外观"
      hideHeaderOnPhone={false}
      status={{
        draft: undefined,
        error: "",
        notice: "",
        busy: true,
        recoveredBackup: "",
        canRecover: false,
        onRecover: vi.fn(),
        macos: false,
        inputSourceStartup: null,
        onOpenSettings: vi.fn(),
        onDismiss: vi.fn(),
        onError: vi.fn(),
      }}
      standalone={{
        client: { load: vi.fn(), save: vi.fn() },
        accountPlatform: undefined,
        keyboardPreviewTheme: "light",
        communityView: { category: "skin", scope: "", initialMine: false },
        communityKey: "all",
        mobileSecondaryPages: [],
        settingsPageSelection: { onOpenPage: vi.fn() },
        selectHomeScheme: vi.fn(),
        accountPageActions: {} as never,
        openAccountLogin: vi.fn(),
      }}
    />,
  );

  expect(screen.getByRole("heading", { name: "外观" })).toBeTruthy();
  expect(screen.getByRole("status").textContent).toBe("正在读取设置…");
});
