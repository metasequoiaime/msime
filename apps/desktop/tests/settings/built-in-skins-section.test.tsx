// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { BuiltInSkinsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects a built-in skin and toggles its preview theme", () => {
  const onSelect = vi.fn();
  const onTogglePreview = vi.fn();
  render(
    <BuiltInSkinsSection
      selected="willow_green"
      previewThemes={{ willow_green: "dark" }}
      defaultTheme="light"
      linux={false}
      onSelect={onSelect}
      onTogglePreview={onTogglePreview}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "Fluent" }));
  fireEvent.click(screen.getByRole("button", { name: "预览浅色" }));

  expect(onSelect).toHaveBeenCalledWith("fluent");
  expect(onTogglePreview).toHaveBeenCalledWith("willow_green");
  expect(screen.getByText("Fluent (Light)")).toBeTruthy();
});
