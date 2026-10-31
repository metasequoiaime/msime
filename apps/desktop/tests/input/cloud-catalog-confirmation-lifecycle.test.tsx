// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CloudDictionaryCatalogPanel } from "@msime/ui";

afterEach(cleanup);

test("更换客户端后回答旧目录删除确认不调用旧宿主", async () => {
  const entry = { kind: "pinyin", code: "synthetic", word: "合成目录词条", weight: 10 };
  const request = vi.fn().mockResolvedValue({ catalog_entries: [entry], revision: 7 });
  const panel = render(<CloudDictionaryCatalogPanel client={{ close: vi.fn(), request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "完整目录编码" }), {
    target: { value: "synthetic" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询完整目录" }));
  await screen.findByText(entry.word);
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  const nextRequest = vi.fn().mockResolvedValue({ catalog_entries: [], revision: 9 });
  panel.rerender(<CloudDictionaryCatalogPanel client={{ close: vi.fn(), request: nextRequest }} />);
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(request).not.toHaveBeenCalledWith(expect.objectContaining({ operation: "edit_catalog" }));
  expect(nextRequest).not.toHaveBeenCalled();
});

test.each(["confirm", "cancel"] as const)("当前目录删除的 %s 保留原参数与刷新", async (answer) => {
  const entry = { kind: "pinyin", code: "synthetic", word: "合成当前目录词条", weight: 10 };
  const request = vi.fn().mockResolvedValue({ catalog_entries: [entry], revision: 7 });
  render(<CloudDictionaryCatalogPanel client={{ close: vi.fn(), request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "完整目录编码" }), {
    target: { value: "synthetic" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询完整目录" }));
  await screen.findByText(entry.word);
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  const dialog = screen.getByRole("alertdialog");
  await act(async () => {
    fireEvent.click(
      within(dialog).getByRole("button", { name: answer === "confirm" ? "删除" : "取消" }),
    );
  });
  const action = {
    operation: "edit_catalog",
    kind: "pinyin",
    code: entry.code,
    word: entry.word,
    revision: 7,
    replacement: null,
  };
  if (answer === "confirm") {
    expect(request).toHaveBeenCalledWith(action);
    expect(screen.getByRole("status").textContent).toBe("目录词条已删除");
    expect(request).toHaveBeenCalledTimes(3);
  } else {
    expect(request).not.toHaveBeenCalledWith(action);
    expect(request).toHaveBeenCalledTimes(1);
  }
});
