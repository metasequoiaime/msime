// @vitest-environment jsdom
import type { CommunityResourcesPageProps } from "@msime/ui";
import { StrictMode } from "react";
import { act, cleanup, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunityResourceGallery } from "../../../../packages/ui/src/community/use-community-resource-gallery";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";
import type {
  CommunityResource,
  CommunityResourceClient,
  CommunityResourceKind,
  CommunityResourcePage,
} from "../../../../packages/ui/src/community/community-resource-model";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function resource(id = "10000000-0000-4000-8000-000000000001"): CommunityResource {
  return {
    id,
    kind: "dictionary",
    name: "合成词库",
    description: "合成说明",
    author: "示例作者",
    content: { entries: [{ kind: "pinyin", code: "he", word: "合成词", weight: 100 }] },
    revision: 7,
    saves: 0,
    saved: false,
    owned: false,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}
function options() {
  return {
    kind: "dictionary" as CommunityResourceKind,
    client: {
      list: vi.fn<CommunityResourceClient["list"]>(async () => ({
        items: [resource()],
        has_more: true,
      })),
      detail: vi.fn<CommunityResourceClient["detail"]>(async () => resource()),
      apply: vi.fn<CommunityResourceClient["apply"]>(async () => ({
        revision: 8,
        imported: 2,
        resource_revision: 7,
      })),
      save: vi.fn<CommunityResourceClient["save"]>(async () => {}),
      storeReply: vi.fn<NonNullable<CommunityResourceClient["storeReply"]>>(async () => {}),
    },
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("最小客户端加载初始范围，搜索保留原文，分页按行数去重", async () => {
  const props = options();
  const initialScope: CommunityResourcesPageProps["initialScope"] = "saved";
  const { result } = renderHook(() => useCommunityResourceGallery({ ...props, initialScope }));
  await waitFor(() => expect(result.current.busy).toBe(false));
  expect(props.client.list).toHaveBeenCalledWith("dictionary", "saved", "", 0);
  act(() => result.current.onSearchChange(" 合成 查询 "));
  await act(async () => result.current.load(false, result.current.search));
  expect(props.client.list).toHaveBeenLastCalledWith("dictionary", "saved", " 合成 查询 ", 0);
  act(() => result.current.onSearchChange("未提交草稿"));
  const second = resource("10000000-0000-4000-8000-000000000002");
  props.client.list.mockResolvedValue({ items: [resource(), second], has_more: false });
  await act(async () => result.current.load(true));
  expect(props.client.list).toHaveBeenLastCalledWith("dictionary", "saved", " 合成 查询 ", 1);
  expect(result.current.items).toEqual([resource(), second]);
  expect(result.current.more).toBe(false);
  act(() => result.current.onScopeChange("mine"));
  await waitFor(() =>
    expect(props.client.list).toHaveBeenLastCalledWith("dictionary", "mine", " 合成 查询 ", 0),
  );
});

test.each([false, true])("加载失败 append=%s 保留行，只有首次查询失败隐藏分页", async (append) => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceGallery(props));
  await waitFor(() => expect(result.current.busy).toBe(false));
  props.client.list.mockRejectedValueOnce(new Error("合成失败"));
  await act(async () => result.current.load(append, "合成查询"));
  expect(result.current.items).toEqual([resource()]);
  expect(result.current.more).toBe(append);
  expect(result.current.error).not.toBe("");
  expect(result.current.busy).toBe(false);
});

test("同客户端的新查询使旧成功与 finally 失效", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceGallery(props));
  await waitFor(() => expect(result.current.busy).toBe(false));
  const old = deferred<CommunityResourcePage>();
  const current = deferred<CommunityResourcePage>();
  props.client.list.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
  let previous!: Promise<void>;
  let latest!: Promise<void>;
  act(() => {
    previous = result.current.load(false, "旧查询");
    latest = result.current.load(false, "新查询");
  });
  await act(async () => {
    old.resolve({ items: [], has_more: false });
    await previous;
  });
  expect(result.current.items).toEqual([resource()]);
  expect(result.current.busy).toBe(true);
  await act(async () => {
    current.resolve({ items: [], has_more: false });
    await latest;
  });
  expect(result.current.items).toEqual([]);
  expect(result.current.busy).toBe(false);
});

