// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { AddDictionarySheet } from "../../../../packages/ui/src/dictionary/add-dictionary-sheet";
import { collectionImportSources } from "../../../../packages/ui/src/dictionary/dictionary-collections";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const sources = collectionImportSources(["txt", "scel", "rime", "hans"]);

test("来源面板可独立嵌入表单，按传入顺序列出来源，先关闭再新建", () => {
  const events: string[] = [];
  const onFile = vi.fn();
  const { container } = render(
    <form>
      <AddDictionarySheet
        open
        sources={sources}
        onClose={() => events.push("close")}
        onCreate={() => events.push("create")}
        onFile={onFile}
      />
    </form>,
  );
  expect(container.querySelectorAll("form")).toHaveLength(1);
  const dialog = screen.getByRole("dialog", { name: "添加词库" });
  expect(
    within(dialog)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual([
    "新建空词库",
    "导入文本文件",
    "导入搜狗细胞词库",
    "导入 Rime 词典",
    "导入纯汉字词表",
    "取消",
  ]);
  fireEvent.click(within(dialog).getByRole("button", { name: "新建空词库" }));
  expect(events).toEqual(["close", "create"]);
  expect(onFile).not.toHaveBeenCalled();
});

test("系统文件选择返回时使用最新回调，并保留打开选择器时的来源", () => {
  const click = vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(() => {});
  const first = vi.fn();
  const second = vi.fn();
  const props = { sources, onClose: vi.fn(), onCreate: vi.fn() };
  const { rerender } = render(<AddDictionarySheet {...props} open onFile={first} />);
  fireEvent.click(screen.getByRole("button", { name: "导入搜狗细胞词库" }));
  const input = screen.getByLabelText("选择词库文件") as HTMLInputElement;
  expect(input.accept).toBe(".scel,application/octet-stream");
  expect(click).toHaveBeenCalledOnce();
  expect(props.onClose).toHaveBeenCalledOnce();
  expect(first).not.toHaveBeenCalled();
  rerender(<AddDictionarySheet {...props} open={false} sources={[]} onFile={second} />);
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByLabelText("选择词库文件")).toBe(input);
  const file = new File(["synthetic"], "合成词库.scel");
  fireEvent.change(input, { target: { files: [file] } });
  expect(second).toHaveBeenCalledWith(sources[1], file);
  expect(first).not.toHaveBeenCalled();
  expect(input.value).toBe("");
});

test("取消选择文件只关闭，重新选择同一文件仍传给调用方", () => {
  vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(() => {});
  const onFile = vi.fn();
  const onClose = vi.fn();
  const props = { sources, onClose, onCreate: vi.fn(), onFile };
  const { rerender } = render(<AddDictionarySheet {...props} open />);
  fireEvent.click(screen.getByRole("button", { name: "导入 Rime 词典" }));
  const input = screen.getByLabelText("选择词库文件") as HTMLInputElement;
  expect(input.accept).toBe(".yaml,.yml,text/plain");
  fireEvent.change(input, { target: { files: [] } });
  expect(onClose).toHaveBeenCalledTimes(2);
  expect(onFile).not.toHaveBeenCalled();
  rerender(<AddDictionarySheet {...props} open={false} />);
  rerender(<AddDictionarySheet {...props} open />);
  fireEvent.click(screen.getByRole("button", { name: "导入文本文件" }));
  const file = new File(["synthetic"], "合成词库.txt");
  Object.defineProperty(input, "value", { value: "synthetic.txt", writable: true });
  fireEvent.change(input, { target: { files: [file] } });
  expect(input.value).toBe("");
  input.value = "synthetic.txt";
  fireEvent.change(input, { target: { files: [file] } });
  expect(input.value).toBe("");
  expect(onFile).toHaveBeenCalledTimes(2);
  expect(onFile).toHaveBeenLastCalledWith(sources[0], file);
});
