// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SkinPreviewStage } from "@msime/ui";

afterEach(cleanup);

test("renders the shared skin preview stage hook", () => {
  render(<SkinPreviewStage>候选预览</SkinPreviewStage>);

  const stage = screen.getByText("候选预览");
  expect(stage.tagName).toBe("DIV");
  expect(stage.className).toBe("flex min-w-0 items-start overflow-hidden px-6 py-[9px]");
  expect(stage.getAttribute("data-skin-stage")).toBe("");
});

test("forwards stage attributes and appends a local class", () => {
  render(
    <SkinPreviewStage className="compact" aria-label="阶段" data-testid="stage">
      内容
    </SkinPreviewStage>,
  );

  const stage = screen.getByTestId("stage");
  expect(stage.className).toContain("compact");
  expect(stage.getAttribute("aria-label")).toBe("阶段");
});
