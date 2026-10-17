import { expect, test, vi } from "vitest";
import { createCommunityLocalDictionary, type CommunityResource } from "@msime/ui";
import type { DictionaryCollectionsClient } from "@msime/ui";

const resource: CommunityResource = {
  id: "resource-1",
  kind: "dictionary",
  name: "合成词库",
  description: "测试",
  author: "test-author",
  content: { entries: [] },
  revision: 2,
  saves: 0,
  saved: false,
  owned: false,
  rating_count: 0,
  rating_average: 0,
  my_rating: 0,
};

test("没有命名词库客户端时保留原本机词库客户端", () => {
  const dictionary = { import: vi.fn() };
  expect(createCommunityLocalDictionary(dictionary, undefined)).toBe(dictionary);
});

test("命名词库客户端接管社区安装并转换第三方词条核对格式", async () => {
  const installCommunity = vi.fn(async () => ({ collections: [], formats: [] }));
  const thirdPartyMatches = vi.fn(async () => ({
    collections: [],
    formats: [],
    third_party_matches: [1],
  }));
  const collections = {
    installCommunity,
    thirdPartyMatches,
  } as unknown as DictionaryCollectionsClient;
  const dictionary = { import: vi.fn() };
  const local = createCommunityLocalDictionary(dictionary, collections);

  await local?.installCollection?.(resource);
  expect(installCommunity).toHaveBeenCalledExactlyOnceWith(resource);

  const matches = await local?.thirdPartyMatches?.([
    { kind: "quick", code: "ab", word: "甲", weight: 3 },
    { kind: "pinyin", code: "jia", word: "家", weight: 2 },
  ]);
  expect(thirdPartyMatches).toHaveBeenCalledExactlyOnceWith([
    { kind: "quickPhrase", key: "ab", value: "甲", weight: 3 },
    { kind: "pinyin", key: "jia", value: "家", weight: 2 },
  ]);
  expect(matches).toEqual([1]);
});
