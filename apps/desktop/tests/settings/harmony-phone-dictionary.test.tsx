// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { CommunityResource } from "@msime/ui";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";
import { DictionarySettingsPage } from "../../../../packages/ui/src/settings/pages/dictionary-page";
import {
  SettingsFormContext,
  type SettingsFormModel,
} from "../../../../packages/ui/src/settings/settings-form-context";
import type {
  DictionaryCollection,
  DictionaryCollectionsClient,
  DictionaryCollectionsView,
} from "../../../../packages/ui/src/dictionary/dictionary-collections";

afterEach(() => {
  cleanup();
  window.history.replaceState(null, "");
});

const work: DictionaryCollection = {
  id: "work",
  name: "工作",
  kind: "pinyin",
  source: { type: "user" },
  enabled: false,
  entry_count: 3,
  pending: 2,
};
const slang: DictionaryCollection = {
  id: "slang",
  name: "网络流行语",
  kind: "pinyin",
  source: { type: "community", resource_id: "r-slang", revision: 2 },
  enabled: true,
  entry_count: 4812,
  pending: 0,
};
const formats = ["txt", "standard", "windows", "hans", "rime"];

function viewOf(collections: DictionaryCollection[]): DictionaryCollectionsView {
  return { collections, formats };
}

function resource(id: string, name: string): CommunityResource {
  return {
    id,
    kind: "dictionary",
    name,
    description: "",
    author: "小莫",
    content: { entries: [{ kind: "pinyin", word: "绝绝子", code: "jue'jue'zi", weight: 10 }] },
    revision: 1,
  } as unknown as CommunityResource;
}

function setup({ harmonyPhone = true } = {}) {
  let current = viewOf([work, slang]);
  const respond = (next: DictionaryCollection[]) => {
    current = viewOf(next);
    return Promise.resolve(current);
  };
  const collections: DictionaryCollectionsClient = {
    load: vi.fn(() => Promise.resolve(current)),
    flush: vi.fn(() => Promise.resolve(current)),
    create: vi.fn((name: string) =>
      respond([...current.collections, { ...work, id: "new", name, entry_count: 0, pending: 0 }]),
    ),
    delete: vi.fn((id: string) => respond(current.collections.filter((item) => item.id !== id))),
    setEnabled: vi.fn((id: string, enabled: boolean) =>
      respond(current.collections.map((item) => (item.id === id ? { ...item, enabled } : item))),
    ),
    addWords: vi.fn(() => Promise.resolve(current)),
    importFile: vi.fn(() => Promise.resolve(current)),
    installCommunity: vi.fn((item: CommunityResource) =>
      respond([
        ...current.collections,
        {
          ...work,
          id: `installed-${item.id}`,
          name: item.name,
          source: { type: "community", resource_id: item.id, revision: 1 },
        },
      ]),
    ),
  };
  const dictionary = {
    list: vi.fn(async () => ({
      entries: [{ kind: "pinyin", key: "shui'shan", value: "水杉", weight: 12 }],
      has_more: false,
    })),
    edit: vi.fn(async () => undefined),
    export: vi.fn(async () => ({ text: "", has_more: false })),
    count: vi.fn(async () => 123456),
  };
  const communityResources = {
    list: vi.fn(async () => ({
      items: [resource("r-slang", "网络流行语"), resource("r-game", "游戏术语")],
      has_more: false,
    })),
    detail: vi.fn(async (id: string) => resource(id, id === "r-game" ? "游戏术语" : "网络流行语")),
  };
  const setDraft = vi.fn();
  const selectPage = vi.fn();
  const model = {
    client: {
      dictionary,
      dictionaryCollections: collections,
      communityResources,
      openCloudDictionary: vi.fn(),
    },
    draft: { learning: true },
    setDraft,
    busy: false,
    page: "dictionary",
    harmonyPlatform: harmonyPhone,
    mobilePlatform: harmonyPhone,
    openPanel: vi.fn(),
    pageEntry: (id: string) => (id === "vocabulary" ? { id, title: "背单词" } : undefined),
    selectPage,
    phrases: [],
    phrasePage: { offset: 0, hasMore: false, status: "" },
    dictionaryFailures: [],
    loadPhrases: vi.fn(),
    setPhraseSearch: vi.fn(),
    phraseSearch: "",
    dictionaryKind: "quick_phrase",
    dictionaryFormat: "standard",
  } as unknown as SettingsFormModel;
  render(
    <SettingsFormContext.Provider value={model}>
      <ToastProvider>
        <DictionarySettingsPage />
      </ToastProvider>
    </SettingsFormContext.Provider>,
  );
  return { collections, dictionary, communityResources, setDraft, selectPage };
}

