// @vitest-environment jsdom
// 主题页的外部皮肤列表在目录变化后自己重扫：社区装入皮肤、页面重新显示、窗口重新获得焦点，都不必再点「刷新皮肤」。
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  within,
} from "@testing-library/react";
import {
  SettingsPage,
  type CandidateSkinCommunityClient,
  type CommunityCandidateSkin,
  type SkinCatalog,
  type Snapshot,
} from "@msime/ui";
import { useSkinCatalog } from "../../../../packages/ui/src/skin/external-skins";
import { notifySkinCatalogChanged } from "../../../../packages/ui/src/skin/skin-catalog-changes";
import { offeredThemeCatalog } from "../../../../packages/ui/src/theme/global-theme";

// 这里渲染的设置页不带宿主信息，轮播不列只在 iOS 提供的「原生」。
const themeCatalog = offeredThemeCatalog(undefined);

const renderSkinPreview = vi.hoisted(() => vi.fn());
vi.mock("../../../../packages/ui/src/skin/skin-preview-render", () => ({
  renderSkinPreview,
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    candidate_layout: "horizontal",
    learning: true,
    chinese_punctuation: true,
  },
};

function catalog(ids: string[]): SkinCatalog {
  return {
    directory: "/synthetic/skins",
    issues: [],
    packages: ids.map((id) => ({
      id,
      name: `本地 ${id}`,
      version: "1",
      base: "system",
      author: null,
      description: null,
      layouts: ["horizontal", "vertical"],
      themes: ["dark", "light"],
      minWidthDip: 0,
      decorationTopDip: 0,
      decorationWidthDip: 0,
      toolbarStylesheet: null,
      preview: null,
      candidate: { dark: {}, light: {} },
    })),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const communitySkin = {
  id: "10000000-0000-4000-8000-000000000001",
  package_id: "ink-wash",
  name: "水墨",
  description: "水墨 的说明",
  author: "示例作者",
  version: "1.0.0",
  license: { code: "MIT", assets: "CC-BY-4.0", source: "" },
  size: 1024,
  file_count: 1,
  downloads: 7,
  rating_count: 2,
  rating_average: 4.5,
  owned: false,
  my_rating: 0,
  created_at: "2026-09-30T00:00:00Z",
  visibility: "public",
  updated_at: "2026-09-30T00:00:00Z",
} satisfies CommunityCandidateSkin;

function communityClient(installed: () => void): CandidateSkinCommunityClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [communitySkin], has_more: false }),
    detail: vi.fn().mockResolvedValue(communitySkin),
    preview: vi.fn().mockResolvedValue({ dataUrl: "data:image/png;base64,iVBORw0KGgo=" }),
    install: vi.fn().mockImplementation(async () => {
      installed();
      return catalog(["ink-wash"]);
    }),
    packPreview: vi.fn(),
    addPreview: vi.fn(),
    addLicense: vi.fn(),
    publish: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
    setVisibility: vi.fn(),
    setCategory: vi.fn(),
    sync: vi.fn().mockResolvedValue({
      uploaded: [],
      downloaded: [],
      deleted_local: [],
      deleted_cloud: [],
      skipped: [],
      stopped: null,
    }),
  };
}

test("installing from the community gallery lists the skin on 我的皮肤 without a manual refresh", async () => {
  let directory = catalog([]);
  const scan = vi.fn(async () => directory);
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
        scanSkinCatalog: scan,
        communityCandidateSkins: communityClient(() => {
          directory = catalog(["ink-wash"]);
        }),
      }}
    />,
  );
  await settingsFormReady();
  const tabs = screen.getByRole("tablist", { name: "皮肤来源" });
  fireEvent.click(within(tabs).getByRole("tab", { name: "社区皮肤" }));
  fireEvent.click(await screen.findByRole("button", { name: "查看候选窗口皮肤 水墨" }));
  await screen.findByRole("heading", { name: "水墨" });
  const scansBeforeInstall = scan.mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "一键安装" }));
  expect(await screen.findByText("已安装到外部皮肤。")).toBeTruthy();
  await settle();
  // 安装前图库查一次是否会覆盖同名皮肤；装入后的通知让隐藏着的主题页再扫一次。
  expect(scan.mock.calls.length).toBe(scansBeforeInstall + 2);
  fireEvent.click(within(tabs).getByRole("tab", { name: "我的皮肤" }));
  expect(await screen.findByRole("article", { name: "本地 ink-wash" })).toBeTruthy();
  expect(screen.queryByText("没有发现外部皮肤。")).toBeNull();
});

