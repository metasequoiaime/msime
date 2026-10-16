// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunityResourceDetail } from "../../../../packages/ui/src/community/use-community-resource-detail";
import type {
  CommunityResource,
  CommunityResourceClient,
} from "../../../../packages/ui/src/community/community-resource-model";

afterEach(cleanup);

function resource(): CommunityResource {
  return {
    id: "10000000-0000-4000-8000-000000000001",
    kind: "dictionary",
    name: "合成词库",
    description: "合成说明",
    author: "示例作者",
    content: { entries: [{ kind: "pinyin", code: "he", word: "合成词", weight: 100 }] },
    revision: 7,
    saves: 0,
    saved: false,
    owned: true,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}
function options(initial = resource()) {
  return {
    client: {
      detail: vi.fn<CommunityResourceClient["detail"]>(async () => initial),
      save: vi.fn<CommunityResourceClient["save"]>(async () => {}),
      apply: vi.fn<CommunityResourceClient["apply"]>(async () => ({
        revision: 8,
        imported: 2,
        resource_revision: 7,
      })),
      rate: vi.fn<CommunityResourceClient["rate"]>(async () => {}),
      unpublish: vi.fn<CommunityResourceClient["unpublish"]>(async () => {}),
      storeReply: vi.fn<NonNullable<CommunityResourceClient["storeReply"]>>(async () => {}),
      removeReply: vi.fn<NonNullable<CommunityResourceClient["removeReply"]>>(async () => {}),
      report: vi.fn<NonNullable<CommunityResourceClient["report"]>>(async () => {}),
    },
    initial,
    onClose: vi.fn(),
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

test("无需列表与发布能力即可加载详情，并格式化失败", async () => {
  const props = options();
  const loaded = { ...props.initial, name: "最新合成词库" };
  props.client.detail.mockResolvedValueOnce(loaded);
  const { result, unmount } = renderHook(() => useCommunityResourceDetail(props));
  await waitFor(() => expect(result.current.item).toEqual(loaded));
  expect(props.client.detail).toHaveBeenCalledWith(props.initial.id);
  unmount();
  props.client.detail.mockRejectedValue(new Error("合成失败"));
  const failed = renderHook(() => useCommunityResourceDetail(props));
  await waitFor(() => expect(failed.result.current.error).not.toBe(""));
  expect(failed.result.current.item).toEqual(props.initial);
});

test("收藏同步防重，完成后重读服务器状态", async () => {
  const props = options();
  const pending = deferred<void>();
  props.client.save.mockReturnValue(pending.promise);
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  props.client.detail.mockResolvedValue({ ...props.initial, saved: true });
  act(() => {
    result.current.save();
    result.current.save();
  });
  expect(props.client.save).toHaveBeenCalledExactlyOnceWith(props.initial.id, true);
  expect(result.current.busy).toBe(true);
  await act(async () => pending.resolve());
  await waitFor(() => expect(result.current.item.saved).toBe(true));
  expect(props.client.detail).toHaveBeenCalledTimes(2);
  expect(result.current.busy).toBe(false);
});

test.each([
  { action: "save", outcome: "success" },
  { action: "save", outcome: "failure" },
  { action: "rate", outcome: "success" },
  { action: "rate", outcome: "failure" },
  { action: "publish", outcome: "success" },
  { action: "publish", outcome: "failure" },
])("$action 后初始详情的 $outcome 不覆盖新结果", async ({ action, outcome }) => {
  const props = options();
  const pending = deferred<CommunityResource>();
  const updated = { ...props.initial, saved: true, my_rating: 4, revision: 8 };
  props.client.detail.mockReturnValueOnce(pending.promise).mockResolvedValue(updated);
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  if (action === "publish") {
    act(() => result.current.openEditor());
    await act(async () => result.current.onPublished());
  } else {
    act(() => {
      if (action === "save") result.current.save();
      else result.current.rateResource(4);
    });
  }
  await waitFor(() => expect(result.current.item).toEqual(updated));
  await act(async () => {
    if (outcome === "success") pending.resolve(props.initial);
    else pending.reject(new Error("过期合成加载失败"));
  });
  expect(result.current.item).toEqual(updated);
  expect(result.current.error).toBe("");
});

test.each(["success", "failure"])("收藏等待期间初始详情的 %s 已失效", async (outcome) => {
  const props = options();
  const loading = deferred<CommunityResource>();
  const saving = deferred<void>();
  props.client.detail.mockReturnValueOnce(loading.promise).mockResolvedValue({
    ...props.initial,
    saved: true,
  });
  props.client.save.mockReturnValue(saving.promise);
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  act(() => result.current.save());
  await act(async () => {
    if (outcome === "success") loading.resolve({ ...props.initial, name: "过期合成词库" });
    else loading.reject(new Error("过期合成加载失败"));
  });
  expect(result.current.busy).toBe(true);
  expect(result.current.item).toEqual(props.initial);
  expect(result.current.error).toBe("");
  await act(async () => saving.resolve());
  await waitFor(() => expect(result.current.item.saved).toBe(true));
});

test.each(["success", "failure"])("收藏后较早的发布刷新 %s 不覆盖当前结果", async (outcome) => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  act(() => result.current.openEditor());
  const pending = deferred<CommunityResource>();
  const updated = { ...props.initial, saved: true };
  props.client.detail.mockReturnValueOnce(pending.promise).mockResolvedValue(updated);
  let refreshing!: Promise<void>;
  act(() => {
    refreshing = result.current.onPublished();
  });
  act(() => result.current.save());
  await waitFor(() => expect(result.current.item).toEqual(updated));
  await act(async () => {
    if (outcome === "success") pending.resolve(props.initial);
    else pending.reject(new Error("过期合成刷新失败"));
    await refreshing;
  });
  expect(result.current.item).toEqual(updated);
  expect(result.current.error).toBe("");
});

test("当前发布刷新失败仍向调用方报告", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  act(() => result.current.openEditor());
  const failure = new Error("当前合成刷新失败");
  props.client.detail.mockRejectedValueOnce(failure);
  await act(async () => {
    await expect(result.current.onPublished()).rejects.toBe(failure);
  });
  expect(result.current.editing).toBe(false);
});

test("云端导入使用当前展示版本并显示导入数量", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  await act(async () => result.current.apply());
  expect(props.client.apply).toHaveBeenCalledExactlyOnceWith(props.initial.id, 7);
  expect(result.current.notice).toBe("已导入云端词库，新增或更新 2 个词条。");
});

