// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DictionaryEntries } from "../../../../packages/ui/src/settings/dictionary-entries";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("marks the dictionary save action busy", () => {
  render(
    <DictionaryEntries
      kind="pinyin"
      entries={[]}
      form={{ key: "ni", value: "你", weight: 100, previous: null }}
      busy
      listRef={{ current: null }}
      onFormChange={vi.fn()}
      onSave={vi.fn()}
      onCancel={vi.fn()}
      onEdit={vi.fn()}
      onRemove={vi.fn()}
    />,
  );

  const button = screen.getByRole("button", { name: "保存" });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  expect(button.getAttribute("aria-busy")).toBe("true");
});