test("the catalog is scanned again when its page is shown again, not while it stays hidden", async () => {
  const scan = vi.fn(async () => catalog(["a"]));
  const { result, rerender } = renderHook(
    ({ active }: { active: boolean }) => useSkinCatalog(scan, undefined, false, active),
    { initialProps: { active: true } },
  );
  await settle();
  expect(scan).toHaveBeenCalledTimes(1);
  expect(result.current.catalog?.packages.map((skin) => skin.id)).toEqual(["a"]);
  rerender({ active: false });
  await settle();
  expect(scan).toHaveBeenCalledTimes(1);
  rerender({ active: true });
  await settle();
  expect(scan).toHaveBeenCalledTimes(2);
  // 再次渲染而显示状态没变，不算一次切换。
  rerender({ active: true });
  await settle();
  expect(scan).toHaveBeenCalledTimes(2);
});

test("window focus or returning to the foreground rescans only while the page is shown", async () => {
  const scan = vi.fn(async () => catalog(["a"]));
  const { rerender } = renderHook(
    ({ active }: { active: boolean }) => useSkinCatalog(scan, undefined, false, active),
    { initialProps: { active: true } },
  );
  await settle();
  expect(scan).toHaveBeenCalledTimes(1);
  act(() => void window.dispatchEvent(new Event("focus")));
  await settle();
  expect(scan).toHaveBeenCalledTimes(2);
  act(() => void document.dispatchEvent(new Event("visibilitychange")));
  await settle();
  expect(scan).toHaveBeenCalledTimes(3);
  rerender({ active: false });
  act(() => void window.dispatchEvent(new Event("focus")));
  act(() => void document.dispatchEvent(new Event("visibilitychange")));
  await settle();
  expect(scan).toHaveBeenCalledTimes(3);
});

test("scans never overlap: focus during a scan is dropped, a change notice is scanned once afterwards", async () => {
  let running = 0;
  let overlapped = false;
  const answers: ReturnType<typeof deferred<SkinCatalog>>[] = [];
  const scan = vi.fn(() => {
    running++;
    if (running > 1) overlapped = true;
    const answer = deferred<SkinCatalog>();
    answers.push(answer);
    return answer.promise.finally(() => running--);
  });
  renderHook(() => useSkinCatalog(scan, undefined, false, true));
  expect(scan).toHaveBeenCalledTimes(1);
  // 焦点和前台切换通常一起到达，碰上进行中的扫描就放弃。
  act(() => void window.dispatchEvent(new Event("focus")));
  act(() => void document.dispatchEvent(new Event("visibilitychange")));
  // 进行中的扫描可能没读到刚装入的皮肤，几次通知合并成结束后的一次补扫。
  act(() => notifySkinCatalogChanged());
  act(() => notifySkinCatalogChanged());
  expect(scan).toHaveBeenCalledTimes(1);
  await act(async () => answers[0].resolve(catalog([])));
  expect(scan).toHaveBeenCalledTimes(2);
  await act(async () => answers[1].resolve(catalog(["ink-wash"])));
  await settle();
  expect(scan).toHaveBeenCalledTimes(2);
  expect(overlapped).toBe(false);
});

test("an automatic rescan that finds nothing new keeps the catalog and does not reload images", async () => {
  const scan = vi.fn(async () => catalog(["a"]));
  const { result } = renderHook(() => useSkinCatalog(scan, undefined, false, true));
  await settle();
  const listed = result.current.catalog;
  const revision = result.current.revision;
  act(() => void window.dispatchEvent(new Event("focus")));
  await settle();
  expect(scan).toHaveBeenCalledTimes(2);
  expect(result.current.catalog).toBe(listed);
  expect(result.current.revision).toBe(revision);
  // 「刷新皮肤」照旧重读图片，留作清单没变而图片变了时的手动兜底。
  await act(async () => result.current.refresh());
  expect(result.current.revision).toBe(revision + 1);
});

test("a rescan that adds a skin keeps the carousel on the card being viewed", async () => {
  let directory = catalog(["sample"]);
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        scanSkinCatalog: vi.fn(async () => directory),
      }}
    />,
  );
  await screen.findByRole("article", { name: "本地 sample" });
  const carousel = screen.getByRole("region", { name: "主题列表" });
  fireEvent.click(within(carousel).getByRole("button", { name: "查看本地 sample" }));
  const current = () =>
    within(carousel)
      .getAllByRole("button")
      .find((button) => button.getAttribute("aria-current"))
      ?.getAttribute("aria-label");
  expect(current()).toBe("查看本地 sample");
  // 新皮肤排在正在看的那张前面，序号变了，看的仍是同一张。
  directory = catalog(["added", "sample"]);
  act(() => notifySkinCatalogChanged());
  await screen.findByRole("article", { name: "本地 added" });
  expect(current()).toBe("查看本地 sample");
  const total = themeCatalog.length + 2;
  expect(within(carousel).getByText(`${total} / ${total}`)).toBeTruthy();
});
