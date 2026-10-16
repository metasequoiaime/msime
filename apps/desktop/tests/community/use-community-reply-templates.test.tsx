// @vitest-environment jsdom
import { StrictMode, type ReactNode } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  CommunityReplyTemplates,
  ToastProvider,
  useCommunityReplyTemplates,
  type CommunityResource,
  type CommunityResourceClient,
  type CommunityResourcePage,
} from "@msime/ui";

afterEach(cleanup);

const item: CommunityResource = {
  id: "synthetic-reply",
  kind: "reply",
  name: "合成回复",
  description: "合成说明",
  author: "合成作者",
  content: { prompt: "合成模板内容" },
  revision: 1,
  saves: 0,
  saved: false,
  owned: false,
  rating_count: 0,
  rating_average: 0,
  my_rating: 0,
};

function client(): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
    detail: vi.fn().mockResolvedValue(item),
    save: vi.fn().mockResolvedValue(undefined),
    storeReply: vi.fn().mockResolvedValue(undefined),
    publish: vi.fn(),
    apply: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
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

function wrapper({ children }: { children: ReactNode }) {
  return (
    <StrictMode>
      <ToastProvider>{children}</ToastProvider>
    </StrictMode>
  );
}

test("独立预览读取前五条，添加最新版本并只记录本次已添加项", async () => {
  const community = client();
  const items = Array.from({ length: 6 }, (_, index) => ({ ...item, id: `synthetic-${index}` }));
  vi.mocked(community.list).mockResolvedValue({ items, has_more: true });
  const latest = { ...items[0], revision: 2 };
  vi.mocked(community.detail).mockResolvedValue(latest);
  const { result } = renderHook(() => useCommunityReplyTemplates({ client: community }));
  await waitFor(() => expect(result.current.items).toHaveLength(5));
  expect(community.list).toHaveBeenCalledExactlyOnceWith("reply", "", "", 0);
  await act(async () => {
    await result.current.add(items[0]);
  });
  expect(community.save).toHaveBeenCalledExactlyOnceWith(items[0].id, true);
  expect(community.storeReply).toHaveBeenCalledExactlyOnceWith(latest);
  expect(result.current.added.has(items[0].id)).toBe(true);
  expect(result.current.notice).toBe("已添加「合成回复」到高情商回复键盘");
  expect(result.current.adding).toBe(null);
});

test("无键盘能力时只收藏，toast 模式不产生行内提示", async () => {
  const community = client();
  delete community.storeReply;
  const { result } = renderHook(
    () => useCommunityReplyTemplates({ client: community, inlineNotice: false }),
    { wrapper },
  );
  await waitFor(() => expect(result.current.items).toHaveLength(1));
  await act(async () => {
    await result.current.add(item);
  });
  expect(result.current.notice).toBe("");
  expect(screen.getByText("已收藏「合成回复」")).toBeTruthy();
  expect(result.current.added.has(item.id)).toBe(true);
});

test("当前添加失败保留可重试项并反馈错误", async () => {
  const community = client();
  vi.mocked(community.save).mockRejectedValueOnce({ code: "community_unavailable" });
  const { result } = renderHook(() => useCommunityReplyTemplates({ client: community }));
  await waitFor(() => expect(result.current.items).toHaveLength(1));
  await act(async () => {
    await result.current.add(item);
  });
  expect(result.current.notice).toContain("社区暂时不可用");
  expect(result.current.added.size).toBe(0);
  expect(result.current.adding).toBe(null);
  await act(async () => {
    await result.current.add(item);
  });
  expect(result.current.added.has(item.id)).toBe(true);
});

