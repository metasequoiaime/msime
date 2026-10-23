// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  BuiltinDictionaryDetail,
  CollectionDetail,
} from "../../../../packages/ui/src/dictionary/dictionary-details";
import type { DictionaryCollection } from "../../../../packages/ui/src/dictionary/dictionary-collections";
import type { DictionaryClient } from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const collection: DictionaryCollection = {
  id: "synthetic",
  name: "合成词库",
  kind: "pinyin",
  source: { type: "user" },
  enabled: false,
  entry_count: 3,
  pending: 2,
};

function dictionaryClient() {
  return {
    list: vi.fn<DictionaryClient["list"]>(async () => ({ entries: [], has_more: false })),
    edit: vi.fn<DictionaryClient["edit"]>(async () => {}),
  };
}

test("命名词库详情独立挂载，更新属性和回调时保留添加草稿，只有成功才关闭", async () => {
  const first = vi.fn(async () => undefined);
  const second = vi.fn(async () => ({ collections: [], formats: [] }));
  const onEnabled = vi.fn();
  const onDelete = vi.fn(async () => {});
  const props = { collection, busy: false, onEnabled, onDelete };
  const { rerender } = render(<CollectionDetail {...props} onAddWord={first} />);
  fireEvent.click(screen.getByRole("switch", { name: "启用此词库" }));
  expect(onEnabled).toHaveBeenCalledWith(true);
  expect(screen.getByText("3 条，还有 2 条等键盘同步")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "添加词条" }));
  fireEvent.change(screen.getByRole("textbox", { name: "词语" }), { target: { value: "合成" } });
  fireEvent.change(screen.getByRole("textbox", { name: "拼音" }), {
    target: { value: "He Cheng" },
  });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  await waitFor(() => expect(first).toHaveBeenCalledWith("合成", "he'cheng"));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "添加" })).toHaveProperty("disabled", false),
  );
  expect(screen.getByRole("dialog", { name: "添加词条" })).toBeTruthy();
  rerender(
    <CollectionDetail
      {...props}
      collection={{ ...collection, enabled: true }}
      onAddWord={second}
    />,
  );
  expect(screen.getByRole("switch", { name: "启用此词库" })).toHaveProperty("checked", true);
  expect(screen.getByRole("textbox", { name: "词语" })).toHaveProperty("value", "合成");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  await waitFor(() => expect(second).toHaveBeenCalledWith("合成", "he'cheng"));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(onDelete).not.toHaveBeenCalled();
});

test("命名词库详情忙碌时禁用操作，删除需要确认", async () => {
  const onDelete = vi.fn(async () => {});
  const props = {
    collection,
    onDelete,
    onEnabled: vi.fn(),
    onAddWord: vi.fn(async () => undefined),
  };
  const { rerender } = render(<CollectionDetail {...props} busy />);
  for (const name of ["添加词条", "删除此词库"])
    expect(screen.getByRole("button", { name })).toHaveProperty("disabled", true);
  expect(screen.getByRole("switch", { name: "启用此词库" })).toHaveProperty("disabled", true);
  rerender(<CollectionDetail {...props} busy={false} />);
  fireEvent.click(screen.getByRole("button", { name: "删除此词库" }));
  const confirm = screen.getByRole("alertdialog");
  expect(onDelete).not.toHaveBeenCalled();
  fireEvent.click(within(confirm).getByRole("button", { name: "取消" }));
  expect(onDelete).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "删除此词库" }));
  fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "删除" }));
  await waitFor(() => expect(onDelete).toHaveBeenCalledOnce());
});

test("内置详情的导出能力由回调提供，取消不提示并使用最新回调", async () => {
  const dictionary = dictionaryClient();
  const first = vi.fn(async () => "");
  const second = vi.fn(async () => "合成导出完成");
  const tree = (onExport?: () => Promise<string>) => (
    <ToastProvider>
      <BuiltinDictionaryDetail dictionary={dictionary} count={null} onExport={onExport} />
    </ToastProvider>
  );
  const { rerender } = render(tree());
  expect(screen.queryByRole("button", { name: "导出词库" })).toBeNull();
  rerender(tree(first));
  fireEvent.click(screen.getByRole("button", { name: "导出词库" }));
  await waitFor(() => expect(first).toHaveBeenCalledOnce());
  expect(screen.getByRole("status").textContent).toBe("");
  rerender(tree(second));
  fireEvent.click(screen.getByRole("button", { name: "导出词库" }));
  expect(await screen.findByText("合成导出完成")).toBeTruthy();
  expect(second).toHaveBeenCalledOnce();
});

