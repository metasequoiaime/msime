import { expect, test } from "vitest";
import { statisticsDistributionSlices } from "../../../../packages/ui/src/settings/statistics-distribution-model";

test("字符分布保留分类顺序、颜色和符号，缺失计数补零", () => {
  const result = statisticsDistributionSlices({ characters: { han: 7, unknown: 3 }, sources: {} });
  expect(result.characterSlices).toEqual([
    { id: "han", title: "汉字", count: 7, color: "#19a78d", symbol: "汉" },
    { id: "latin", title: "拉丁字母", count: 0, color: "#4c8ee8", symbol: "A" },
    { id: "otherLetter", title: "其他文字", count: 0, color: "#7772df", symbol: "文" },
    { id: "number", title: "数字", count: 0, color: "#e59b43", symbol: "123" },
    { id: "punctuation", title: "标点", count: 0, color: "#db6f9f", symbol: "，" },
    { id: "emoji", title: "表情", count: 0, color: "#a879d7", symbol: "😀" },
    { id: "symbol", title: "其他符号", count: 0, color: "#98705a", symbol: "#" },
    { id: "unknown", title: "历史未分类", count: 3, color: "#888b92", symbol: "?" },
  ]);
});

test("中文语言扇区聚合全部中文方案，其他来源保留独立模式", () => {
  const chinese = [
    "quanpin",
    "nineKey",
    "fourteenKey",
    "shuangpin",
    "ziranma",
    "microsoft",
    "shoudao",
    "smart_abc",
    "wubi",
    "cantonese",
    "zhuyin",
    "stroke",
  ];
  const sources = Object.fromEntries(chinese.map((key) => [key, 1]));
  Object.assign(sources, {
    japanese: 2,
    korean: 3,
    vietnamese: 4,
    tibetan: 5,
    english: 6,
    local: 7,
    ai: 8,
    reply: 9,
    voice: 10,
    unknown: 11,
    handwriting: 12,
  });
  const frozen = Object.freeze({ characters: Object.freeze({}), sources: Object.freeze(sources) });
  const { languageSlices, sourceSlices } = statisticsDistributionSlices(frozen);
  expect(languageSlices.map(({ id, count }) => [id, count])).toEqual([
    ["chinese", 12],
    ["japanese", 2],
    ["korean", 3],
    ["vietnamese", 4],
    ["tibetan", 5],
    ["english", 6],
    ["local", 7],
    ["ai", 8],
    ["reply", 9],
    ["voice", 10],
    ["unknown", 11],
  ]);
  expect(languageSlices.find(({ id }) => id === "tibetan")).toEqual({
    id: "tibetan",
    title: "藏文模式",
    count: 5,
    color: "#b8873a",
    symbol: "藏",
  });
  expect(sourceSlices.find(({ id }) => id === "handwriting")?.count).toBe(12);
  expect(sourceSlices.find(({ id }) => id === "fourteenKey")?.title).toBe("全拼 14 键");
  expect(sourceSlices.find(({ id }) => id === "smart_abc")?.title).toBe("智能 ABC 双拼");
  expect(sourceSlices.map(({ id }) => id)).toEqual([
    "quanpin",
    "nineKey",
    "fourteenKey",
    "shuangpin",
    "ziranma",
    "microsoft",
    "shoudao",
    "smart_abc",
    "wubi",
    "japanese",
    "korean",
    "cantonese",
    "zhuyin",
    "vietnamese",
    "tibetan",
    "stroke",
    "handwriting",
    "english",
    "local",
    "ai",
    "reply",
    "voice",
    "unknown",
  ]);
});

test("重新构造不保留上一次范围的计数或可变输出", () => {
  const first = statisticsDistributionSlices({ characters: { han: 5 }, sources: { quanpin: 5 } });
  first.characterSlices[0].count = 99;
  const next = statisticsDistributionSlices({ characters: {}, sources: {} });
  expect(
    [...next.characterSlices, ...next.sourceSlices, ...next.languageSlices].every(
      ({ count }) => count === 0,
    ),
  ).toBe(true);
});
