// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DictionaryEntries } from "../../../../packages/ui/src/settings/dictionary-entries";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("distinguishes an unqueried dictionary from a completed empty result", () => {
  const props = {
    kind: "pinyin" as const,
    entries: [],
    form: null,
    busy: false,
    listRef: { current: null },
    onFormChange: vi.fn(),
    onSave: vi.fn(),
    onCancel: vi.fn(),
    onEdit: vi.fn(),
    onRemove: vi.fn(),
  };
  const { rerender } = render(<DictionaryEntries {...props} />);
  expect(screen.getByText("输入关键词后查看中文 · 拼音词条")).toBeTruthy();
  rerender(<DictionaryEntries {...props} emptyMessage="未找到匹配词条" />);
  expect(screen.getByText("未找到匹配词条")).toBeTruthy();
  expect(screen.queryByText("输入关键词后查看中文 · 拼音词条")).toBeNull();
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

test("explains the cursor marker only when editing a quick phrase", () => {
  const props = {
    entries: [],
    form: { key: "tt", value: 'title:"${cursor}"', weight: 100, previous: null },
    busy: false,
    listRef: { current: null },
    onFormChange: vi.fn(),
    onSave: vi.fn(),
    onCancel: vi.fn(),
    onEdit: vi.fn(),
    onRemove: vi.fn(),
  };
  const { rerender } = render(<DictionaryEntries kind="quick_phrase" {...props} />);
  expect(screen.getByText(/编辑器不支持定位时光标留在末尾。以前存过/)).toBeTruthy();
  rerender(<DictionaryEntries kind="pinyin" {...props} />);
  expect(screen.queryByText(/编辑器不支持定位时光标留在末尾/)).toBeNull();
});
