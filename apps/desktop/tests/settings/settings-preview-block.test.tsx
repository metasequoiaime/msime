// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsPreviewBlock } from "@msime/ui";

afterEach(cleanup);

test("renders a labeled settings preview with the shared layout and label", () => {
  render(<SettingsPreviewBlock aria-label="示例预览">预览内容</SettingsPreviewBlock>);

  const preview = screen.getByLabelText("示例预览");
  expect(preview.tagName).toBe("DIV");
  expect(preview.className).toBe(
    "min-w-0 border-t border-[var(--p-row-divider)] px-6 pt-5 pb-[30px] first:border-t-0 max-phone:px-4",
  );
  expect(screen.getByText("预览").className).toBe("mb-[18px] text-xs text-muted");
  expect(screen.getByText("预览内容")).toBeTruthy();
});

test("supports section semantics and a custom preview label", () => {
  render(
    <SettingsPreviewBlock as="section" aria-label="候选预览" label="预览说明">
      候选内容
    </SettingsPreviewBlock>,
  );

  const preview = screen.getByLabelText("候选预览");
  expect(preview.tagName).toBe("SECTION");
  expect(screen.getByText("预览说明")).toBeTruthy();
});