test.each([
  { owner: "client", outcome: "success" },
  { owner: "kind", outcome: "success" },
  { owner: "scope", outcome: "success" },
  { owner: "client", outcome: "failure" },
  { owner: "kind", outcome: "failure" },
  { owner: "scope", outcome: "failure" },
])("替换 $owner 后旧 $outcome 不覆盖新加载与忙状态", async ({ owner, outcome }) => {
  const props = options();
  const stale = deferred<CommunityResourcePage>();
  const current = deferred<CommunityResourcePage>();
  props.client.list.mockReturnValue(stale.promise);
  const { result, rerender } = renderHook((value) => useCommunityResourceGallery(value), {
    initialProps: props,
    wrapper: StrictMode,
  });
  const next =
    owner === "client"
      ? options()
      : { ...props, kind: owner === "kind" ? ("reply" as const) : props.kind };
  next.client.list.mockReturnValue(current.promise);
  if (owner === "scope") act(() => result.current.onScopeChange("mine"));
  else rerender(next);
  await act(async () => {
    if (outcome === "success") stale.resolve({ items: [resource()], has_more: true });
    else stale.reject(new Error("过期合成失败"));
  });
  expect(result.current.items).toEqual([]);
  expect(result.current.busy).toBe(true);
  expect(result.current.error).toBe("");
  await act(async () => current.resolve({ items: [], has_more: false }));
  await waitFor(() => expect(result.current.busy).toBe(false));
});

test("卸载后的加载成功不回写列表", async () => {
  const props = options();
  const pending = deferred<CommunityResourcePage>();
  props.client.list.mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() => useCommunityResourceGallery(props), {
    wrapper: StrictMode,
  });
  unmount();
  await act(async () => pending.resolve({ items: [resource()], has_more: true }));
  expect(result.current.items).toEqual([]);
});

test("替换客户端后旧添加结果不污染新图库", async () => {
  const props = options();
  const pending = deferred<CommunityResource>();
  props.client.detail.mockReturnValue(pending.promise);
  const replacement = options();
  const { result, rerender } = renderHook((value) => useCommunityResourceGallery(value), {
    initialProps: props,
    wrapper: ToastProvider,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));

  let adding!: Promise<void>;
  act(() => {
    adding = result.current.add(resource());
  });
  rerender({ ...props, client: replacement.client });
  await waitFor(() => expect(replacement.client.list).toHaveBeenCalled());

  await act(async () => {
    pending.resolve({ ...resource(), revision: 9 });
    await adding;
  });

  expect(props.client.apply).not.toHaveBeenCalled();
  expect(replacement.client.apply).not.toHaveBeenCalled();
  expect(result.current.added.has(resource().id)).toBe(false);
  expect(screen.getByRole("status").textContent).toBe("");

  await act(async () => result.current.add(resource()));
  expect(replacement.client.apply).toHaveBeenCalledWith(resource().id, resource().revision);
  expect(result.current.added.has(resource().id)).toBe(true);
});