test.each(["collection", "entries"])("本地 %s 安装保留详情提示", async (kind) => {
  const props = options();
  const localDictionary =
    kind === "collection"
      ? { installCollection: vi.fn(async () => {}) }
      : { import: vi.fn(async () => ({ applied: 1 })) };
  const { result } = renderHook(() => useCommunityResourceDetail({ ...props, localDictionary }));
  await act(async () => {});
  await act(async () => result.current.applyLocal());
  expect(result.current.notice).toBe(
    kind === "collection"
      ? "已添加为词库「合成词库」，可以在「词库」里停用或删除。"
      : "已导入本机词库，应用 1 个词条。",
  );
});

test("回复先收藏再加载最新内容最后存入键盘", async () => {
  const props = options({ ...resource(), kind: "reply", content: { prompt: "合成提示" } });
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  const latest = { ...props.initial, revision: 8, content: { prompt: "最新合成提示" } };
  const order: string[] = [];
  props.client.save.mockImplementation(async () => {
    order.push("save");
  });
  props.client.detail.mockImplementation(async () => {
    order.push("detail");
    return latest;
  });
  props.client.storeReply.mockImplementation(async () => {
    order.push("store");
  });
  await act(async () => result.current.storeReply());
  expect(order).toEqual(["save", "detail", "store"]);
  expect(props.client.save).toHaveBeenCalledWith(props.initial.id, true);
  expect(props.client.storeReply).toHaveBeenCalledWith(latest);
  expect(result.current.item).toEqual(latest);
  expect(result.current.notice).toContain("已添加到高情商回复键盘");
});

