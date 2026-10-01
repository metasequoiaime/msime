// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import {
  CommunityResourcesPage,
  CommunitySkinsPage,
  type CommunityResource,
  type CommunityResourceClient,
  type CommunitySkin,
  type CommunitySkinClient,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const resource: CommunityResource = {
  id: "10000000-0000-4000-8000-000000000001",
  kind: "dictionary",
  name: "开发词包",
  description: "公开说明",
  author: "示例作者",
  content: { entries: [{ kind: "pinyin", code: "kai fa", word: "开发", weight: 100 }] },
  revision: 2,
  saves: 3,
  saved: false,
  owned: false,
  rating_count: 1,
  rating_average: 5,
  my_rating: 0,
};

const skin: CommunitySkin = {
  id: "20000000-0000-4000-8000-000000000001",
  name: "湖心夜",
  author: "示例作者",
  description: "说明",
  design: {
    background: 0xe8f0eb,
    keyBackground: 0xffffff,
    keyForeground: 0x17251d,
    accent: 0x185c47,
    actionBackground: 0x185c47,
    cornerRadius: 8,
    borderWidth: 0,
    shadow: 0,
    pattern: 0,
    monospaced: false,
  },
  downloads: 4,
  rating_count: 2,
  rating_average: 4.5,
  my_rating: 0,
  owned: false,
};

function resourceClient(): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [resource], has_more: false }),
    detail: vi.fn().mockResolvedValue(resource),
    publish: vi.fn().mockResolvedValue(undefined),
    apply: vi.fn().mockResolvedValue({ revision: 3, imported: 1, resource_revision: 2 }),
    save: vi.fn().mockResolvedValue(undefined),
    rate: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    storeReply: vi.fn().mockResolvedValue(undefined),
    removeReply: vi.fn().mockResolvedValue(undefined),
  };
}

function skinClient(): CommunitySkinClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [skin], has_more: false }),
    detail: vi.fn().mockResolvedValue(skin),
    download: vi.fn().mockResolvedValue({ id: skin.id, name: skin.name, design: skin.design }),
    publish: vi.fn().mockResolvedValue(undefined),
    rate: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    setCategory: vi.fn().mockResolvedValue(skin),
    finishTrial: vi.fn().mockResolvedValue(undefined),
  };
}

// The resource gallery shipped with class names and no stylesheet rules whatsoever -- no border, no
// padding, no grid -- from the commit that added it. Nothing failed, because unstyled markup renders
// fine. This asserts the tiles now carry styling, which is the only signal a page like this gives.
test("a resource tile is actually styled", async () => {
  render(<CommunityResourcesPage client={resourceClient()} kind="dictionary" />);

  const tile = await screen.findByRole("button", { name: /查看词库 开发词包/ });
  expect(tile.className).toContain("rounded");
  expect(tile.className).toContain("border");
  expect(tile.className).toContain("bg-card");
  // The glyph standing in for the missing artwork, and the metrics line beneath it.
  const glyph = tile.querySelector('[aria-hidden="true"]')!;
  expect(glyph.className).toContain("rounded");
});

test("the resource gallery lays its tiles out in a grid", async () => {
  render(<CommunityResourcesPage client={resourceClient()} kind="dictionary" />);

  const tile = await screen.findByRole("button", { name: /查看词库 开发词包/ });
  expect(tile.parentElement!.className).toContain("grid");
});

// The scope buttons were hidden below 560px for every page, but only the resource gallery has the
// overflow menu that was meant to replace them. On a phone the skin gallery therefore had no way to
// reach 我的作品 at all. The skin gallery keeps its two buttons at every width now.
test("the skin gallery keeps its scope switch at phone width", async () => {
  render(<CommunitySkinsPage client={skinClient()} theme="dark" />);
  await waitFor(() => expect(screen.getByRole("group", { name: "社区皮肤范围" })).toBeTruthy());

  const scope = screen.getByRole("group", { name: "社区皮肤范围" });
  expect(within(scope).getByRole("button", { name: "全部皮肤" })).toBeTruthy();
  expect(within(scope).getByRole("button", { name: "我的作品" })).toBeTruthy();
  expect(scope.className).not.toContain("hidden");
});

// The resource gallery may collapse them, because it has somewhere for them to go.
test("the resource gallery collapses its scope switch into a menu", async () => {
  render(<CommunityResourcesPage client={resourceClient()} kind="dictionary" />);
  await screen.findByRole("button", { name: /查看词库 开发词包/ });

  const scope = screen.getByRole("group", { name: "词库筛选范围", hidden: true });
  expect(scope).toBeTruthy();
  const collapsing = document.querySelector('[class*="max-tight:hidden"]');
  expect(collapsing).toBeTruthy();
});
