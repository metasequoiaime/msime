// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useEmojiPanel, type EmojiPanelClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => (resolve = accept));
  return { promise, resolve };
}

function catalog(text = "合成项目") {
  return {
    emoji: [
      {
        title: "合成分类",
        icon: "◉",
        items: Array.from({ length: 60 }, (_, index) => ({
          text: `${text}${index}`,
          keywords: `synthetic-${index}`,
        })),
      },
    ],
    symbols: [],
    kaomoji: [],
  };
}

test("面板状态支持目录分页、搜索复位和更多入口", async () => {
  const client = { close: async () => {}, loadCatalog: async () => catalog() };
  const { result } = renderHook(() => useEmojiPanel(client));
  await waitFor(() => expect(result.current.catalogLoading).toBe(false));
  act(() => result.current.showMore("emoji"));
  expect(result.current.itemPageCount).toBe(2);
  expect(result.current.displayGroups[0].items).toHaveLength(48);
  act(() => result.current.nextItemPage());
  expect(result.current.displayGroups[0].items).toHaveLength(12);
  act(() => result.current.updateQuery("synthetic-0"));
  expect(result.current.itemPage).toBe(0);
  expect(result.current.displayGroups[0].items.map((item) => item.text)).toEqual(["合成项目0"]);
  act(() => result.current.selectPage("symbols"));
  expect(result.current.query).toBe("");
  expect(result.current.effectiveMode).toBe("copy");
});

test.each([12, 60])("颜文字插件的重叠项目各自保留并分页（新增 %i 项）", async (uniqueCount) => {
  const shared = Array.from({ length: 48 }, (_, index) => ({
    text: `合成共同项${index}`,
    keywords: `shared-${index}`,
  }));
  const unique = Array.from({ length: uniqueCount }, (_, index) => ({
    text: `合成新增项${index}`,
    keywords: `unique-${index}`,
  }));
  const client = {
    close: async () => {},
    loadCatalog: async () => ({
      emoji: [],
      symbols: [],
      kaomoji: [
        {
          title: "合成甲组",
          icon: ";-)",
          pack: "synthetic-a",
          keywords: "合成检索",
          items: shared,
        },
        {
          title: "合成乙组",
          icon: ";-)",
          pack: "synthetic-b",
          keywords: "合成检索",
          items: [...shared, ...unique],
        },
      ],
    }),
  };
  const { result } = renderHook(() => useEmojiPanel(client));
  await waitFor(() => expect(result.current.catalogLoading).toBe(false));
  act(() => result.current.selectPage("kaomoji"));
  act(() => result.current.updateQuery("合成检索"));
  const pageCount = Math.ceil((shared.length + uniqueCount) / 48);
  expect(result.current.itemPageCount).toBe(pageCount);
  const displayed: string[] = [];
  for (let page = 0; page < pageCount; page++) {
    expect(result.current.itemPage).toBe(page);
    expect(result.current.displayGroups.length).toBeGreaterThan(0);
    displayed.push(
      ...result.current.displayGroups.flatMap((group) => group.items.map((item) => item.text)),
    );
    act(() => result.current.nextItemPage());
  }
  expect(displayed).toEqual([...shared, ...shared, ...unique].map((item) => item.text));
  expect(result.current.itemPage).toBe(pageCount - 1);
  if (pageCount > 1) {
    act(() => result.current.previousItemPage());
    expect(result.current.itemPage).toBe(pageCount - 2);
  }
});

test("候选操作成功后才记录最近使用，客户端切换隔离迟到结果", async () => {
  const pending = deferred<void>();
  const first: EmojiPanelClient = { close: async () => {}, copyText: () => pending.promise };
  const second: EmojiPanelClient = { close: async () => {}, copyText: async () => {} };
  const { result, rerender } = renderHook(({ client }) => useEmojiPanel(client), {
    initialProps: { client: first },
    wrapper: StrictMode,
  });
  act(() => result.current.copy("合成旧候选"));
  expect(result.current.recent).toEqual([]);
  rerender({ client: second });
  await act(async () => pending.resolve());
  expect(result.current.recent).toEqual([]);
  await act(async () => result.current.copy("合成新候选", false, "synthetic"));
  expect(result.current.recent).toEqual([{ text: "合成新候选", keywords: "synthetic" }]);
  expect(JSON.parse(localStorage.getItem("msime.emoji.recent")!)).toEqual(result.current.recent);
  act(() => result.current.clearRecent());
  expect(result.current.recent).toEqual([]);
});

