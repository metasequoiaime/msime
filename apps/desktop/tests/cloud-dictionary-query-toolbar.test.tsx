// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type ToolbarProps = {
  kind: "pinyin" | "wubi" | "quick" | "english";
  busy: boolean;
  queryLabel: string;
  queryAriaLabel: string;
  queryValue: string;
  queryButtonLabel: string;
  queryDisabled?: boolean;
  placeholder?: string;
  inputClassName?: string;
  onKindChange: (kind: ToolbarProps["kind"]) => void;
  onQueryChange: (value: string) => void;
  onQuery: () => void;
  children?: ReactNode;
};

test("cloud dictionary query toolbar shares kind, input, and submit behavior", () => {
  const Toolbar = (ui as unknown as { CloudDictionaryQueryToolbar: ComponentType<ToolbarProps> })
    .CloudDictionaryQueryToolbar;
  expect(Toolbar).toBeDefined();

  const onKindChange = vi.fn();
  const onQueryChange = vi.fn();
  const onQuery = vi.fn();
  render(
    <Toolbar
      kind="pinyin"
      busy={false}
      queryLabel="编码"
      queryAriaLabel="云端候选编码"
      queryValue=""
      queryButtonLabel="查询云端候选"
      onKindChange={onKindChange}
      onQueryChange={onQueryChange}
      onQuery={onQuery}
    >
      <button type="button">附加操作</button>
    </Toolbar>,
  );

  fireEvent.change(screen.getByRole("combobox", { name: "词库类型" }), {
    target: { value: "wubi" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "he" },
  });
  fireEvent.keyDown(screen.getByRole("textbox", { name: "云端候选编码" }), { key: "Enter" });
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  expect(onKindChange).toHaveBeenCalledWith("wubi");
  expect(onQueryChange).toHaveBeenCalledWith("he");
  expect(onQuery).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("button", { name: "附加操作" })).toBeTruthy();
});

test.each([
  { name: "中文组词期间", event: { isComposing: true } },
  { name: "组词结束的兼容事件", event: { isComposing: false, keyCode: 229 } },
])("$name 的回车不发起云词库查询", ({ event }) => {
  const onQuery = vi.fn();
  render(
    <ui.CloudDictionaryQueryToolbar
      kind="pinyin"
      busy={false}
      queryLabel="搜索"
      queryAriaLabel="搜索云词条"
      queryValue="合成词条"
      queryButtonLabel="查询"
      onKindChange={vi.fn()}
      onQueryChange={vi.fn()}
      onQuery={onQuery}
    />,
  );
  const input = screen.getByRole("textbox", { name: "搜索云词条" });
  fireEvent.keyDown(input, { key: "Enter", ...event });
  expect(onQuery).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: "Enter", isComposing: false, keyCode: 13 });
  expect(onQuery).toHaveBeenCalledTimes(1);
});

test.each([
  { name: "查询进行中", busy: true, queryDisabled: false },
  { name: "查询条件不可用", busy: false, queryDisabled: true },
])("$name 时回车和按钮都不发起云词库查询", ({ busy, queryDisabled }) => {
  const onQuery = vi.fn();
  const { rerender } = render(
    <ui.CloudDictionaryQueryToolbar
      kind="pinyin"
      busy={busy}
      queryDisabled={queryDisabled}
      queryLabel="搜索"
      queryAriaLabel="搜索云词条"
      queryValue="合成词条"
      queryButtonLabel="查询"
      onKindChange={vi.fn()}
      onQueryChange={vi.fn()}
      onQuery={onQuery}
    />,
  );
  fireEvent.keyDown(screen.getByRole("textbox", { name: "搜索云词条" }), { key: "Enter" });
  fireEvent.click(screen.getByRole("button", { name: "查询" }));
  expect(onQuery).not.toHaveBeenCalled();
  rerender(
    <ui.CloudDictionaryQueryToolbar
      kind="pinyin"
      busy={false}
      queryDisabled={false}
      queryLabel="搜索"
      queryAriaLabel="搜索云词条"
      queryValue="合成词条"
      queryButtonLabel="查询"
      onKindChange={vi.fn()}
      onQueryChange={vi.fn()}
      onQuery={onQuery}
    />,
  );
  fireEvent.keyDown(screen.getByRole("textbox", { name: "搜索云词条" }), { key: "Enter" });
  expect(onQuery).toHaveBeenCalledTimes(1);
});