test("独立内置详情保留搜索与分页，客户端变化后丢弃旧分页响应", async () => {
  vi.useFakeTimers();
  const dictionary = dictionaryClient();
  dictionary.list.mockResolvedValueOnce({
    entries: [{ kind: "pinyin", key: "he", value: "合成首词", weight: 10 }],
    has_more: true,
  });
  const { rerender } = render(<BuiltinDictionaryDetail dictionary={dictionary} count={0} />);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(screen.getByText("合成首词")).toBeTruthy();
  let finish!: (page: Awaited<ReturnType<DictionaryClient["list"]>>) => void;
  dictionary.list.mockReturnValueOnce(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  expect(dictionary.list).toHaveBeenLastCalledWith(1, 100, "pinyin", undefined);
  const next = dictionaryClient();
  rerender(<BuiltinDictionaryDetail dictionary={next} count={0} />);
  await act(async () => {
    finish({
      entries: [{ kind: "pinyin", key: "jiu", value: "合成旧页", weight: 1 }],
      has_more: false,
    });
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(screen.queryByText("合成旧页")).toBeNull();
  fireEvent.change(screen.getByRole("searchbox", { name: "搜索词条" }), {
    target: { value: "He Cheng" },
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(249);
  });
  expect(next.list).toHaveBeenCalledOnce();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(next.list).toHaveBeenLastCalledWith(0, 100, "pinyin", "he'cheng");
});

test("按词语过滤后，加载更多从宿主已读取条数继续", async () => {
  vi.useFakeTimers();
  const dictionary = dictionaryClient();
  render(<BuiltinDictionaryDetail dictionary={dictionary} count={0} />);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  dictionary.list.mockResolvedValueOnce({
    entries: [
      { kind: "pinyin", key: "jia", value: "甲", weight: 1 },
      { kind: "pinyin", key: "hecheng", value: "合成首词", weight: 2 },
      { kind: "pinyin", key: "yi", value: "乙", weight: 3 },
    ],
    has_more: true,
  });
  fireEvent.change(screen.getByRole("searchbox", { name: "搜索词条" }), {
    target: { value: "合成" },
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
  });
  expect(screen.getByText("合成首词")).toBeTruthy();
  expect(screen.queryByText("甲")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  expect(dictionary.list).toHaveBeenLastCalledWith(3, 100, "pinyin", undefined);
  await act(async () => {});
});

test("内置详情添加成功后通过共享状态刷新当前查询并关闭对话框", async () => {
  vi.useFakeTimers();
  const dictionary = dictionaryClient();
  render(
    <ToastProvider>
      <BuiltinDictionaryDetail dictionary={dictionary} count={0} />
    </ToastProvider>,
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  fireEvent.change(screen.getByRole("searchbox", { name: "搜索词条" }), {
    target: { value: "He Cheng" },
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(250);
  });
  dictionary.list.mockResolvedValueOnce({
    entries: [{ kind: "pinyin", key: "he'cheng", value: "合成词条", weight: 10 }],
    has_more: false,
  });
  fireEvent.click(screen.getByRole("button", { name: "添加词条" }));
  fireEvent.change(screen.getByRole("textbox", { name: "词语" }), {
    target: { value: "合成词条" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "拼音" }), {
    target: { value: "He Cheng" },
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "添加" }));
  });
  expect(dictionary.edit).toHaveBeenCalledExactlyOnceWith(
    null,
    { kind: "pinyin", key: "he'cheng", value: "合成词条", weight: 10 },
    expect.stringMatching(/^phone-dictionary-\d+$/),
  );
  expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", "he'cheng");
  expect(screen.getByText("合成词条")).toBeTruthy();
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("已添加");
});
