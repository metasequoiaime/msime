// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  CommunityHomePage,
  CommunityResourcesPage,
  type CommunityResource,
  type CommunityResourceClient,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const base = (
  kind: "dictionary" | "reply",
  id = "10000000-0000-4000-8000-000000000001",
): CommunityResource => ({
  id,
  kind,
  name: kind === "dictionary" ? "开发词包" : "礼貌回复",
  description: "公开说明",
  author: "示例作者",
  content:
    kind === "dictionary"
      ? { entries: [{ kind: "pinyin", code: "kai fa", word: "开发", weight: 100 }] }
      : { prompt: "请简洁、礼貌地回复。" },
  revision: 2,
  saves: 3,
  saved: false,
  owned: false,
  rating_count: 1,
  rating_average: 5,
  my_rating: 0,
});

function client(overrides: Partial<CommunityResourceClient> = {}): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [], has_more: false }),
    detail: vi.fn().mockImplementation(async (id) => base("dictionary", id)),
    publish: vi.fn().mockResolvedValue(undefined),
    apply: vi.fn().mockResolvedValue({ revision: 3, imported: 1, resource_revision: 2 }),
    save: vi.fn().mockResolvedValue(undefined),
    rate: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    storeReply: vi.fn().mockResolvedValue(undefined),
    removeReply: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

test("loads a resource kind with exact search and scope, then removes duplicate pages", async () => {
  const first = base("dictionary");
  const second = base("dictionary", "10000000-0000-4000-8000-000000000002");
  const list = vi
    .fn()
    .mockResolvedValueOnce({ items: [first], has_more: true })
    .mockResolvedValueOnce({ items: [first], has_more: true })
    .mockResolvedValueOnce({ items: [first, second], has_more: false })
    .mockResolvedValue({ items: [], has_more: false });
  const view = render(<CommunityResourcesPage client={client({ list })} kind="dictionary" />);
  await waitFor(() => expect(list).toHaveBeenCalledWith("dictionary", "", "", 0));
  fireEvent.change(screen.getByRole("textbox", { name: "搜索词库" }), {
    target: { value: " C++ 词 " },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith("dictionary", "", " C++ 词 ", 0));
  fireEvent.click(await screen.findByRole("button", { name: "加载更多" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith("dictionary", "", " C++ 词 ", 1));
  expect(screen.getAllByRole("button", { name: /查看词库/ })).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "收藏" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith("dictionary", "saved", " C++ 词 ", 0));
  view.unmount();
});

test("community home opens the requested resource collection", async () => {
  const resources = client();
  const skins = {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    finishTrial: vi.fn(),
  };
  render(
    <CommunityHomePage
      skins={skins}
      resources={resources}
      theme="light"
      initialCategory="reply"
      initialScope="saved"
    />,
  );
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("reply", "saved", "", 0));
  expect(screen.getByRole("tab", { name: "回复" }).getAttribute("aria-selected")).toBe("true");
});

test("resource scope also has a compact filter menu for mobile layouts", async () => {
  const list = vi.fn().mockResolvedValue({ items: [], has_more: false });
  render(<CommunityResourcesPage client={client({ list })} kind="reply" initialScope="saved" />);
  await waitFor(() => expect(list).toHaveBeenCalledWith("reply", "saved", "", 0));
  const filter = screen.getByRole("group", { name: "回复筛选范围" });
  expect(filter.querySelector("button[aria-pressed='true']")?.textContent).toBe("收藏");
  fireEvent.click(screen.getByRole("button", { name: "筛选范围：我的作品" }));
  await waitFor(() => expect(list).toHaveBeenLastCalledWith("reply", "mine", "", 0));
});

test("dictionary details apply the displayed revision and refresh saved state", async () => {
  const item = base("dictionary");
  const detail = vi.fn().mockResolvedValue(item);
  const apply = vi.fn().mockResolvedValue({ revision: 3, imported: 1, resource_revision: 2 });
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <CommunityResourcesPage
      client={client({
        list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
        detail,
        apply,
        save,
      })}
      kind="dictionary"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 开发词包" }));
  fireEvent.click(await screen.findByRole("button", { name: "导入这版词库到云端" }));
  await waitFor(() => expect(apply).toHaveBeenCalledWith(item.id, 2));
  expect(await screen.findByText("已导入云端词库，新增或更新 1 个词条。")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "收藏，关注后续更新" }));
  await waitFor(() => expect(save).toHaveBeenCalledWith(item.id, true));
});

