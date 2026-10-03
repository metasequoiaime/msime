// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SkinPreviewSurface } from "@msime/ui";

afterEach(cleanup);

test("renders the shared skin preview surface hook", () => {
  render(<SkinPreviewSurface>皮肤预览</SkinPreviewSurface>);

  const surface = screen.getByText("皮肤预览");
  expect(surface.tagName).toBe("DIV");
  expect(surface.className).toBe(
    "skin-card-preview flex flex-col bg-[var(--skin-preview-stage-bg)] py-[9px] [&_.candidate]:max-w-full [&_.candidate]:min-w-0 [&_.candidate]:text-base [&_.wnd-v_.container]:w-fit [&_.wnd-v_.container]:max-w-full",
  );
  expect(surface.getAttribute("data-skin-preview")).toBe("");
});

test("forwards surface attributes and appends a local class", () => {
  render(
    <SkinPreviewSurface className="compact" aria-label="皮肤预览" data-testid="surface">
      内容
    </SkinPreviewSurface>,
  );

  const surface = screen.getByTestId("surface");
  expect(surface.className).toContain("compact");
  expect(surface.getAttribute("aria-label")).toBe("皮肤预览");
});