test("评分刷新作品，移除本机回复保留收藏提示", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  props.client.detail.mockResolvedValue({ ...props.initial, my_rating: 4 });
  await act(async () => result.current.rateResource(4));
  expect(props.client.rate).toHaveBeenCalledWith(props.initial.id, 4);
  expect(result.current.item.my_rating).toBe(4);
  expect(result.current.notice).toBe("已评分：4 星。");
  await act(async () => result.current.removeReply());
  expect(props.client.removeReply).toHaveBeenCalledWith(props.initial.id);
  expect(result.current.notice).toBe("已从本机高情商回复键盘移除，社区收藏保留。");
});

test("举报成功返回 true，失败返回 false 并显示错误", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  let reported = false;
  await act(async () => {
    reported = await result.current.report("其他", "合成举报说明");
  });
  expect(reported).toBe(true);
  expect(props.client.report).toHaveBeenCalledWith(
    "dictionary",
    props.initial.id,
    "其他",
    "合成举报说明",
  );
  expect(result.current.notice).not.toBe("");
  props.client.report.mockRejectedValueOnce(new Error("合成失败"));
  await act(async () => {
    reported = await result.current.report("其他", "合成举报说明");
  });
  expect(reported).toBe(false);
  expect(result.current.error).not.toBe("");
});

test("没有可选能力时回复与举报动作不发请求", async () => {
  const props = options();
  const {
    storeReply: _storeReply,
    removeReply: _removeReply,
    report: _report,
    ...client
  } = props.client;
  const { result } = renderHook(() => useCommunityResourceDetail({ ...props, client }));
  await act(async () => {});
  await act(async () => {
    result.current.storeReply();
  });
  await act(async () => {
    result.current.removeReply();
  });
  let reported = true;
  await act(async () => {
    reported = await result.current.report("其他", "合成说明");
  });
  expect(reported).toBe(false);
  expect(client.save).not.toHaveBeenCalled();
  expect(result.current.notice).toBe("");
});

test("下架成功才关闭详情并清除确认状态", async () => {
  const props = options();
  const pending = deferred<void>();
  props.client.unpublish.mockReturnValue(pending.promise);
  const { result } = renderHook(() => useCommunityResourceDetail(props));
  await act(async () => {});
  act(() => result.current.requestUnpublish());
  expect(result.current.confirmDelete).toBe(true);
  act(() => result.current.cancelUnpublish());
  expect(result.current.confirmDelete).toBe(false);
  act(() => {
    result.current.requestUnpublish();
    result.current.unpublish();
  });
  expect(props.onClose).not.toHaveBeenCalled();
  await act(async () => pending.resolve());
  await waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
  expect(result.current.confirmDelete).toBe(false);
});

test.each([
  { owner: "client", outcome: "success" },
  { owner: "id", outcome: "success" },
  { owner: "client", outcome: "failure" },
  { owner: "id", outcome: "failure" },
])("替换 $owner 后旧加载 $outcome 不覆盖当前详情", async ({ owner, outcome }) => {
  const props = options();
  const stale = deferred<CommunityResource>();
  props.client.detail.mockReturnValue(stale.promise);
  const { result, rerender } = renderHook((value) => useCommunityResourceDetail(value), {
    initialProps: props,
    wrapper: StrictMode,
  });
  const replacement = {
    ...props.initial,
    id: owner === "id" ? "10000000-0000-4000-8000-000000000002" : props.initial.id,
    name: "当前合成词库",
  };
  const next = owner === "id" ? { ...props, initial: replacement } : options(replacement);
  next.client.detail.mockResolvedValue(replacement);
  rerender(next);
  await waitFor(() => expect(result.current.item).toEqual(replacement));
  await act(async () => {
    if (outcome === "success") stale.resolve({ ...props.initial, name: "旧合成词库" });
    else stale.reject(new Error("过期合成加载失败"));
  });
  expect(result.current.item).toEqual(replacement);
  expect(result.current.error).toBe("");
});