test("a resource action from a replaced client cannot overwrite the current detail", async () => {
  const item = base("dictionary");
  const replacement = { ...item, name: "新客户端词库", saved: true };
  let finishSave!: () => void;
  let finishOldDetail!: () => void;
  const stale = { ...item, saved: false };
  const oldClient = client({
    list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
    detail: vi
      .fn()
      .mockResolvedValueOnce(item)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finishOldDetail = () => resolve(stale);
          }),
      ),
    save: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishSave = resolve;
        }),
    ),
  });
  const nextClient = client({
    list: vi.fn().mockResolvedValue({ items: [replacement], has_more: false }),
    detail: vi.fn().mockResolvedValue(replacement),
  });
  const view = render(<CommunityResourcesPage client={oldClient} kind="dictionary" />);
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 开发词包" }));
  fireEvent.click(await screen.findByRole("button", { name: "收藏，关注后续更新" }));

  finishSave();
  await waitFor(() => expect(oldClient.detail).toHaveBeenCalledTimes(2));
  view.rerender(<CommunityResourcesPage client={nextClient} kind="dictionary" />);
  expect(await screen.findByRole("button", { name: "取消收藏" })).not.toBeNull();

  finishOldDetail();
  await Promise.resolve();
  expect(screen.getByRole("button", { name: "取消收藏" })).not.toBeNull();
});

test("a publish response from a replaced resource client cannot close the editor", async () => {
  let finishPublish!: () => void;
  const oldClient = client({
    publish: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishPublish = resolve;
        }),
    ),
  });
  const nextClient = client();
  const view = render(<CommunityResourcesPage client={oldClient} kind="reply" />);
  fireEvent.click(await screen.findByRole("button", { name: "发布作品" }));
  fireEvent.change(screen.getByRole("textbox", { name: "社区作品名称" }), {
    target: { value: "新的语气" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "社区回复提示词" }), {
    target: { value: "请保持简洁" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));

  view.rerender(<CommunityResourcesPage client={nextClient} kind="reply" />);
  expect(screen.getByRole("dialog", { name: "发布回复" })).not.toBeNull();
  finishPublish();
  await Promise.resolve();
  expect(screen.getByRole("dialog", { name: "发布回复" })).not.toBeNull();
});

