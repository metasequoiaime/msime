import { expect, test } from "vitest";
import {
  collectionFailureMessage,
  collectionFormatForFile,
  collectionImportMessage,
  collectionImportSources,
  collectionNameFromFile,
  collectionSubtitle,
  displayPinyinCode,
  normalizePinyinCode,
  validCollectionName,
  validPinyinCode,
  type DictionaryCollection,
} from "../../../../packages/ui/src/dictionary/dictionary-collections";

// 这些规则与 Android 的 `DictionaryCollectionsStore` 和 client-core 的集合校验一致，界面在提交前先挡住。

test("a collection name is 1–32 characters with no surrounding space or control characters", () => {
  expect(validCollectionName("工作")).toBe(true);
  expect(validCollectionName("字".repeat(32))).toBe(true);
  expect(validCollectionName("字".repeat(33))).toBe(false);
  expect(validCollectionName("")).toBe(false);
  expect(validCollectionName(" 工作")).toBe(false);
  expect(validCollectionName("工\u0007作")).toBe(false);
  // 按字计，不按 UTF-16 单元计。
  expect(validCollectionName("😀".repeat(32))).toBe(true);
});

test("typed pinyin becomes a code with apostrophe separators", () => {
  expect(normalizePinyinCode(" Shui Shan ")).toBe("shui'shan");
  expect(normalizePinyinCode("xi’an")).toBe("xi'an");
  expect(validPinyinCode("shui'shan")).toBe(true);
  expect(validPinyinCode("'shui")).toBe(false);
  expect(validPinyinCode("shui''shan")).toBe(false);
  expect(validPinyinCode("水杉")).toBe(false);
  expect(displayPinyinCode("shui'shan")).toBe("shui’shan");
});

test("an imported file names its collection and picks its format", () => {
  expect(collectionNameFromFile("网络流行语.txt")).toBe("网络流行语");
  expect(collectionNameFromFile("luna_pinyin.dict.yaml")).toBe("luna_pinyin");
  expect(collectionNameFromFile(".txt")).toBe("导入的词库");
  expect(Array.from(collectionNameFromFile(`${"长".repeat(40)}.txt`))).toHaveLength(32);
  expect(collectionFormatForFile("a.dict.yaml", "txt")).toBe("rime");
  expect(collectionFormatForFile("a.txt", "hans")).toBe("hans");
});

test("the import sources list only what client-core accepts, text first", () => {
  expect(
    collectionImportSources(["txt", "standard", "windows", "hans", "rime"]).map((s) => s.format),
  ).toEqual(["txt", "rime", "hans"]);
  expect(collectionImportSources(["txt"]).map((s) => s.title)).toEqual(["文本文件（.txt）"]);
});

test("the import toast says what happened to the rows", () => {
  expect(
    collectionImportMessage("工作", {
      imported: 10,
      duplicates: 2,
      failed: 1,
      truncated: true,
      swapped: false,
    }),
  ).toBe("已导入「工作」，10 条，跳过重复 2 条，1 行无法识别，词库已满");
  expect(collectionImportMessage("工作", undefined)).toBe("已导入「工作」");
});

test("client-core failure codes read as sentences, unknown ones as a generic failure", () => {
  expect(collectionFailureMessage({ code: "collections_limit" })).toBe(
    "词库数量或词条数已达上限。",
  );
  expect(collectionFailureMessage({ code: "builtin_locked" })).toBe(
    "内置词库始终启用，不能停用或改名。",
  );
  expect(collectionFailureMessage(new Error("boom"))).toBe("词库操作失败，请稍后重试。");
});

test("a collection's subtitle carries its count, community origin and pending changes", () => {
  const collection: DictionaryCollection = {
    id: "a",
    name: "网络流行语",
    kind: "pinyin",
    source: { type: "community", resource_id: "r", revision: 1 },
    enabled: true,
    entry_count: 4812,
    pending: 0,
  };
  expect(collectionSubtitle(collection)).toBe("4,812 条 · 社区");
  expect(collectionSubtitle({ ...collection, source: { type: "user" }, pending: 128 })).toBe(
    "4,812 条 · 128 条待同步",
  );
});