function groupTitles(): string[] {
  return Array.from(document.querySelectorAll("[data-group-title]")).map(
    (node) => node.textContent ?? "",
  );
}

function group(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

test("the HarmonyOS phone 词库 page lists dictionaries the way Android does, without the desktop form", async () => {
  setup();

  await screen.findByText("工作");
  expect(groupTitles()).toEqual(["已安装", "管理", "发现词库", "学习", "更多"]);
  for (const desktop of [
    "编码前缀",
    "文件格式",
    "导出当前类型",
    "导出全部",
    "上一页",
    "新增词条",
  ]) {
    expect(screen.queryByText(desktop)).toBeNull();
  }

  const installed = group("已安装");
  // `count` 数的是自己的词（含学过权重的内置词），不是内置词条总数。
  await within(installed).findByText("内置词库 · 123,456 条自己的词");
  expect(within(installed).getByText("拼音词库")).toBeTruthy();
  expect(within(installed).getByText("3 条 · 2 条待同步")).toBeTruthy();
  expect(within(installed).getByText("已停用")).toBeTruthy();
  expect(within(installed).getByText("4,812 条 · 社区")).toBeTruthy();

  expect(
    within(group("管理"))
      .getAllByRole("button")
      .map((button) => button.querySelector("[data-row-title]")?.textContent),
  ).toEqual(["新建词库", "导入词库", "导出词库", "刷新词库"]);
});

test("the list flushes queued words when it opens and on 刷新词库", async () => {
  const { collections } = setup();
  await screen.findByText("工作");
  expect(collections.flush).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByText("刷新词库"));
  await waitFor(() => expect(collections.flush).toHaveBeenCalledTimes(2));
  expect(await screen.findByText("已刷新")).toBeTruthy();
});

test("a dictionary opens in place, turns on with its switch, and back returns to the list", async () => {
  const { collections } = setup();
  fireEvent.click(await screen.findByText("工作"));

  const toggle = await screen.findByRole("switch", { name: "启用此词库" });
  expect(toggle).toHaveProperty("checked", false);
  expect(screen.getByText("3 条，还有 2 条等键盘同步")).toBeTruthy();
  expect(window.history.state?.dictionaryDetail).toBe("work");
  fireEvent.click(toggle);
  await waitFor(() => expect(collections.setEnabled).toHaveBeenCalledWith("work", true));
  expect(await screen.findByText("已启用，键盘下次启动时生效")).toBeTruthy();
  await waitFor(() =>
    expect(screen.getByRole("switch", { name: "启用此词库" })).toHaveProperty("checked", true),
  );

  // 系统返回键：历史记录离开这个详情时回到列表。
  act(() => {
    window.history.replaceState(null, "");
    window.dispatchEvent(new PopStateEvent("popstate", { state: null }));
  });
  expect(await screen.findByText("新建词库")).toBeTruthy();
});

