// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SettingsStandalonePages, type SettingsClient } from "@msime/ui";

test("renders the standalone More settings page and routes its selection", () => {
  const onOpenPage = vi.fn();
  render(
    <SettingsStandalonePages
      page="more"
      client={{} as SettingsClient}
      mobile
      accountPlatform={undefined}
      keyboardPreviewTheme="light"
      communityView={{ category: "skin", scope: "", initialMine: false }}
      communityKey="all"
      mobileSecondaryPages={[{ id: "about", title: "关于", icon: "about.svg" }]}
      settingsPageSelection={{ onOpenPage }}
      selectHomeScheme={vi.fn()}
      accountPageActions={{} as never}
      openAccountLogin={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "关于" }));

  expect(onOpenPage).toHaveBeenCalledWith("about");
});
