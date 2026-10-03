// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsPreviewLabel } from "@msime/ui";

afterEach(cleanup);

test("renders the shared preview label style", () => {
  render(<SettingsPreviewLabel>预览</SettingsPreviewLabel>);

  const label = screen.getByText("预览");
  expect(label.tagName).toBe("DIV");
  expect(label.className).toBe("mb-[18px] text-xs text-muted");
});

test("supports paragraph semantics and forwards attributes", () => {
  render(
    <SettingsPreviewLabel as="p" className="compact" aria-label="说明" data-testid="label">
      说明内容
    </SettingsPreviewLabel>,
  );

  const label = screen.getByTestId("label");
  expect(label.tagName).toBe("P");
  expect(label.className).toContain("compact");
  expect(label.getAttribute("aria-label")).toBe("说明");
});
