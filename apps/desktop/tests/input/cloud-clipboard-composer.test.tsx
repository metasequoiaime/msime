// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CloudClipboardComposer } from "@msime/ui";

afterEach(cleanup);

test("编辑原样转发草稿并等待受控值更新，上传回调不接收事件", () => {
  const onDraftChange = vi.fn();
  const onUpload = vi.fn();
  const props = { enabled: true, busy: false, onDraftChange, onUpload };
  const view = render(<CloudClipboardComposer {...props} draft="合成初稿" />);
  const input = screen.getByRole("textbox", { name: "待上传文本" }) as HTMLTextAreaElement;
  expect(input.placeholder).toBe("输入要上传的文本");
  expect(input.rows).toBe(3);
  expect(input.hasAttribute("maxlength")).toBe(false);
  fireEvent.change(input, { target: { value: "  合成新稿\n" } });
  expect(onDraftChange.mock.calls).toEqual([["  合成新稿\n"]]);
  expect(input.value).toBe("合成初稿");
  view.rerender(<CloudClipboardComposer {...props} draft={"  合成新稿\n"} />);
  expect(input.value).toBe("  合成新稿\n");
  expect(screen.getByText("7 / 4000")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "上传明确选择的文本" }));
  expect(onUpload.mock.calls).toEqual([[]]);
});

test.each([
  { name: "空草稿", draft: "", enabled: true, busy: false, disabled: true },
  { name: "全空白", draft: " \n\t　", enabled: true, busy: false, disabled: true },
  { name: "未启用", draft: "合成草稿", enabled: false, busy: false, disabled: true },
  { name: "上传中", draft: "合成草稿", enabled: true, busy: true, disabled: true },
  { name: "有效草稿", draft: " 合成草稿 ", enabled: true, busy: false, disabled: false },
  { name: "恰好上限", draft: "x".repeat(4000), enabled: true, busy: false, disabled: false },
  { name: "超过上限", draft: "x".repeat(4001), enabled: true, busy: false, disabled: true },
  { name: "双单元恰好上限", draft: "😀".repeat(2000), enabled: true, busy: false, disabled: false },
  {
    name: "双单元超过上限",
    draft: "😀".repeat(2000) + "x",
    enabled: true,
    busy: false,
    disabled: true,
  },
])("$name 的上传许可保持", ({ draft, enabled, busy, disabled }) => {
  const onUpload = vi.fn();
  render(
    <CloudClipboardComposer
      draft={draft}
      enabled={enabled}
      busy={busy}
      onDraftChange={vi.fn()}
      onUpload={onUpload}
    />,
  );
  const button = screen.getByRole("button", { name: "上传明确选择的文本" }) as HTMLButtonElement;
  expect(button.disabled).toBe(disabled);
  fireEvent.click(button);
  expect(onUpload.mock.calls).toEqual(disabled ? [] : [[]]);
  expect(screen.getByText(`${draft.length} / 4000`)).toBeDefined();
});

test("忙状态与未启用不冻结编辑，状态恢复后允许上传", () => {
  const onDraftChange = vi.fn();
  const onUpload = vi.fn();
  const view = render(
    <CloudClipboardComposer
      draft="合成草稿"
      enabled={false}
      busy
      onDraftChange={onDraftChange}
      onUpload={onUpload}
    />,
  );
  const input = screen.getByRole("textbox", { name: "待上传文本" }) as HTMLTextAreaElement;
  expect(input.disabled).toBe(false);
  expect(input.readOnly).toBe(false);
  fireEvent.change(input, { target: { value: "合成后续草稿" } });
  expect(onDraftChange.mock.calls).toEqual([["合成后续草稿"]]);
  view.rerender(
    <CloudClipboardComposer
      draft="合成后续草稿"
      enabled
      busy={false}
      onDraftChange={onDraftChange}
      onUpload={onUpload}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "上传明确选择的文本" }));
  expect(onUpload.mock.calls).toEqual([[]]);
});

test("超限计数带警告样式，缩短后恢复", () => {
  const props = { enabled: true, busy: false, onDraftChange: vi.fn(), onUpload: vi.fn() };
  const view = render(<CloudClipboardComposer {...props} draft={"x".repeat(4001)} />);
  expect(screen.getByText("4001 / 4000").classList.contains("font-semibold")).toBe(true);
  view.rerender(<CloudClipboardComposer {...props} draft={"x".repeat(4000)} />);
  expect(screen.getByText("4000 / 4000").classList.contains("font-semibold")).toBe(false);
});
