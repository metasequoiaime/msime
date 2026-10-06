// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsGroupNote } from "@msime/ui";

afterEach(cleanup);

test("renders the shared settings note with its platform style", () => {
  render(<SettingsGroupNote>这是设置说明。</SettingsGroupNote>);

  const note = screen.getByText("这是设置说明。");
  expect(note.tagName).toBe("P");
  expect(note.className).toBe(
    "m-0 [padding:var(--p-row-pad)] [font-size:var(--p-sub-fs)] [color:var(--p-sub)]",
  );
});

test("preserves a status role for live settings results", () => {
  render(<SettingsGroupNote role="status">已保存。</SettingsGroupNote>);

  expect(screen.getByRole("status").textContent).toBe("已保存。");
});

test("allows a note to add a local layout class", () => {
  render(<SettingsGroupNote className="break-anywhere">长文本。</SettingsGroupNote>);

  expect(screen.getByText("长文本。").className).toContain("break-anywhere");
});