test.each(["search", "append", "scope"])("添加期间 %s 刷新列表仍能完成添加", async (refresh) => {
  const props = options();
  const pending = deferred<CommunityResource>();
  props.client.detail.mockReturnValue(pending.promise);
  const { result } = renderHook(() => useCommunityResourceGallery(props), {
    wrapper: ToastProvider,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  let adding!: Promise<void>;
  act(() => {
    adding = result.current.add(resource());
  });
  if (refresh === "scope") {
    act(() => result.current.onScopeChange("mine"));
    await waitFor(() => expect(result.current.busy).toBe(false));
  } else {
    await act(async () => result.current.load(refresh === "append", "合成查询"));
  }
  await act(async () => {
    pending.resolve(resource());
    await adding;
  });
  expect(props.client.apply).toHaveBeenCalledWith(resource().id, resource().revision);
  expect(result.current.added.has(resource().id)).toBe(true);
  expect(result.current.adding.size).toBe(0);
  expect(screen.getByRole("status").textContent).toContain("已添加");
});

test.each(["success", "failure"])(
  "旧导入 %s 不更新新客户端或解除新添加的忙标记",
  async (outcome) => {
    const props = options();
    const previous = deferred<Awaited<ReturnType<CommunityResourceClient["apply"]>>>();
    props.client.apply.mockReturnValue(previous.promise);
    const replacement = options();
    const pending = deferred<CommunityResource>();
    replacement.client.detail.mockReturnValue(pending.promise);
    const { result, rerender } = renderHook((value) => useCommunityResourceGallery(value), {
      initialProps: props,
      wrapper: ToastProvider,
    });
    await waitFor(() => expect(result.current.busy).toBe(false));
    let oldAdding!: Promise<void>;
    act(() => {
      oldAdding = result.current.add(resource());
    });
    await waitFor(() => expect(props.client.apply).toHaveBeenCalledOnce());
    rerender(replacement);
    expect(result.current.adding.size).toBe(0);
    let newAdding!: Promise<void>;
    act(() => {
      newAdding = result.current.add(resource());
    });
    await act(async () => {
      if (outcome === "success")
        previous.resolve({ revision: 8, imported: 2, resource_revision: 7 });
      else previous.reject(new Error("旧合成失败"));
      await oldAdding;
    });
    expect(result.current.adding.has(resource().id)).toBe(true);
    expect(result.current.added.size).toBe(0);
    expect(screen.getByRole("status").textContent).toBe("");
    await act(async () => {
      pending.resolve(resource());
      await newAdding;
    });
    expect(result.current.adding.size).toBe(0);
    expect(result.current.added.has(resource().id)).toBe(true);
  },
);

test("普通详情直接选择和关闭，发布后先关编辑器再刷新", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceGallery(props));
  await waitFor(() => expect(result.current.busy).toBe(false));
  act(() => result.current.openDetail(resource()));
  expect(result.current.selected).toEqual(resource());
  expect(window.history.state).toBeNull();
  act(() => result.current.closeDetail());
  expect(result.current.selected).toBeNull();
  act(() => result.current.openEditor());
  expect(result.current.editing).toBe(true);
  act(() => result.current.closeEditor());
  expect(result.current.editing).toBe(false);
  act(() => result.current.openEditor());
  const refresh = deferred<CommunityResourcePage>();
  props.client.list.mockReturnValue(refresh.promise);
  let published!: Promise<void>;
  act(() => {
    published = result.current.onPublished();
  });
  expect(result.current.editing).toBe(false);
  expect(result.current.busy).toBe(true);
  await act(async () => {
    refresh.resolve({ items: [], has_more: false });
    await published;
  });
  expect(result.current.items).toEqual([]);
});

test("移动详情保留所属功能页与图库记录，返回交给历史事件关闭", async () => {
  const props = options();
  const state = {
    msimeSettings: true,
    page: "dictionary",
    communityGallery: { category: "dictionary" },
  };
  window.history.replaceState(state, "");
  const back = vi.spyOn(window.history, "back").mockImplementation(() => {});
  const { result } = renderHook(() =>
    useCommunityResourceGallery({ ...props, mobile: true, historyPage: "dictionary" }),
  );
  await waitFor(() => expect(result.current.busy).toBe(false));
  act(() => result.current.openDetail(resource()));
  expect(window.history.state).toMatchObject({
    ...state,
    communityDetail: { kind: "dictionary", id: resource().id },
  });
  act(() => result.current.closeDetail());
  expect(back).toHaveBeenCalledOnce();
  expect(result.current.selected).toEqual(resource());
  act(() => window.dispatchEvent(new PopStateEvent("popstate", { state })));
  expect(result.current.selected).toBeNull();
});

