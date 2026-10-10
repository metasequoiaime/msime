// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  CommunityPage,
  ToastProvider,
  type CommunityResource,
  type CommunityResourceClient,
  type CommunitySkin,
  type CommunitySkinClient,
  type CustomSkinLibraryClient,
  type Preferences,
  type SavedTouchKeyboardSkin,
  type TouchKeyboardSkinDesign,
} from "@msime/ui";
import { communityResourceSubtitle } from "../../../../packages/ui/src/community/community-resources";
import { ScreenKeyboardLayoutContext } from "../../../../packages/ui/src/keyboard/screen-keyboard-preview";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function design(background: number): TouchKeyboardSkinDesign {
  return {
    background,
    keyBackground: 0xffffff,
    keyForeground: 0x17251d,
    accent: 0x185c47,
    actionBackground: 0x185c47,
    cornerRadius: 8,
    borderWidth: 0,
    shadow: 0,
    pattern: 0,
    monospaced: false,
  };
}

const onKeyboard = "10000000-0000-4000-8000-000000000001";
const inLibrary = "10000000-0000-4000-8000-000000000002";
const notTaken = "10000000-0000-4000-8000-000000000003";

function skin(id: string, name: string, background: number): CommunitySkin {
  return {
    id,
    name,
    description: `${name} 的说明`,
    author: "示例作者",
    design: design(background),
    downloads: 12345,
    rating_count: 2,
    rating_average: 4.5,
    owned: false,
    my_rating: 0,
  };
}

const skins = [
  skin(onKeyboard, "湖心夜", 0x111111),
  skin(inLibrary, "春芽", 0x222222),
  skin(notTaken, "纸白", 0x333333),
];

const basePreferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

// 素材库用大写 id 记录键盘上正在使用的设计，就像宿主可能返回 UUID 那样。
const saved: SavedTouchKeyboardSkin[] = [
  { id: onKeyboard.toUpperCase(), name: "湖心夜", design: design(0x111111) },
  { id: inLibrary, name: "春芽", design: design(0x222222) },
];

const preferences: Preferences = {
  ...basePreferences,
  global_theme: "custom",
  custom_theme: { base: "system", keyboard: design(0x111111) },
};