test.each(["success", "failure"])("换客户端后旧列表 %s 不覆盖当前读取", async (outcome) => {
  const oldList = deferred<CommunityResourcePage>();
  const nextList = deferred<CommunityResourcePage>();
  const initial = client();
  const replacement = client();
  vi.mocked(initial.list).mockReturnValue(oldList.promise);
  vi.mocked(replacement.list).mockReturnValue(nextList.promise);
  const { result, rerender } = renderHook(
    ({ community }) => useCommunityReplyTemplates({ client: community }),
    { initialProps: { community: initial }, wrapper },
  );
  rerender({ community: replacement });
  await act(async () => {
    if (outcome === "success") oldList.resolve({ items: [item], has_more: false });
    else oldList.reject(new Error("synthetic failure"));
  });
  expect(result.current.items).toBe(null);
  expect(result.current.failure).toBe("");
  await act(async () => {
    nextList.resolve({ items: [], has_more: false });
  });
  expect(result.current.items).toEqual([]);
});

test.each(["success", "failure"])(
  "旧添加 %s 不标记新客户端的同标识条目或解除当前忙碌",
  async (outcome) => {
    const oldSave = deferred<void>();
    const nextSave = deferred<void>();
    const initial = client();
    const replacement = client();
    vi.mocked(initial.save).mockReturnValue(oldSave.promise);
    vi.mocked(replacement.save).mockReturnValue(nextSave.promise);
    const { result, rerender } = renderHook(
      ({ community }) => useCommunityReplyTemplates({ client: community }),
      { initialProps: { community: initial }, wrapper },
    );
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    let oldAction!: Promise<void> | undefined;
    act(() => {
      oldAction = result.current.add(item);
    });
    rerender({ community: replacement });
    await waitFor(() => expect(result.current.items).toHaveLength(1));
    expect(result.current.adding).toBe(null);
    let nextAction!: Promise<void> | undefined;
    act(() => {
      nextAction = result.current.add(item);
    });
    await act(async () => {
      if (outcome === "success") oldSave.resolve();
      else oldSave.reject({ code: "community_unavailable" });
      await oldAction;
    });
    expect(result.current.adding).toBe(item.id);
    expect(result.current.added.size).toBe(0);
    expect(result.current.notice).toBe("");
    expect(screen.queryByText("已添加「合成回复」到高情商回复键盘")).toBeNull();
    await act(async () => {
      nextSave.resolve();
      await nextAction;
    });
    expect(result.current.added.has(item.id)).toBe(true);
    expect(result.current.adding).toBe(null);
    expect(screen.getByText("已添加「合成回复」到高情商回复键盘")).toBeTruthy();
  },
);

test("更换客户端清理之前的列表失败、添加反馈和已添加集合", async () => {
  const initial = client();
  const failed = client();
  vi.mocked(failed.list).mockRejectedValue(new Error("synthetic failure"));
  const nextList = deferred<CommunityResourcePage>();
  const replacement = client();
  vi.mocked(replacement.list).mockReturnValue(nextList.promise);
  const { result, rerender } = renderHook(
    ({ community }) => useCommunityReplyTemplates({ client: community }),
    { initialProps: { community: initial } },
  );
  await waitFor(() => expect(result.current.items).toHaveLength(1));
  await act(async () => {
    await result.current.add(item);
  });
  rerender({ community: failed });
  await waitFor(() => expect(result.current.failure).not.toBe(""));
  expect(result.current.added.size).toBe(0);
  expect(result.current.notice).toBe("");
  rerender({ community: replacement });
  expect(result.current.items).toBe(null);
  expect(result.current.failure).toBe("");
  await act(async () => {
    nextList.resolve({ items: [item], has_more: false });
  });
  expect(result.current.items).toEqual([item]);
});

test("公共回复模板区不依赖设置上下文，显示添加状态并转交浏览动作", async () => {
  const community = client();
  const onBrowseMore = vi.fn();
  render(<CommunityReplyTemplates client={community} onBrowseMore={onBrowseMore} />);
  expect(screen.getByText("正在读取社区回复模板…")).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: "添加回复模板 合成回复" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "添加回复模板 合成回复" }).textContent).toBe(
      "已添加",
    ),
  );
  expect(screen.getByRole("status").textContent).toBe("已添加「合成回复」到高情商回复键盘");
  fireEvent.click(screen.getByRole("button", { name: "浏览更多回复模板" }));
  expect(onBrowseMore).toHaveBeenCalledOnce();
});
