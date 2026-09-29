// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

afterEach(cleanup);

vi.mock("../../../../packages/ui/src/settings/settings-content-intro", () => ({
  SettingsContentIntro: ({
    pageTitle,
    hideHeaderOnPhone,
  }: {
    pageTitle: string;
    hideHeaderOnPhone: boolean;
  }) => (
    <div
      data-testid="settings-page-intro"
      data-page-title={pageTitle}
      data-header-hidden={String(hideHeaderOnPhone)}
    />
  ),
}));

import { SettingsPageIntro } from "../../../../packages/ui/src/settings/settings-page-intro";

test("derives the page title and mobile header policy from the shared page registry", () => {
  render(
    <SettingsPageIntro
      page="input"
      mobile
      availablePages={[{ id: "input", title: "输入" }]}
      status={{} as never}
      standalone={{} as never}
    />,
  );

  expect(screen.getByTestId("settings-page-intro").getAttribute("data-page-title")).toBe("输入");
  expect(screen.getByTestId("settings-page-intro").getAttribute("data-header-hidden")).toBe(
    "false",
  );
});

test("hides headings for headerless mobile top-level pages", () => {
  render(
    <SettingsPageIntro
      page="home"
      mobile
      availablePages={[{ id: "home", title: "首页" }]}
      status={{} as never}
      standalone={{} as never}
    />,
  );

  expect(screen.getByTestId("settings-page-intro").getAttribute("data-header-hidden")).toBe("true");
});