function skinClient(overrides: Partial<CommunitySkinClient> = {}): CommunitySkinClient {
  return {
    list: vi.fn().mockResolvedValue({ skins, has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => skins.find((item) => item.id === id)),
    download: vi.fn().mockResolvedValue({
      skin: { id: notTaken, name: "纸白", design: design(0x333333) },
      trial: { id: "trial-1", name: "纸白" },
    }),
    rate: vi.fn().mockResolvedValue(undefined),
    publish: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    setCategory: vi.fn(),
    finishTrial: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

const dictionary: CommunityResource = {
  id: "20000000-0000-4000-8000-000000000001",
  kind: "dictionary",
  name: "网络流行语",
  description: "公开说明",
  author: "水杉词库组",
  content: {
    entries: [
      { kind: "pinyin", code: "yyds", word: "永远的神", weight: 100 },
      { kind: "pinyin", code: "xswl", word: "笑死我了", weight: 90 },
    ],
  },
  revision: 2,
  saves: 3,
  saved: false,
  owned: false,
  rating_count: 1,
  rating_average: 5,
  my_rating: 0,
};

const reply: CommunityResource = {
  ...dictionary,
  id: "20000000-0000-4000-8000-000000000002",
  kind: "reply",
  name: "温和拒绝",
  description: "礼貌地说不",
  author: "小店主",
  content: { prompt: "请用温和的语气拒绝。" },
};

function resourceClient(overrides: Partial<CommunityResourceClient> = {}): CommunityResourceClient {
  return {
    list: vi.fn().mockImplementation(async (kind: string) => ({
      items: kind === "dictionary" ? [dictionary] : [reply],
      has_more: false,
    })),
    detail: vi
      .fn()
      .mockImplementation(async (id: string) => (id === dictionary.id ? dictionary : reply)),
    publish: vi.fn().mockResolvedValue(undefined),
    apply: vi.fn().mockResolvedValue({ revision: 3, imported: 2, resource_revision: 2 }),
    save: vi.fn().mockResolvedValue(undefined),
    rate: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    storeReply: vi.fn().mockResolvedValue(undefined),
    removeReply: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function library(items = saved): CustomSkinLibraryClient {
  return {
    load: vi.fn().mockResolvedValue(items),
    mutate: vi.fn(),
  };
}

function renderHarmony(props: Partial<Parameters<typeof CommunityPage>[0]> = {}) {
  return render(
    <ToastProvider>
      <CommunityPage
        theme="light"
        skins={skinClient()}
        resources={resourceClient()}
        localSkinLibrary={library()}
        preferences={preferences}
        onApplyPreferences={vi.fn()}
        onSkinApplied={vi.fn()}
        look="harmony"
        {...props}
      />
    </ToastProvider>,
  );
}

function card(name: string): HTMLElement {
  const open = screen.getByRole("button", { name: `查看皮肤 ${name}` });
  return open.parentElement!.parentElement!;
}

test("draws the 皮肤 | 词库 | 短语 pill segments and keeps the old labels elsewhere", async () => {
  renderHarmony();
  const tabs = within(screen.getByRole("tablist", { name: "社区分类" })).getAllByRole("tab");
  expect(tabs.map((tab) => tab.textContent)).toEqual(["皮肤", "词库", "短语"]);
  expect(tabs[0]?.getAttribute("aria-selected")).toBe("true");
  await screen.findByRole("button", { name: "查看皮肤 湖心夜" });
  cleanup();

  renderHarmony({ look: undefined });
  const plain = within(screen.getByRole("tablist", { name: "社区分类" })).getAllByRole("tab");
  expect(plain.map((tab) => tab.textContent)).toEqual(["皮肤", "词库", "回复模板"]);
  await screen.findByRole("button", { name: "查看皮肤 湖心夜" });
  expect(screen.queryByRole("button", { name: "获取皮肤 纸白" })).toBeNull();
});

test("the 短语 segment lists reply templates under AI 回复模板", async () => {
  renderHarmony();
  fireEvent.click(screen.getByRole("tab", { name: "短语" }));
  expect(await screen.findByRole("heading", { name: "AI 回复模板" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "查看回复模板 温和拒绝" })).toBeTruthy();
  expect(screen.getByText("@小店主")).toBeTruthy();
  expect(screen.getByText("礼貌地说不")).toBeTruthy();
});

test("a skin card shows its use count and the pill that matches the library and the keyboard", async () => {
  renderHarmony();
  await screen.findByRole("button", { name: "查看皮肤 湖心夜" });
  await waitFor(() =>
    expect(within(card("湖心夜")).getByRole("button", { name: "使用中皮肤 湖心夜" })).toBeTruthy(),
  );
  const current = within(card("湖心夜")).getByRole("button", { name: "使用中皮肤 湖心夜" });
  expect((current as HTMLButtonElement).disabled).toBe(true);
  const use = within(card("春芽")).getByRole("button", { name: "使用皮肤 春芽" });
  expect((use as HTMLButtonElement).disabled).toBe(false);
  const take = within(card("纸白")).getByRole("button", { name: "获取皮肤 纸白" });
  expect((take as HTMLButtonElement).disabled).toBe(false);
  expect(within(card("纸白")).getByText("12,345 次使用")).toBeTruthy();
  // 画廊外框：没有桌面端标题，胶囊搜索框不带提交按钮，chip 行保持低调。
  expect(screen.queryByText("换个心情，从键盘开始")).toBeNull();
  expect(screen.getByRole("textbox", { name: "搜索皮肤设计" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "搜索" })).toBeNull();
  expect(screen.getByRole("button", { name: "我的作品" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "发布设计" })).toBeTruthy();
});

test("获取 saves the design into the library without changing the keyboard", async () => {
  const client = skinClient();
  const onApplyPreferences = vi.fn();
  renderHarmony({ skins: client, onApplyPreferences });
  await screen.findByRole("button", { name: "查看皮肤 纸白" });
  fireEvent.click(within(card("纸白")).getByRole("button", { name: "获取皮肤 纸白" }));

  await waitFor(() =>
    expect(within(card("纸白")).getByRole("button", { name: "使用皮肤 纸白" })).toBeTruthy(),
  );
  expect(client.download).toHaveBeenCalledWith(notTaken, "纸白");
  // 宿主的下载会开始试用；「获取」结束试用但不保留，因此之前的皮肤保持不变。
  expect(client.finishTrial).toHaveBeenCalledWith("trial-1", false);
  expect(onApplyPreferences).not.toHaveBeenCalled();
  expect(screen.getByText("已获取「纸白」，点「使用」换上")).toBeTruthy();
  // 胶囊按钮直接执行操作，没有打开详情。
  expect(client.detail).not.toHaveBeenCalled();
});

test("获取 names the full library instead of claiming success", async () => {
  const client = skinClient({
    download: vi
      .fn()
      .mockRejectedValue(Object.assign(new Error("full"), { code: "community_skin_library_full" })),
  });
  renderHarmony({ skins: client });
  await screen.findByRole("button", { name: "查看皮肤 纸白" });
  fireEvent.click(within(card("纸白")).getByRole("button", { name: "获取皮肤 纸白" }));
  expect(await screen.findByText("最多保存 12 套皮肤，请先删除不需要的设计。")).toBeTruthy();
  expect(within(card("纸白")).getByRole("button", { name: "获取皮肤 纸白" })).toBeTruthy();
  expect(client.finishTrial).not.toHaveBeenCalled();
});

test("使用 applies the library design and reports the skin change", async () => {
  const onApplyPreferences = vi.fn();
  const onSkinApplied = vi.fn();
  renderHarmony({ onApplyPreferences, onSkinApplied, preferences: basePreferences });
  await screen.findByRole("button", { name: "查看皮肤 春芽" });
  const use = await waitFor(() =>
    within(card("春芽")).getByRole("button", { name: "使用皮肤 春芽" }),
  );
  fireEvent.click(use);

  await waitFor(() => expect(onApplyPreferences).toHaveBeenCalledTimes(1));
  const next = onApplyPreferences.mock.calls[0]?.[0] as Preferences;
  expect(next.global_theme).toBe("custom");
  expect(next.custom_theme?.keyboard).toEqual(design(0x222222));
  // 屏幕上原来的主题保留在自定义键盘之下，与皮肤页相同。
  expect(next.custom_theme?.base).toBe("system");
  expect(onSkinApplied).toHaveBeenCalledWith(inLibrary);
  expect(await screen.findByText("已换上「春芽」")).toBeTruthy();
});

test("tapping a card outside its pill still opens the detail", async () => {
  const client = skinClient();
  renderHarmony({ skins: client });
  fireEvent.click(await screen.findByRole("button", { name: "查看皮肤 春芽" }));
  await waitFor(() => expect(client.detail).toHaveBeenCalledWith(inLibrary));
  expect(await screen.findByRole("button", { name: "下载并试用" })).toBeTruthy();
});

// 手机上的社区皮肤详情画触屏键盘：设置页根部按宿主提供 `touch`，详情预览不再画 Windows 屏幕键盘的 Win、Alt、Caps Lock。
test("the skin detail on a touch host previews the touch keyboard", async () => {
  render(
    <ScreenKeyboardLayoutContext.Provider value="touch">
      <ToastProvider>
        <CommunityPage
          theme="light"
          skins={skinClient()}
          resources={resourceClient()}
          localSkinLibrary={library()}
          preferences={preferences}
          onApplyPreferences={vi.fn()}
          onSkinApplied={vi.fn()}
          look="harmony"
          mobile
        />
      </ToastProvider>
    </ScreenKeyboardLayoutContext.Provider>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "查看皮肤 春芽" }));
  await screen.findByRole("button", { name: "下载并试用" });
  const keys = Array.from(document.querySelectorAll("[data-keyboard-key]")).map((key) =>
    key.getAttribute("data-keyboard-key"),
  );
  expect(keys.length).toBeGreaterThan(0);
  expect(keys).not.toContain("Caps Lock");
  expect(keys).not.toContain("Win");
});

test("添加 imports a dictionary into this device and settles on 已添加", async () => {
  const resources = resourceClient();
  const importWords = vi.fn().mockResolvedValue({ applied: 2 });
  renderHarmony({ resources, localDictionary: { import: importWords } });
  fireEvent.click(screen.getByRole("tab", { name: "词库" }));
  expect(await screen.findByText("@水杉词库组 · 2 条")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "添加词库 网络流行语" }));

  const done = await screen.findByRole("button", { name: "已添加词库 网络流行语" });
  expect((done as HTMLButtonElement).disabled).toBe(true);
  expect(importWords).toHaveBeenCalledWith(
    "pinyin",
    "standard",
    "永远的神\tyyds\t100\n笑死我了\txswl\t90",
    expect.stringMatching(/^community-local-/),
  );
  expect(resources.apply).not.toHaveBeenCalled();
  expect(screen.getByText("已添加「网络流行语」，本机词库应用 2 个词条")).toBeTruthy();
  // 胶囊按钮直接执行操作，没有打开详情。
  expect(screen.queryByText("词条预览 · 2 条")).toBeNull();
});

test("添加 installs a dictionary as its own dictionary when the host keeps named dictionaries", async () => {
  const resources = resourceClient();
  const importWords = vi.fn().mockResolvedValue({ applied: 2 });
  const installCollection = vi.fn().mockResolvedValue(undefined);
  renderHarmony({ resources, localDictionary: { import: importWords, installCollection } });
  fireEvent.click(screen.getByRole("tab", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "添加词库 网络流行语" }));

  await screen.findByRole("button", { name: "已添加词库 网络流行语" });
  // 装的是重新读到的详情，成为一个可以在「词库」里停用或删除的词库，而不是逐条导入主词库。
  expect(installCollection).toHaveBeenCalledTimes(1);
  expect(installCollection.mock.calls[0][0].name).toBe("网络流行语");
  expect(importWords).not.toHaveBeenCalled();
  expect(screen.getByText("已添加「网络流行语」，可以在「词库」里停用或删除")).toBeTruthy();
});

test("添加 keeps a reply template for the reply keyboard", async () => {
  const resources = resourceClient();
  renderHarmony({ resources });
  fireEvent.click(screen.getByRole("tab", { name: "短语" }));
  fireEvent.click(await screen.findByRole("button", { name: "添加回复模板 温和拒绝" }));
  expect(await screen.findByRole("button", { name: "已添加回复模板 温和拒绝" })).toBeTruthy();
  expect(resources.save).toHaveBeenCalledWith(reply.id, true);
  expect(resources.storeReply).toHaveBeenCalledWith(reply);
  expect(screen.getByText("已添加「温和拒绝」到高情商回复键盘")).toBeTruthy();
});

test("the row subtitle writes only what the item carries", () => {
  expect(communityResourceSubtitle(dictionary)).toBe("@水杉词库组 · 2 条");
  expect(communityResourceSubtitle({ ...dictionary, author: "" })).toBe("2 条");
  expect(communityResourceSubtitle({ ...dictionary, content: {} })).toBe("@水杉词库组");
  expect(communityResourceSubtitle(reply)).toBe("@小店主");
  expect(communityResourceSubtitle({ ...dictionary, owned: true, moderation: "removed" })).toBe(
    "@水杉词库组 · 2 条 · 已下架",
  );
});