test("mobile resource details join the WebView history stack and system back restores the list", async () => {
  window.history.replaceState({ msimeSettings: true, page: "community" }, "");
  const item = base("dictionary", "10000000-0000-4000-8000-000000000061");
  render(
    <CommunityResourcesPage
      client={client({
        list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
        detail: vi.fn().mockResolvedValue(item),
      })}
      kind="dictionary"
      mobile
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 开发词包" }));
  expect(window.history.state.communityDetail).toEqual({ kind: "dictionary", id: item.id });
  expect(await screen.findByRole("button", { name: "← 社区" })).not.toBeNull();
  const state = { msimeSettings: true, page: "community" };
  window.history.replaceState(state, "");
  window.dispatchEvent(new PopStateEvent("popstate", { state }));
  expect(await screen.findByRole("button", { name: "查看词库 开发词包" })).not.toBeNull();
});

test("dictionary details can import the preview into the local dictionary without cloud mutation", async () => {
  const item = base("dictionary");
  const localImport = vi.fn().mockResolvedValue({ applied: 1 });
  const apply = vi.fn();
  render(
    <CommunityResourcesPage
      client={client({
        list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
        detail: vi.fn().mockResolvedValue(item),
        apply,
      })}
      kind="dictionary"
      localDictionary={{ import: localImport }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 开发词包" }));
  fireEvent.click(await screen.findByRole("button", { name: "导入这版词库到本机" }));
  await waitFor(() =>
    expect(localImport).toHaveBeenCalledWith(
      "pinyin",
      "standard",
      "开发\tkai fa\t100",
      expect.stringMatching(/^community-local-/),
    ),
  );
  expect(apply).not.toHaveBeenCalled();
});

test("dictionary local import preserves each entry kind", async () => {
  const item = {
    ...base("dictionary"),
    content: {
      entries: [
        { kind: "pinyin" as const, code: "kai fa", word: "开发", weight: 100 },
        { kind: "quick" as const, code: "smile", word: "微笑", weight: 90 },
      ],
    },
  };
  const localImport = vi.fn().mockResolvedValue({ applied: 1 });
  render(
    <CommunityResourcesPage
      client={client({
        list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
        detail: vi.fn().mockResolvedValue(item),
      })}
      kind="dictionary"
      localDictionary={{ import: localImport }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 开发词包" }));
  fireEvent.click(await screen.findByRole("button", { name: "导入这版词库到本机" }));
  await waitFor(() => expect(localImport).toHaveBeenCalledTimes(2));
  expect(localImport).toHaveBeenCalledWith(
    "pinyin",
    "standard",
    "开发\tkai fa\t100",
    expect.stringMatching(/^community-local-/),
  );
  expect(localImport).toHaveBeenCalledWith(
    "quick_phrase",
    "standard",
    "微笑\tsmile\t90",
    expect.stringMatching(/^community-local-/),
  );
});

test("reply details store an explicit local copy and never hide the prompt", async () => {
  const item = base("reply");
  const storeReply = vi.fn().mockResolvedValue(undefined);
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <CommunityResourcesPage
      client={client({
        list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
        detail: vi.fn().mockResolvedValue(item),
        storeReply,
        save,
      })}
      kind="reply"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看回复 礼貌回复" }));
  expect(screen.getByText("请简洁、礼貌地回复。")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "添加到回复键盘" }));
  await waitFor(() => expect(storeReply).toHaveBeenCalledWith(item));
  expect(await screen.findByText("已添加到回复键盘；只有点按生成时才会发送文字。")).not.toBeNull();
});

test("publishing a reply requires explicit rights confirmation", async () => {
  const publish = vi.fn().mockResolvedValue(undefined);
  render(<CommunityResourcesPage client={client({ publish })} kind="reply" />);
  fireEvent.click(await screen.findByRole("button", { name: "发布作品" }));
  fireEvent.change(screen.getByRole("textbox", { name: "社区作品名称" }), {
    target: { value: "我的语气" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "社区回复提示词" }), {
    target: { value: "请礼貌回复" },
  });
  const submit = screen.getByRole("button", { name: "公开发布" }) as HTMLButtonElement;
  expect(submit.disabled).toBe(false);
  fireEvent.click(submit);
  expect(publish).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(submit);
  await waitFor(() =>
    expect(publish).toHaveBeenCalledWith(
      expect.stringMatching(/^[0-9a-f-]{36}$/),
      "reply",
      "我的语气",
      "",
      { prompt: "请礼貌回复" },
      0,
    ),
  );
});

test("resource editor keeps a new entry after removing an existing entry in one batch", async () => {
  const item = {
    ...base("dictionary"),
    owned: true,
    content: {
      entries: [{ kind: "pinyin" as const, code: "jiu", word: "旧词", weight: 100 }],
    },
  };
  render(
    <CommunityResourcesPage
      client={client({
        list: vi.fn().mockResolvedValue({ items: [item], has_more: false }),
        detail: vi.fn().mockResolvedValue(item),
      })}
      kind="dictionary"
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看词库 开发词包" }));
  fireEvent.click(await screen.findByRole("button", { name: "编辑并发布新版本" }));
  fireEvent.change(screen.getByRole("textbox", { name: "社区词条编码" }), {
    target: { value: "xin" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "社区词条文字" }), {
    target: { value: "新词" },
  });

  act(() => {
    fireEvent.click(screen.getByRole("button", { name: "移除" }));
    fireEvent.click(screen.getByRole("button", { name: "添加词条" }));
  });

  const dialog = screen.getByRole("dialog", { name: "更新社区作品" });
  expect(within(dialog).getByLabelText("待发布词条 1/128")).not.toBeNull();
  expect(within(dialog).getByText(/新词/)).not.toBeNull();
  expect(within(dialog).queryByText(/旧词/)).toBeNull();
});
