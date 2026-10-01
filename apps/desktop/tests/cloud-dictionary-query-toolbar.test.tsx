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
