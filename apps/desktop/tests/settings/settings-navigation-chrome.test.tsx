// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SettingsNavigationChrome } from "@msime/ui";

test("renders mobile and sidebar navigation from one chrome component", () => {
  const onSelectTab = vi.fn();
  const onSelectPage = vi.fn();
  render(
    <SettingsNavigationChrome
      mobile
      tabs={[{ id: "home", title: "首页", icon: "home.svg" }]}
      activeTab="home"
      onSelectTab={onSelectTab}
      groups={[[{ id: "appearance", title: "外观", icon: "appearance.svg" }]]}
      selectedPage="appearance"
      onSelectPage={onSelectPage}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "键盘" }));
  fireEvent.click(screen.getByRole("button", { name: "外观" }));

  expect(onSelectTab).toHaveBeenCalledWith("home");
  expect(onSelectPage).toHaveBeenCalledWith("appearance");
});