test("StrictMode 下目录客户端切换丢弃旧目录并支持失败重试", async () => {
  const pending = deferred<ReturnType<typeof catalog>>();
  const first: EmojiPanelClient = { close: async () => {}, loadCatalog: () => pending.promise };
  const loadCatalog = vi
    .fn()
    .mockRejectedValueOnce(new Error("synthetic failure"))
    .mockResolvedValue(catalog("合成新目录"));
  const second: EmojiPanelClient = { close: async () => {}, loadCatalog };
  const { result, rerender } = renderHook(({ client }) => useEmojiPanel(client), {
    initialProps: { client: first },
    wrapper: StrictMode,
  });
  await act(async () => undefined);
  rerender({ client: second });
  await waitFor(() =>
    expect(result.current.catalogUnavailable).toEqual(["emoji", "kaomoji", "symbols"]),
  );
  act(() => result.current.retryCatalog());
  await waitFor(() => expect(result.current.catalogUnavailable).toEqual([]));
  await act(async () => pending.resolve(catalog("合成旧目录")));
  act(() => result.current.selectPage("emoji"));
  expect(result.current.displayGroups[0].items[0].text).toBe("合成新目录0");
});

test("剪贴板操作互斥并在成功后刷新列表", async () => {
  const pending = deferred<void>();
  let snapshot = ["合成待删记录", "合成保留记录"];
  const remove = vi.fn(async () => {
    await pending.promise;
    snapshot = ["合成保留记录"];
  });
  const clear = vi.fn(async () => {});
  const client = {
    close: async () => {},
    clipboard: { list: async () => snapshot, remove, clear },
  };
  const { result } = renderHook(() => useEmojiPanel(client, "clipboard"));
  await waitFor(() => expect(result.current.clipboard).toHaveLength(2));
  let operation: Promise<void> | undefined;
  act(() => {
    operation = result.current.removeClipboard("合成待删记录");
  });
  expect(result.current.clipboardBusy).toBe(true);
  act(() => {
    result.current.clearClipboard();
    result.current.removeClipboard("合成待删记录");
  });
  expect(remove).toHaveBeenCalledOnce();
  expect(clear).not.toHaveBeenCalled();
  await act(async () => {
    pending.resolve();
    await operation;
  });
  expect(result.current.clipboard).toEqual(["合成保留记录"]);
  expect(result.current.clipboardBusy).toBe(false);
});

test.each(["kaomoji", "symbols"] as const)(
  "%s 重试只回退内置目录，插件随新快照更新、卸载并支持目录恢复",
  async (page) => {
    type Catalog = Awaited<ReturnType<NonNullable<EmojiPanelClient["loadCatalog"]>>>;
    const group = (title: string, pack?: string) => ({
      title,
      icon: "◉",
      ...(pack ? { pack } : {}),
      items: [{ text: title, keywords: title }],
    });
    const response = (
      groups: Catalog["symbols"],
      unavailable: Catalog["unavailable"],
    ): Catalog => ({
      emoji: [],
      kaomoji: [],
      symbols: [],
      [page]: groups,
      unavailable,
    });
    const builtin = group("合成已有内置组");
    const oldPlugin = group("合成旧插件组", "synthetic-pack");
    const newPlugin = group("合成新插件组", "synthetic-pack");
    const recovered = group("合成恢复内置组");
    const loadCatalog = vi
      .fn<NonNullable<EmojiPanelClient["loadCatalog"]>>()
      .mockResolvedValueOnce(response([builtin, oldPlugin], ["emoji"]))
      .mockResolvedValueOnce(response([newPlugin], [page]))
      .mockRejectedValueOnce(new Error("synthetic transport failure"))
      .mockResolvedValueOnce(response([], [page]))
      .mockResolvedValueOnce(response([recovered, newPlugin], []));
    const client = { close: async () => {}, loadCatalog };
    const { result } = renderHook(() => useEmojiPanel(client));
    await waitFor(() => expect(result.current.catalogLoading).toBe(false));
    act(() => result.current.selectPage(page));
    const titles = () => result.current.filteredGroups.map((entry) => entry.title);
    expect(titles()).toEqual([builtin.title, oldPlugin.title]);
    act(() => result.current.retryCatalog());
    await waitFor(() => expect(result.current.catalogLoading).toBe(false));
    expect(titles()).toEqual([builtin.title, newPlugin.title]);
    expect(result.current.catalogUnavailable).toEqual([page]);
    act(() => result.current.retryCatalog());
    await waitFor(() => expect(result.current.catalogLoading).toBe(false));
    expect(titles()).toEqual([builtin.title, newPlugin.title]);
    expect(result.current.catalogUnavailable).toEqual(["emoji", "kaomoji", "symbols"]);
    act(() => result.current.retryCatalog());
    await waitFor(() => expect(result.current.catalogLoading).toBe(false));
    expect(titles()).toEqual([builtin.title]);
    act(() => result.current.retryCatalog());
    await waitFor(() => expect(result.current.catalogLoading).toBe(false));
    expect(titles()).toEqual([recovered.title, newPlugin.title]);
    expect(result.current.catalogUnavailable).toEqual([]);
  },
);