test("客户端替换后旧操作失败不解除新操作锁或显示旧错误", async () => {
  const props = options();
  const next = options();
  const stale = deferred<void>();
  const current = deferred<void>();
  props.client.save.mockReturnValue(stale.promise);
  next.client.save.mockReturnValue(current.promise);
  const { result, rerender } = renderHook((value) => useCommunityResourceDetail(value), {
    initialProps: props,
    wrapper: StrictMode,
  });
  await act(async () => {});
  act(() => result.current.save());
  rerender(next);
  await act(async () => {});
  act(() => result.current.save());
  await act(async () => stale.reject(new Error("过期合成失败")));
  expect(result.current.busy).toBe(true);
  expect(result.current.error).toBe("");
  await act(async () => current.resolve());
  await waitFor(() => expect(result.current.busy).toBe(false));
});

test.each(["save", "detail", "store"])(
  "添加回复在 %s 等待时换客户端，不继续旧操作或回写状态",
  async (stage) => {
    const props = options({ ...resource(), kind: "reply" });
    const next = options({ ...props.initial, name: "当前合成回复" });
    const { result, rerender } = renderHook((value) => useCommunityResourceDetail(value), {
      initialProps: props,
      wrapper: StrictMode,
    });
    await act(async () => {});
    const pending = deferred<void>();
    const latest = { ...props.initial, name: "过期合成回复" };
    if (stage === "save") props.client.save.mockReturnValue(pending.promise);
    if (stage === "detail")
      props.client.detail.mockImplementation(() => pending.promise.then(() => latest));
    if (stage === "store") props.client.storeReply.mockReturnValue(pending.promise);
    const detailCalls = props.client.detail.mock.calls.length;
    act(() => result.current.storeReply());
    if (stage === "detail")
      await waitFor(() => expect(props.client.detail).toHaveBeenCalledTimes(detailCalls + 1));
    if (stage === "store")
      await waitFor(() => expect(props.client.storeReply).toHaveBeenCalledOnce());
    rerender(next);
    await waitFor(() => expect(result.current.item.name).toBe("当前合成回复"));
    await act(async () => pending.resolve());
    if (stage === "save") expect(props.client.detail).toHaveBeenCalledTimes(detailCalls);
    if (stage !== "store") expect(props.client.storeReply).not.toHaveBeenCalled();
    expect(result.current.item.name).toBe("当前合成回复");
    expect(result.current.notice).toBe("");
  },
);

test("编辑完成先关闭编辑器，过期刷新不覆盖新详情", async () => {
  const props = options();
  const { result, rerender } = renderHook((value) => useCommunityResourceDetail(value), {
    initialProps: props,
    wrapper: StrictMode,
  });
  await act(async () => {});
  const pending = deferred<CommunityResource>();
  props.client.detail.mockReturnValue(pending.promise);
  act(() => result.current.openEditor());
  expect(result.current.editing).toBe(true);
  let refresh!: Promise<void>;
  act(() => {
    refresh = result.current.onPublished();
  });
  expect(result.current.editing).toBe(false);
  const next = options({ ...props.initial, name: "当前合成详情" });
  rerender(next);
  await waitFor(() => expect(result.current.item.name).toBe("当前合成详情"));
  await act(async () => {
    pending.resolve({ ...props.initial, name: "旧编辑结果" });
    await refresh;
  });
  expect(result.current.item.name).toBe("当前合成详情");
});

test("卸载后旧下架不关闭调用方，新加载失败也不回写", async () => {
  const props = options();
  const pending = deferred<void>();
  const loading = deferred<CommunityResource>();
  props.client.detail.mockReturnValue(loading.promise);
  props.client.unpublish.mockReturnValue(pending.promise);
  const { result, unmount } = renderHook(() => useCommunityResourceDetail(props), {
    wrapper: StrictMode,
  });
  act(() => result.current.unpublish());
  unmount();
  await act(async () => {
    pending.resolve();
    loading.reject(new Error("过期合成加载失败"));
  });
  expect(props.onClose).not.toHaveBeenCalled();
});
