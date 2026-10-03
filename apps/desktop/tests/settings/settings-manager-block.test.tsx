// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsManagerBlock } from "@msime/ui";

afterEach(cleanup);

test("renders a settings manager block with the shared block style", () => {
  render(<SettingsManagerBlock>管理内容</SettingsManagerBlock>);

  const block = screen.getByText("管理内容");
  expect(block.tagName).toBe("DIV");
  expect(block.className).toBe("flex min-w-0 flex-col gap-3 [padding:var(--p-row-pad)]");
});

test("preserves manager block attributes and appends a local class", () => {
  render(
    <SettingsManagerBlock className="danger" role="group" aria-label="管理">
      管理区域
    </SettingsManagerBlock>,
  );

  const block = screen.getByRole("group", { name: "管理" });
  expect(block.className).toContain("danger");
});