test("添加词库同一时刻防重，使用最新版本云端导入并标记已添加", async () => {
  const props = options();
  const pending = deferred<CommunityResource>();
  props.client.detail.mockReturnValue(pending.promise);
  const { result } = renderHook(() => useCommunityResourceGallery(props), {
    wrapper: ToastProvider,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  let adding!: Promise<void>;
  act(() => {
    adding = result.current.add(resource());
    void result.current.add(resource());
  });
  expect(props.client.detail).toHaveBeenCalledOnce();
  expect(result.current.adding.has(resource().id)).toBe(true);
  await act(async () => {
    pending.resolve({ ...resource(), revision: 9 });
    await adding;
  });
  expect(props.client.apply).toHaveBeenCalledWith(resource().id, 9);
  expect(result.current.adding.size).toBe(0);
  expect(result.current.added.has(resource().id)).toBe(true);
  expect(screen.getByRole("status").textContent).toBe(
    "已添加「合成词库」，云端词库新增或更新 2 个词条",
  );
});

test.each(["collection", "entries"])("本机 %s 路径使用最新词库并保留添加提示", async (kind) => {
  const props = options();
  const latest = { ...resource(), name: "最新合成词库" };
  props.client.detail.mockResolvedValue(latest);
  const installCollection = vi.fn(async () => {});
  const localImport = vi.fn(async () => ({ applied: 1 }));
  const localDictionary =
    kind === "collection" ? { installCollection, import: localImport } : { import: localImport };
  const { result } = renderHook(() => useCommunityResourceGallery({ ...props, localDictionary }), {
    wrapper: ToastProvider,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  await act(async () => result.current.add(resource()));
  expect(props.client.apply).not.toHaveBeenCalled();
  if (kind === "collection") {
    expect(installCollection).toHaveBeenCalledExactlyOnceWith(latest);
    expect(localImport).not.toHaveBeenCalled();
  } else {
    expect(localImport).toHaveBeenCalledWith(
      "pinyin",
      "standard",
      "合成词\the\t100",
      expect.any(String),
    );
  }
  expect(result.current.added.has(resource().id)).toBe(true);
  expect(screen.getByRole("status").textContent).toBe(
    kind === "collection"
      ? "已添加「最新合成词库」，可以在「词库」里停用或删除"
      : "已添加「最新合成词库」，本机词库应用 1 个词条",
  );
});

test("添加失败解除同步锁且不标成功，之后能够重试", async () => {
  const props = options();
  props.client.detail.mockRejectedValueOnce(new Error("合成失败"));
  const { result } = renderHook(() => useCommunityResourceGallery(props), {
    wrapper: ToastProvider,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  await act(async () => result.current.add(resource()));
  expect(result.current.adding.size).toBe(0);
  expect(result.current.added.size).toBe(0);
  expect(screen.getByRole("status").textContent).not.toBe("");
  await act(async () => result.current.add(resource()));
  expect(props.client.detail).toHaveBeenCalledTimes(2);
  expect(result.current.added.has(resource().id)).toBe(true);
});

test.each([true, false])("回复添加依次收藏、读取与可选保存，storeReply=%s", async (store) => {
  const props = options();
  const reply = {
    ...resource(),
    kind: "reply" as const,
    name: "合成回复",
    content: { prompt: "合成提示" },
  };
  props.client.detail.mockResolvedValue(reply);
  const { storeReply, ...withoutStore } = props.client;
  const client = store ? props.client : withoutStore;
  const order: string[] = [];
  props.client.save.mockImplementation(async () => {
    order.push("save");
  });
  props.client.detail.mockImplementation(async () => {
    order.push("detail");
    return reply;
  });
  storeReply.mockImplementation(async () => {
    order.push("store");
  });
  const { result } = renderHook(() => useCommunityResourceGallery({ ...props, client }), {
    wrapper: ToastProvider,
  });
  await waitFor(() => expect(result.current.busy).toBe(false));
  await act(async () => result.current.add(reply));
  expect(order).toEqual(store ? ["save", "detail", "store"] : ["save", "detail"]);
  expect(props.client.save).toHaveBeenCalledWith(reply.id, true);
  expect(result.current.added.has(reply.id)).toBe(true);
  expect(screen.getByRole("status").textContent).toBe(
    store ? "已添加「合成回复」到高情商回复键盘" : "已收藏「合成回复」",
  );
});

test("不同资源可同时添加，完成一项只清除自己的忙标记", async () => {
  const props = options();
  const first = resource();
  const second = resource("10000000-0000-4000-8000-000000000002");
  const pendingFirst = deferred<CommunityResource>();
  const pendingSecond = deferred<CommunityResource>();
  props.client.detail.mockImplementation((id) =>
    id === first.id ? pendingFirst.promise : pendingSecond.promise,
  );
  const { result } = renderHook(() => useCommunityResourceGallery(props));
  await waitFor(() => expect(result.current.busy).toBe(false));
  let a!: Promise<void>;
  let b!: Promise<void>;
  act(() => {
    a = result.current.add(first);
    b = result.current.add(second);
  });
  expect(result.current.adding).toEqual(new Set([first.id, second.id]));
  await act(async () => {
    pendingFirst.resolve(first);
    await a;
  });
  expect(result.current.adding).toEqual(new Set([second.id]));
  expect(result.current.added).toEqual(new Set([first.id]));
  await act(async () => {
    pendingSecond.resolve(second);
    await b;
  });
  expect(result.current.adding.size).toBe(0);
  expect(result.current.added).toEqual(new Set([first.id, second.id]));
});
