// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CloudDictionaryTab } from "../../../../packages/ui/src/settings/cloud-dictionary-tab";
import { CloudDictionaryFilesPanel } from "../../../../packages/ui/src/keyboard/panels";
import type { CloudDictionaryPanelClient } from "../../../../packages/ui/src/keyboard/panel-client";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("云词库标签实时防抖查询，条件变化后旧响应不能覆盖结果", async () => {
  vi.useFakeTimers();
  let finishOld: (value: object) => void = () => {};
  const request = vi.fn().mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finishOld = resolve;
      }),
  );
  const client: CloudDictionaryPanelClient = { close: vi.fn(), request };
  render(<CloudDictionaryTab client={client} onClose={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "查询" })).toBeNull();
  await act(() => vi.advanceTimersByTimeAsync(280));
  expect(request).toHaveBeenCalledWith({
    operation: "list",
    kind: "pinyin",
    offset: 0,
    search: "",
  });
  const input = screen.getByRole("textbox", { name: "搜索云词条" });
  fireEvent.change(input, { target: { value: "合" } });
  await act(() => vi.advanceTimersByTimeAsync(100));
  fireEvent.change(input, { target: { value: "合成" } });
  await act(async () =>
    finishOld({
      entries: [{ id: 1, kind: "pinyin", code: "old", word: "旧响应", weight: 1, revision: 1 }],
      has_more: true,
    }),
  );
  expect(screen.queryByText("旧响应")).toBeNull();
  request.mockResolvedValue({ entries: [], has_more: false, offset: 0 });
  await act(() => vi.advanceTimersByTimeAsync(279));
  expect(request).toHaveBeenCalledTimes(1);
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(request).toHaveBeenLastCalledWith({
    operation: "list",
    kind: "pinyin",
    offset: 0,
    search: "合成",
  });
});

test("云词库加载失败显示登录说明，不冒充空词库；子页可以返回", async () => {
  const request = vi.fn().mockRejectedValue({ code: "account_unauthorized" });
  render(<CloudDictionaryTab client={{ close: vi.fn(), request }} onClose={vi.fn()} />);
  await waitFor(() => expect(screen.getByText("登录已失效，请重新登录。")).toBeTruthy());
  expect(screen.getByText("云词条加载失败")).toBeTruthy();
  expect(screen.queryByText("暂无云词条")).toBeNull();
  expect(screen.queryByText(/provider/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "完整目录" }));
  fireEvent.click(screen.getByText("返回云词库"));
  expect(screen.getByRole("textbox", { name: "搜索云词条" })).toBeTruthy();
});

test("设置页导出通过宿主保存文件，WebView 不触发浏览器下载", async () => {
  const saveExport = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockResolvedValue({ text: "synthetic", filename: "dictionary.tsv" });
  render(<CloudDictionaryFilesPanel client={{ close: vi.fn(), request, saveExport }} />);
  fireEvent.click(screen.getByRole("button", { name: "导出当前类型" }));
  await waitFor(() => expect(saveExport).toHaveBeenCalledWith("dictionary.tsv", "synthetic"));
  expect(await screen.findByText("云词库已导出")).toBeTruthy();
});