test("a word added to a dictionary needs a word and valid pinyin", async () => {
  const { collections } = setup();
  fireEvent.click(await screen.findByText("工作"));
  fireEvent.click(await screen.findByText("添加词条"));

  const dialog = screen.getByRole("dialog", { name: "添加词条" });
  fireEvent.change(within(dialog).getByRole("textbox", { name: "词语" }), {
    target: { value: "水杉" },
  });
  fireEvent.change(within(dialog).getByRole("textbox", { name: "拼音" }), {
    target: { value: "水杉" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "添加" }));
  expect(await within(dialog).findByRole("alert")).toBeTruthy();
  expect(collections.addWords).not.toHaveBeenCalled();

  fireEvent.change(within(dialog).getByRole("textbox", { name: "拼音" }), {
    target: { value: "Shui Shan" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "添加" }));
  await waitFor(() =>
    expect(collections.addWords).toHaveBeenCalledWith("work", [
      { kind: "pinyin", key: "shui'shan", value: "水杉", weight: 10 },
    ]),
  );
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

test("deleting a dictionary asks first and returns to the list", async () => {
  const { collections } = setup();
  fireEvent.click(await screen.findByText("工作"));
  fireEvent.click(await screen.findByText("删除此词库"));
  const confirm = await screen.findByRole("alertdialog");
  fireEvent.click(within(confirm).getByRole("button", { name: "删除" }));
  await waitFor(() => expect(collections.delete).toHaveBeenCalledWith("work"));
  act(() => {
    window.dispatchEvent(new PopStateEvent("popstate", { state: null }));
  });
  expect(await screen.findByText("新建词库")).toBeTruthy();
  expect(screen.queryByText("3 条 · 2 条待同步")).toBeNull();
});

test("新建词库 checks the name, creates the dictionary and opens it", async () => {
  const { collections } = setup();
  fireEvent.click(await screen.findByText("新建词库"));
  const dialog = screen.getByRole("dialog", { name: "新建词库" });
  fireEvent.click(within(dialog).getByRole("button", { name: "创建" }));
  expect((await within(dialog).findByRole("alert")).textContent).toBe("词库名需要 1–32 个字。");
  await waitFor(() =>
    expect(within(dialog).getByRole("button", { name: "创建" })).toHaveProperty("disabled", false),
  );
  fireEvent.change(within(dialog).getByRole("textbox", { name: "词库名称" }), {
    target: { value: "游戏" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "创建" }));
  await waitFor(() => expect(collections.create).toHaveBeenCalledWith("游戏"));
  expect(await screen.findByRole("heading", { name: "游戏" })).toBeTruthy();
});

// 整个设置页在一个 <form aria-label="设置"> 里；对话框若自己再是一个表单，WebView 会把提交当成外层表单的，整页重新加载回首页（jsdom 不会这样，只能直接看结构）。回车由输入框确认。
test("the dialogs are not nested forms, and Enter in a field confirms", async () => {
  const { collections } = setup();
  fireEvent.click(await screen.findByText("新建词库"));
  const dialog = screen.getByRole("dialog", { name: "新建词库" });
  expect(dialog.tagName).not.toBe("FORM");
  expect(dialog.querySelector("form")).toBeNull();
  const field = within(dialog).getByRole("textbox", { name: "词库名称" });
  fireEvent.change(field, { target: { value: "游戏" } });
  fireEvent.keyDown(field, { key: "Enter" });
  await waitFor(() => expect(collections.create).toHaveBeenCalledWith("游戏"));
});

test("a community dictionary is added as its own dictionary, from its latest detail", async () => {
  const { collections, communityResources } = setup();
  const discover = group("发现词库");
  // 已经装过的社区词库显示「已添加」。
  expect(
    await within(discover).findByRole("button", { name: "添加词库 网络流行语" }),
  ).toHaveProperty("disabled", true);
  const add = within(discover).getByRole("button", { name: "添加词库 游戏术语" });
  expect(add.textContent).toBe("添加");
  fireEvent.click(add);
  await waitFor(() => expect(communityResources.detail).toHaveBeenCalledWith("r-game"));
  await waitFor(() => expect(collections.installCommunity).toHaveBeenCalledTimes(1));
  expect(vi.mocked(collections.installCommunity).mock.calls[0][0].id).toBe("r-game");
  await waitFor(() => expect(add.textContent).toBe("已添加"));
});

test("the built-in dictionary searches by pinyin and is always on", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  try {
    const { dictionary } = setup();
    fireEvent.click(await screen.findByText("拼音词库"));
    const toggle = await screen.findByRole("switch", { name: "启用此词库" });
    expect(toggle).toHaveProperty("checked", true);
    expect(toggle).toHaveProperty("disabled", true);
    expect(screen.getByText("内置词库始终启用，有 123,456 条自己的词")).toBeTruthy();
    expect(await screen.findByText("水杉")).toBeTruthy();
    expect(screen.getByText("shui’shan")).toBeTruthy();

    fireEvent.change(screen.getByRole("searchbox", { name: "搜索词条" }), {
      target: { value: "Shui Shan" },
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", "shui'shan");
  } finally {
    vi.useRealTimers();
  }
});

test("学习 writes the preference and 更多 opens 背单词 and 云词库", async () => {
  const { setDraft, selectPage } = setup();
  await screen.findByText("工作");
  fireEvent.click(screen.getByRole("switch", { name: "学习选词习惯" }));
  const update = setDraft.mock.calls.at(-1)?.[0] as (value: { learning: boolean }) => {
    learning: boolean;
  };
  expect(update({ learning: true }).learning).toBe(false);
  fireEvent.click(screen.getByText("背单词"));
  expect(selectPage).toHaveBeenCalledWith("vocabulary");
  expect(screen.getByText("云词库")).toBeTruthy();
});

test("other hosts keep the desktop dictionary form", () => {
  setup({ harmonyPhone: false });
  expect(screen.getByText("编码前缀")).toBeTruthy();
  expect(screen.queryByText("新建词库")).toBeNull();
});
