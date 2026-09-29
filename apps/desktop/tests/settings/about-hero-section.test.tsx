// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { AboutHeroSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the product mark, name, and platform description", () => {
  render(<AboutHeroSection logo="data:image/svg+xml,synthetic" description="桌面输入体验" />);

  expect((screen.getByRole("img", { name: "水杉 IME" }) as HTMLImageElement).src).toBe(
    "data:image/svg+xml,synthetic",
  );
  expect(screen.getByText("Metasequoia IME")).toBeTruthy();
  expect(screen.getByText("水杉 IME")).toBeTruthy();
  expect(screen.getByText("桌面输入体验")).toBeTruthy();
});
