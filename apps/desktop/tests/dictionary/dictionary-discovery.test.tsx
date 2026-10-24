// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { DiscoverDictionaries } from "../../../../packages/ui/src/dictionary/dictionary-discovery";
import type {
  CommunityResource,
  CommunityResourceClient,
  CommunityResourcePage,
} from "../../../../packages/ui/src/community/community-resource-model";
import type { DictionaryCollectionsView } from "../../../../packages/ui/src/dictionary/dictionary-collections";

afterEach(cleanup);

function resource(id: string): CommunityResource {
  return {
    id,
    kind: "dictionary",
    name: `合成词库 ${id}`,
    description: "",
    author: "合成作者",
    content: { entries: [{ kind: "pinyin", code: "he", word: "合", weight: 10 }] },
    revision: 1,
    saves: 0,
    saved: false,
    owned: false,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}

function community(list: CommunityResourceClient["list"]): CommunityResourceClient {
  return {
    list,
    detail: vi.fn(),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("发现词库独立加载第一页，最多展示八项，安装期间与外层忙碌分别禁用按钮", async () => {
  const items = Array.from({ length: 10 }, (_, index) => resource(String(index)));
  const list = vi.fn(async () => ({ items, has_more: true }));
  const installing = deferred<DictionaryCollectionsView | undefined>();
  const onInstall = vi.fn(() => installing.promise);
  const onBrowseMore = vi.fn();
  const props = { community: community(list), view: null, busy: false, onInstall, onBrowseMore };
  const { rerender } = render(<DiscoverDictionaries {...props} />);
  expect(screen.getByText("正在读取社区词库…")).toBeTruthy();
  const add = await screen.findByRole("button", { name: "添加词库 合成词库 0" });
  expect(list).toHaveBeenCalledWith("dictionary", "", "", 0);
  expect(screen.getAllByRole("button", { name: /^添加词库/ })).toHaveLength(8);
  expect(screen.queryByText("合成词库 8")).toBeNull();
  expect(screen.getAllByText("@合成作者 · 1 条")).toHaveLength(8);
  fireEvent.click(add);
  expect(onInstall).toHaveBeenCalledWith(items[0]);
  expect(add.textContent).toBe("添加中");
  expect(add).toHaveProperty("disabled", true);
  rerender(<DiscoverDictionaries {...props} busy />);
  expect(screen.getByRole("button", { name: "添加词库 合成词库 1" })).toHaveProperty(
    "disabled",
    true,
  );
  await act(async () => installing.resolve(undefined));
  rerender(<DiscoverDictionaries {...props} />);
  expect(add).toHaveProperty("disabled", false);
  expect(add.textContent).toBe("添加");
  expect(list).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "浏览更多词库" }));
  expect(onBrowseMore).toHaveBeenCalledOnce();
});

test("已安装状态随外部集合更新，同名用户词库不冒充社区来源，并使用最新安装回调", async () => {
  const item = resource("shared");
  const first = vi.fn(async () => undefined);
  const second = vi.fn(async () => undefined);
  const props = {
    community: community(vi.fn(async () => ({ items: [item], has_more: false }))),
    busy: false,
    onBrowseMore: vi.fn(),
  };
  const collection = {
    id: "local",
    name: item.name,
    kind: "pinyin" as const,
    enabled: true,
    entry_count: 1,
    pending: 0,
  };
  const userView: DictionaryCollectionsView = {
    formats: [],
    collections: [{ ...collection, source: { type: "user" } }],
  };
  const { rerender } = render(
    <DiscoverDictionaries {...props} view={userView} onInstall={first} />,
  );
  const add = await screen.findByRole("button", { name: `添加词库 ${item.name}` });
  expect(add).toHaveProperty("disabled", false);
  rerender(<DiscoverDictionaries {...props} view={userView} onInstall={second} />);
  fireEvent.click(add);
  await waitFor(() => expect(second).toHaveBeenCalledWith(item));
  await waitFor(() => expect(add).toHaveProperty("disabled", false));
  expect(first).not.toHaveBeenCalled();
  const installed: DictionaryCollectionsView = {
    formats: [],
    collections: [
      { ...collection, source: { type: "community", resource_id: item.id, revision: 1 } },
    ],
  };
  rerender(<DiscoverDictionaries {...props} view={installed} onInstall={second} />);
  expect(add.textContent).toBe("已添加");
  expect(add).toHaveProperty("disabled", true);
});

test.each(["成功", "失败"])("更换社区客户端后，旧列表%s不能覆盖新结果", async (outcome) => {
  const old = deferred<CommunityResourcePage>();
  const props = {
    view: null,
    busy: false,
    onInstall: vi.fn(async () => undefined),
    onBrowseMore: vi.fn(),
  };
  const { rerender } = render(
    <DiscoverDictionaries {...props} community={community(() => old.promise)} />,
  );
  const current = community(vi.fn(async () => ({ items: [resource("current")], has_more: false })));
  rerender(<DiscoverDictionaries {...props} community={current} />);
  await screen.findByText("合成词库 current");
  await act(async () => {
    if (outcome === "成功") old.resolve({ items: [resource("stale")], has_more: false });
    else old.reject(new Error("合成过期失败"));
  });
  expect(screen.getByText("合成词库 current")).toBeTruthy();
  expect(screen.queryByText("合成词库 stale")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

test.each(["空列表", "失败"])("发现词库显示%s反馈，仍可浏览更多", async (outcome) => {
  const list = vi.fn(async () => {
    if (outcome === "失败") throw new Error("合成请求失败");
    return { items: [], has_more: false };
  });
  const onBrowseMore = vi.fn();
  render(
    <DiscoverDictionaries
      community={community(list)}
      view={null}
      busy={false}
      onInstall={vi.fn(async () => undefined)}
      onBrowseMore={onBrowseMore}
    />,
  );
  expect(
    await screen.findByText(
      outcome === "失败" ? "社区词库暂时读不到，请稍后再试。" : "社区里还没有词库。",
    ),
  ).toBeTruthy();
  expect(Boolean(screen.queryByRole("alert"))).toBe(outcome === "失败");
  fireEvent.click(screen.getByRole("button", { name: "浏览更多词库" }));
  expect(onBrowseMore).toHaveBeenCalledOnce();
});
