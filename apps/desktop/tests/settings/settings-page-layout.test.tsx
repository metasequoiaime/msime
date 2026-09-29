// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ReactNode } from "react";

afterEach(cleanup);

vi.mock("../../../../packages/ui/src/settings/window-titlebar", () => ({
  WindowTitlebar: () => <div data-testid="window-titlebar" />,
}));
vi.mock("../../../../packages/ui/src/settings/settings-navigation-chrome", () => ({
  SettingsNavigationChrome: () => <nav data-testid="settings-navigation" />,
}));

import { SettingsPageLayout } from "../../../../packages/ui/src/settings/settings-page-layout";

test("keeps titlebar, navigation, confirmation and content in the shared shell", () => {
  const contentRef = { current: null };
  render(
    <SettingsPageLayout
      mobile={false}
      confirmation={<div data-testid="confirmation" />}
      onPointerDownCapture={vi.fn()}
      windowTitlebar={{ maximized: false, onError: vi.fn() }}
      navigation={{} as never}
      contentRef={contentRef}
    >
      <div data-testid="settings-content-child" />
    </SettingsPageLayout>,
  );

  expect(screen.getByTestId("window-titlebar")).toBeDefined();
  expect(screen.getByTestId("settings-navigation")).toBeDefined();
  expect(screen.getByTestId("confirmation")).toBeDefined();
  expect(screen.getByTestId("settings-content-child")).toBeDefined();
  expect(screen.getByRole("main").getAttribute("id")).toBe("settings-content");
});

test("omits desktop chrome on mobile hosts", () => {
  render(
    <SettingsPageLayout
      mobile
      confirmation={null}
      onPointerDownCapture={vi.fn()}
      windowTitlebar={{ maximized: false, onError: vi.fn() }}
      navigation={{} as never}
      contentRef={{ current: null }}
    >
      <span />
    </SettingsPageLayout>,
  );

  expect(screen.queryByTestId("window-titlebar")).toBeNull();
});
