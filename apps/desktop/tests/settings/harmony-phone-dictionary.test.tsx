// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { CommunityResource, DictionaryClient } from "@msime/ui";
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

function viewOf(
  collections: DictionaryCollection[],
  accepted: string[] = formats,
): DictionaryCollectionsView {
  return { collections, formats: accepted };
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

function setup({ harmonyPhone = true, scel = false, importBytes = false, strict = false } = {}) {
  const accepted = scel ? [...formats, "scel"] : formats;
  let current = viewOf([work, slang], accepted);
  const respond = (next: DictionaryCollection[]) => {
    current = viewOf(next, accepted);
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
    ...(importBytes
      ? {
          importBytes: vi.fn((name: string) =>
            respond([
              ...current.collections,
              {
                ...work,
                id: "scel",
                name,
                source: { type: "import", format: "scel" },
                entry_count: 2,
                pending: 0,
              },
            ]).then((view) => ({
              ...view,
              import: { imported: 2, duplicates: 0, failed: 1, truncated: false, swapped: false },
            })),
          ),
        }
      : {}),
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
  const saveExport = vi.fn().mockResolvedValue("/synthetic/export.txt");
  const setDraft = vi.fn();
  const selectPage = vi.fn();
  const openCommunityGallery = vi.fn();
  const model = {
    client: {
      dictionary,
      saveExport,
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
    communityGallery: null,
    openCommunityGallery,
    phrases: [],
    phrasePage: { offset: 0, hasMore: false, status: "" },
    dictionaryFailures: [],
    loadPhrases: vi.fn(),
    setPhraseSearch: vi.fn(),
    phraseSearch: "",
    dictionaryKind: "quick_phrase",
    dictionaryFormat: "standard",
  } as unknown as SettingsFormModel;
  const tree = (value: SettingsFormModel) => (
    <SettingsFormContext.Provider value={value}>
      <ToastProvider>
        <DictionarySettingsPage />
      </ToastProvider>
    </SettingsFormContext.Provider>
  );
  const wrapped = (value: SettingsFormModel) =>
    strict ? <StrictMode>{tree(value)}</StrictMode> : tree(value);
  const view = render(wrapped(model));
  /** 以外壳打开或关上本页上的社区图库后的模型重画。 */
  const showGallery = (communityGallery: SettingsFormModel["communityGallery"]) =>
    view.rerender(wrapped({ ...model, communityGallery }));
  const changeDictionary = (dictionary: DictionaryClient) =>
    view.rerender(wrapped({ ...model, client: { ...model.client, dictionary } }));
  const changeCollections = (dictionaryCollections: DictionaryCollectionsClient) =>
    view.rerender(wrapped({ ...model, client: { ...model.client, dictionaryCollections } }));
  return {
    collections,
    dictionary,
    communityResources,
    saveExport,
    setDraft,
    selectPage,
    openCommunityGallery,
    showGallery,
    changeDictionary,
    changeCollections,
  };
}

function groupTitles(): string[] {
  return Array.from(document.querySelectorAll("[data-group-title]")).map(
    (node) => node.textContent ?? "",
  );
}

/** 「添加词库」→ 底部面板的「新建空词库」。 */
async function openCreateDialog() {
  fireEvent.click(await screen.findByText("添加词库"));
  fireEvent.click(await screen.findByRole("button", { name: "新建空词库" }));
}

function group(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

test("the HarmonyOS phone 词库 page lists dictionaries the way Android does, without the desktop form", async () => {
  setup();

  await screen.findByText("工作");
  expect(groupTitles()).toEqual(["已安装", "发现词库", "学习", "更多"]);
  for (const desktop of [
    "词条或编码",
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

  // 新建和导入合成「已安装」末尾的一行「添加词库」；没有单独一张操作卡片，也没有「刷新词库」（打开时已自动同步），导出在拼音词库的详情里。
  expect(within(installed).getByText("添加词库")).toBeTruthy();
  for (const gone of ["新建词库", "导入词库", "导出词库", "刷新词库"]) {
    expect(screen.queryByText(gone)).toBeNull();
  }
  // 说明在卡片下面，不在卡片里。
  const note = screen.getByText("点进词库可以启用、停用和编辑词条。已启用的词库会一起参与候选。");
  expect(group("已安装").contains(note)).toBe(false);
});

test("the list flushes queued words when it opens", async () => {
  const { collections } = setup();
  await screen.findByText("工作");
  expect(collections.flush).toHaveBeenCalledTimes(1);
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("StrictMode 重放后词库列表和计数能加载，修改完成后恢复可用", async () => {
  const { collections } = setup({ strict: true });
  expect(await screen.findByText("工作")).toBeTruthy();
  expect(await screen.findByText("内置词库 · 123,456 条自己的词")).toBeTruthy();
  fireEvent.click(screen.getByText("工作"));
  const toggle = screen.getByRole("switch", { name: "启用此词库" });
  fireEvent.click(toggle);
  await waitFor(() => expect(collections.setEnabled).toHaveBeenCalledWith("work", true));
  await waitFor(() => expect(toggle).toHaveProperty("checked", true));
  expect(toggle).toHaveProperty("disabled", false);
});

test.each(["成功", "失败"])("图库打开期间忽略旧刷新和计数的%s结果", async (outcome) => {
  const { collections, dictionary, showGallery } = setup();
  await screen.findByText("工作");
  await screen.findByText("内置词库 · 123,456 条自己的词");
  const refresh = deferred<DictionaryCollectionsView>();
  const count = deferred<number>();
  vi.mocked(collections.flush).mockReturnValueOnce(refresh.promise);
  dictionary.count.mockReturnValueOnce(count.promise);
  showGallery({ category: "dictionary", scope: "", initialMine: false });
  showGallery(null);
  showGallery({ category: "dictionary", scope: "", initialMine: false });
  await act(async () => {
    count.resolve(999);
    if (outcome === "成功") refresh.resolve(viewOf([{ ...work, name: "旧列表" }]));
    else refresh.reject(new Error("synthetic stale refresh"));
  });
  expect(screen.getByText("工作")).toBeTruthy();
  expect(screen.queryByText("旧列表")).toBeNull();
  expect(screen.getByText("内置词库 · 123,456 条自己的词")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
});

test.each(["成功", "失败"])(
  "词库客户端更换后旧修改的%s不覆盖新列表，也不解除新操作忙碌",
  async (outcome) => {
    const { collections, changeCollections } = setup();
    await screen.findByText("工作");
    const old = deferred<DictionaryCollectionsView>();
    vi.mocked(collections.setEnabled).mockReturnValueOnce(old.promise);
    fireEvent.click(screen.getByText("工作"));
    fireEvent.click(screen.getByRole("switch", { name: "启用此词库" }));
    const current = deferred<DictionaryCollectionsView>();
    const replacement: DictionaryCollectionsClient = {
      ...collections,
      flush: vi.fn().mockResolvedValue(viewOf([{ ...work, name: "新列表" }])),
      load: vi.fn().mockResolvedValue(viewOf([{ ...work, name: "新列表" }])),
      setEnabled: vi.fn().mockReturnValue(current.promise),
    };
    changeCollections(replacement);
    expect(await screen.findByRole("heading", { name: "新列表" })).toBeTruthy();
    const toggle = screen.getByRole("switch", { name: "启用此词库" });
    expect(toggle).toHaveProperty("disabled", false);
    fireEvent.click(toggle);
    expect(toggle).toHaveProperty("disabled", true);
    await act(async () => {
      if (outcome === "成功") old.resolve(viewOf([{ ...work, name: "旧列表", enabled: true }]));
      else old.reject(new Error("synthetic stale mutation"));
    });
    expect(screen.getByRole("heading", { name: "新列表" })).toBeTruthy();
    expect(toggle).toHaveProperty("disabled", true);
    expect(replacement.load).not.toHaveBeenCalled();
    expect(screen.queryByText("已启用")).toBeNull();
    await act(async () => current.resolve(viewOf([{ ...work, name: "新列表", enabled: true }])));
    expect(toggle).toHaveProperty("disabled", false);
    expect(toggle).toHaveProperty("checked", true);
  },
);

test("同一轮重复切换只发起一次修改，旧刷新不覆盖修改结果", async () => {
  const { collections, showGallery } = setup();
  await screen.findByText("工作");
  const refresh = deferred<DictionaryCollectionsView>();
  const mutation = deferred<DictionaryCollectionsView>();
  vi.mocked(collections.flush).mockReturnValueOnce(refresh.promise);
  vi.mocked(collections.setEnabled).mockReturnValueOnce(mutation.promise);
  showGallery({ category: "dictionary", scope: "", initialMine: false });
  showGallery(null);
  fireEvent.click(screen.getByText("工作"));
  const toggle = screen.getByRole("switch", { name: "启用此词库" });
  await act(async () => {
    fireEvent.click(toggle);
    fireEvent.click(toggle);
  });
  expect(collections.setEnabled).toHaveBeenCalledTimes(1);
  await act(async () => mutation.resolve(viewOf([{ ...work, enabled: true }])));
  await act(async () => refresh.resolve(viewOf([work])));
  expect(toggle).toHaveProperty("checked", true);
  expect(toggle).toHaveProperty("disabled", false);
});

test("图库打开不作废已提交的词库修改", async () => {
  const { collections, showGallery } = setup();
  const mutation = deferred<DictionaryCollectionsView>();
  vi.mocked(collections.setEnabled).mockReturnValueOnce(mutation.promise);
  fireEvent.click(await screen.findByText("工作"));
  fireEvent.click(screen.getByRole("switch", { name: "启用此词库" }));
  showGallery({ category: "dictionary", scope: "", initialMine: false });
  await act(async () => mutation.resolve(viewOf([{ ...work, enabled: true }])));
  const toggle = screen.getByRole("switch", { name: "启用此词库" });
  expect(toggle).toHaveProperty("checked", true);
  expect(toggle).toHaveProperty("disabled", false);
  expect(screen.getByText("已启用")).toBeTruthy();
});

test("添加词库 offers a new empty dictionary first, then the import sources client-core accepts", async () => {
  setup();
  fireEvent.click(await screen.findByText("添加词库"));
  const sheet = await screen.findByRole("dialog", { name: "添加词库" });
  expect(
    within(sheet)
      .getAllByRole("button")
      .map((button) => button.textContent)
      .filter((text) => text !== "取消"),
  ).toEqual(["新建空词库", "导入文本文件", "导入 Rime 词典", "导入纯汉字词表"]);
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
  expect(await screen.findByText("已启用")).toBeTruthy();
  await waitFor(() =>
    expect(screen.getByRole("switch", { name: "启用此词库" })).toHaveProperty("checked", true),
  );

  // 系统返回键：历史记录离开这个详情时回到列表。
  act(() => {
    window.history.replaceState(null, "");
    window.dispatchEvent(new PopStateEvent("popstate", { state: null }));
  });
  expect(await screen.findByText("添加词库")).toBeTruthy();
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
  expect(await screen.findByText("添加词库")).toBeTruthy();
  expect(screen.queryByText("3 条 · 2 条待同步")).toBeNull();
});

test("新建词库 checks the name, creates the dictionary and opens it", async () => {
  const { collections } = setup();
  await openCreateDialog();
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

test.each(
  ["新建词库", "添加词条"].flatMap((title) =>
    ["Enter", "Escape"].flatMap((key) =>
      [
        { name: "组词期间", event: { isComposing: true } },
        { name: "组词结束兼容事件", event: { isComposing: false, keyCode: 229 } },
      ].map((event) => ({ title, key, ...event })),
    ),
  ),
)("$title 保留$name 的 $key", async ({ title, key, event }) => {
  const { collections } = setup();
  if (title === "新建词库") {
    await openCreateDialog();
  } else {
    fireEvent.click(await screen.findByText("工作"));
    fireEvent.click(await screen.findByText("添加词条"));
  }
  const dialog = screen.getByRole("dialog", { name: title });
  const field = within(dialog).getByRole("textbox", {
    name: title === "新建词库" ? "词库名称" : "词语",
  }) as HTMLInputElement;
  fireEvent.change(field, { target: { value: "游戏" } });
  if (title === "添加词条") {
    fireEvent.change(within(dialog).getByRole("textbox", { name: "拼音" }), {
      target: { value: "you xi" },
    });
  }
  const save = title === "新建词库" ? collections.create : collections.addWords;

  const defaultAllowed = fireEvent.keyDown(field, { key, ...event });
  expect(save).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog", { name: title })).toBe(dialog);
  expect(field.value).toBe("游戏");
  expect(defaultAllowed).toBe(true);

  fireEvent.keyDown(field, { key, keyCode: key === "Enter" ? 13 : 27 });
  if (key === "Enter") await waitFor(() => expect(save).toHaveBeenCalledOnce());
  else expect(save).not.toHaveBeenCalled();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: title })).toBeNull());
});

// 整个设置页在一个 <form aria-label="设置"> 里；对话框若自己再是一个表单，WebView 会把提交当成外层表单的，整页重新加载回首页（jsdom 不会这样，只能直接看结构）。回车由输入框确认。
test("the dialogs are not nested forms, and Enter in a field confirms", async () => {
  const { collections } = setup();
  await openCreateDialog();
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

test.each(["成功", "失败"])(
  "社区词库详情等待期间更换客户端，旧详情%s不继续安装",
  async (outcome) => {
    const { collections, communityResources, changeCollections } = setup();
    const detail = deferred<CommunityResource>();
    communityResources.detail.mockReturnValueOnce(detail.promise);
    fireEvent.click(
      await within(group("发现词库")).findByRole("button", { name: "添加词库 游戏术语" }),
    );
    changeCollections({
      ...collections,
      flush: vi.fn(async () => viewOf([{ ...work, name: "新列表" }])),
    });
    await screen.findByText("新列表");
    await act(async () => {
      if (outcome === "成功") detail.resolve(resource("r-game", "旧游戏术语"));
      else detail.reject(new Error("合成旧详情失败"));
    });
    expect(collections.installCommunity).not.toHaveBeenCalled();
    expect(screen.getByText("新列表")).toBeTruthy();
    expect(screen.queryByText("合成旧详情失败")).toBeNull();
  },
);

// 「发现词库」只列前几个，最后一行经 `openCommunityGallery` 去完整的社区词库（外壳把它切到底部「社区」标签的「词库」分类）；本页上开着的词库图库（「我的」里的作品与收藏）关掉时，在里面添加的词库读进「已安装」。
test("发现词库 ends with 浏览更多词库, and closing the gallery reads the dictionaries again", async () => {
  const { collections, openCommunityGallery, showGallery } = setup();
  const discover = group("发现词库");
  fireEvent.click(await within(discover).findByRole("button", { name: "浏览更多词库" }));
  expect(openCommunityGallery).toHaveBeenCalledWith({
    category: "dictionary",
    scope: "",
    initialMine: false,
  });
  await waitFor(() => expect(collections.flush).toHaveBeenCalledTimes(1));

  showGallery({ category: "dictionary", scope: "", initialMine: false });
  showGallery(null);
  await waitFor(() => expect(collections.flush).toHaveBeenCalledTimes(2));
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

test.each(["成功", "失败"])(
  "更换词库客户端后，旧内置词库查询的%s不影响当前列表",
  async (outcome) => {
    const { dictionary, changeDictionary } = setup();
    await screen.findByText("工作");
    vi.useFakeTimers();
    try {
      let finishOld!: (value: Awaited<ReturnType<DictionaryClient["list"]>>) => void;
      let failOld!: (reason: unknown) => void;
      const oldRequest = new Promise<Awaited<ReturnType<DictionaryClient["list"]>>>(
        (resolve, reject) => {
          finishOld = resolve;
          failOld = reject;
        },
      );
      dictionary.list.mockReturnValueOnce(oldRequest);
      const newDictionary = {
        list: vi.fn(async () => ({
          entries: [{ kind: "pinyin", key: "xin", value: "新词", weight: 10 }],
          has_more: false,
        })),
        edit: vi.fn(),
      } as unknown as DictionaryClient;
      fireEvent.click(screen.getByText("拼音词库"));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(dictionary.list).toHaveBeenCalledTimes(1);

      changeDictionary(newDictionary);
      await act(async () => {
        if (outcome === "失败") failOld(new Error("synthetic stale query"));
        else
          finishOld({
            entries: [{ kind: "pinyin", key: "jiu", value: "旧词", weight: 10 }],
            has_more: false,
          });
      });
      expect(screen.queryByText("旧词")).toBeNull();
      expect(screen.queryByText("读取词条失败，请稍后重试。")).toBeNull();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(newDictionary.list).toHaveBeenCalledTimes(1);
      expect(screen.getByText("新词")).toBeTruthy();
    } finally {
      vi.useRealTimers();
    }
  },
);

test("搜索防抖期间，旧查询不能显示过期词条", async () => {
  const { dictionary } = setup();
  await screen.findByText("工作");
  vi.useFakeTimers();
  try {
    let finishOld!: (value: Awaited<ReturnType<DictionaryClient["list"]>>) => void;
    dictionary.list.mockReturnValueOnce(
      new Promise((resolve) => {
        finishOld = resolve;
      }),
    );
    fireEvent.click(screen.getByText("拼音词库"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    fireEvent.change(screen.getByRole("searchbox", { name: "搜索词条" }), {
      target: { value: "xin" },
    });
    await act(async () =>
      finishOld({
        entries: [{ kind: "pinyin", key: "jiu", value: "旧词", weight: 10 }],
        has_more: true,
      }),
    );
    expect(screen.queryByText("旧词")).toBeNull();
    expect(dictionary.list).toHaveBeenCalledTimes(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    expect(dictionary.list).toHaveBeenLastCalledWith(0, 100, "pinyin", "xin");
    expect(screen.getByText("水杉")).toBeTruthy();
  } finally {
    vi.useRealTimers();
  }
});

test("离开内置词库详情后，旧查询失败不在列表上弹出提示", async () => {
  const { dictionary } = setup();
  await screen.findByText("工作");
  vi.useFakeTimers();
  try {
    let fail!: (reason: unknown) => void;
    dictionary.list.mockReturnValueOnce(
      new Promise((_resolve, reject) => {
        fail = reject;
      }),
    );
    fireEvent.click(screen.getByText("拼音词库"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    act(() => {
      window.history.replaceState(null, "");
      window.dispatchEvent(new PopStateEvent("popstate", { state: null }));
    });
    expect(screen.getByText("工作")).toBeTruthy();
    await act(async () => fail(new Error("synthetic closed query")));
    expect(screen.queryByText("读取词条失败，请稍后重试。")).toBeNull();
  } finally {
    vi.useRealTimers();
  }
});

test("学习 writes the preference and 更多 opens 背单词 and 云词库", async () => {
  const { setDraft, selectPage } = setup();
  await screen.findByText("工作");
  fireEvent.click(screen.getByRole("switch", { name: "记忆新词" }));
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
  expect(screen.getByText("词条或编码")).toBeTruthy();
  expect(screen.queryByText("添加词库")).toBeNull();
});

test("添加词库 offers the Sogou cell dictionary after text and sends its bytes as base64", async () => {
  const { collections } = setup({ scel: true, importBytes: true });
  fireEvent.click(await screen.findByText("添加词库"));
  const sheet = await screen.findByRole("dialog", { name: "添加词库" });
  expect(
    within(sheet)
      .getAllByRole("button")
      .map((button) => button.textContent)
      .filter((text) => text !== "取消"),
  ).toEqual(["新建空词库", "导入文本文件", "导入搜狗细胞词库", "导入 Rime 词典", "导入纯汉字词表"]);
  fireEvent.click(within(sheet).getByRole("button", { name: "导入搜狗细胞词库" }));
  const input = screen.getByLabelText("选择词库文件") as HTMLInputElement;
  expect(input.accept).toBe(".scel,application/octet-stream");
  const bytes = new Uint8Array([0x40, 0x15, 0x00, 0x00, 0x44, 0x43, 0x53, 0x01, 0xff]);
  Object.defineProperty(input, "files", { value: [new File([bytes], "化学词汇.scel")] });
  fireEvent.change(input);
  await waitFor(() => expect(collections.importBytes).toHaveBeenCalled());
  expect(collections.importBytes).toHaveBeenCalledWith("化学词汇", "scel", "QBUAAERDUwH/");
  expect(collections.importFile).not.toHaveBeenCalled();
  // 细胞词库导入后提示只供本机使用、不能发布到社区。
  expect(
    await screen.findByText(
      "已导入「化学词汇」，2 条，1 行无法识别。搜狗细胞词库的整理成果属于原作者，导入的词只供你在本机使用，不能发布到社区。",
    ),
  ).toBeTruthy();
});

test("a host that cannot send bytes does not offer the Sogou cell dictionary", async () => {
  setup({ scel: true, importBytes: false });
  fireEvent.click(await screen.findByText("添加词库"));
  const sheet = await screen.findByRole("dialog", { name: "添加词库" });
  expect(within(sheet).queryByRole("button", { name: "导入搜狗细胞词库" })).toBeNull();
});

test("细胞词库导入保留客户端方法接收者", async () => {
  const { collections } = setup({ scel: true, importBytes: true });
  const importBytes = vi.fn(function (this: DictionaryCollectionsClient) {
    return Promise.resolve(viewOf([work, slang]));
  });
  collections.importBytes = importBytes;
  fireEvent.click(await screen.findByText("添加词库"));
  fireEvent.click(screen.getByRole("button", { name: "导入搜狗细胞词库" }));
  const input = screen.getByLabelText("选择词库文件");
  Object.defineProperty(input, "files", {
    value: [new File([new Uint8Array([0x40, 0x15, 0, 0])], "合成词库.scel")],
  });
  fireEvent.change(input);
  await waitFor(() => expect(importBytes).toHaveBeenCalledOnce());
  expect(importBytes.mock.contexts[0]).toBe(collections);
});

test.each(["txt", "scel"])("读取 %s 文件期间更换客户端，不向旧客户端导入", async (format) => {
  const { collections, changeCollections } = setup({ scel: true, importBytes: true });
  const reading = deferred<ArrayBuffer>();
  const file = new File(["synthetic"], `合成词库.${format}`);
  Object.defineProperty(file, "arrayBuffer", { value: () => reading.promise });
  fireEvent.click(await screen.findByText("添加词库"));
  fireEvent.click(
    screen.getByRole("button", { name: format === "txt" ? "导入文本文件" : "导入搜狗细胞词库" }),
  );
  Object.defineProperty(screen.getByLabelText("选择词库文件"), "files", { value: [file] });
  fireEvent.change(screen.getByLabelText("选择词库文件"));
  const nextCollections = { ...collections, flush: vi.fn(async () => viewOf([slang])) };
  changeCollections(nextCollections);
  await waitFor(() => expect(within(group("已安装")).queryByText("工作")).toBeNull());
  await act(async () => reading.resolve(new Uint8Array([0x40, 0x15, 0, 0]).buffer));
  expect(collections.importFile).not.toHaveBeenCalled();
  expect(collections.importBytes).not.toHaveBeenCalled();
  expect(screen.queryByText(/已导入「合成词库」/)).toBeNull();
});

test("读取文件期间更换客户端，旧读取失败不弹出提示", async () => {
  const { collections, changeCollections } = setup();
  const reading = deferred<ArrayBuffer>();
  const file = new File(["synthetic"], "合成词库.txt");
  Object.defineProperty(file, "arrayBuffer", { value: () => reading.promise });
  fireEvent.click(await screen.findByText("添加词库"));
  fireEvent.click(screen.getByRole("button", { name: "导入文本文件" }));
  Object.defineProperty(screen.getByLabelText("选择词库文件"), "files", { value: [file] });
  fireEvent.change(screen.getByLabelText("选择词库文件"));
  changeCollections({ ...collections, flush: vi.fn(async () => viewOf([slang])) });
  await act(async () => reading.reject(new Error("合成读取失败")));
  expect(screen.queryByText("合成读取失败")).toBeNull();
});

test("手机拼音导出逐页读取并过滤单字后保存", async () => {
  const { dictionary, saveExport } = setup();
  dictionary.export
    .mockResolvedValueOnce({ text: "甲\ta\t1\n合成\thecheng\t2\n", has_more: true })
    .mockResolvedValueOnce({ text: "测试\tceshi\t3\n", has_more: false });
  fireEvent.click(await screen.findByText("拼音词库"));
  fireEvent.click(await screen.findByRole("button", { name: /导出词库/ }));
  expect(await screen.findByText("已导出 2 条")).toBeTruthy();
  expect(dictionary.export.mock.calls).toEqual([
    ["pinyin", "standard", 0, 1000],
    ["pinyin", "standard", 2, 1000],
  ]);
  expect(saveExport).toHaveBeenCalledWith(
    "水杉IME-拼音用户词库.txt",
    "\ufeff合成\thecheng\t2\n测试\tceshi\t3\n",
  );
});

test("手机导出仍在过滤拼音单字后检查文件上限", async () => {
  const { dictionary, saveExport } = setup();
  dictionary.export.mockResolvedValue({
    text: "甲\t" + "a".repeat(32 * 1024 * 1024) + "\n合成\thecheng\t2\n",
    has_more: false,
  });
  fireEvent.click(await screen.findByText("拼音词库"));
  fireEvent.click(await screen.findByRole("button", { name: /导出词库/ }));
  expect(await screen.findByText("已导出 1 条")).toBeTruthy();
  expect(saveExport).toHaveBeenCalledWith("水杉IME-拼音用户词库.txt", "\ufeff合成\thecheng\t2\n");
});

test("手机导出取消保存不显示成功提示", async () => {
  const { dictionary, saveExport } = setup();
  dictionary.export.mockResolvedValue({ text: "合成\thecheng\t2\n", has_more: false });
  saveExport.mockResolvedValue(null);
  fireEvent.click(await screen.findByText("拼音词库"));
  fireEvent.click(await screen.findByRole("button", { name: /导出词库/ }));
  await waitFor(() => expect(saveExport).toHaveBeenCalledOnce());
  expect(screen.queryByText(/已导出/)).toBeNull();
});

test.each([
  ["synthetic failure", "synthetic failure"],
  ["dictionary_export_limit", "词库导出失败，请稍后重试。"],
])("手机保留宿主错误 %s 的提示", async (error, message) => {
  const { dictionary, saveExport } = setup();
  dictionary.export.mockRejectedValue(new Error(error));
  fireEvent.click(await screen.findByText("拼音词库"));
  fireEvent.click(await screen.findByRole("button", { name: /导出词库/ }));
  expect(await screen.findByText(message)).toBeTruthy();
  expect(saveExport).not.toHaveBeenCalled();
});
