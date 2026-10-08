// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import {
  dictionaryExportName,
  dictionaryExportPayload,
  dictionaryKindKeyHint,
  loadAllPersonalDictionaryEntries,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
} from "@msime/ui";

test("each dictionary kind exports under its own shipped name", () => {
  expect(dictionaryExportName("pinyin")).toBe("水杉IME-拼音用户词库.txt");
  expect(dictionaryExportName("wubi")).toBe("水杉IME-五笔用户词库.txt");
  expect(dictionaryExportName("english")).toBe("水杉IME-英文用户词库.txt");
  expect(dictionaryExportName("quick_phrase")).toBe("水杉IME-快捷短语用户词库.txt");
  // The old single generic name said nothing about which book it held.
  expect(
    new Set(
      ["pinyin", "wubi", "english", "quick_phrase"].map((k) => dictionaryExportName(k as never)),
    ).size,
  ).toBe(4);
});

test("the payload starts with a UTF-8 BOM", () => {
  // Without it Notepad and Excel on a GBK-default Windows show mojibake.
  const { body } = dictionaryExportPayload("wubi", "standard", "你好\twq\t9\n");
  expect(body.charCodeAt(0)).toBe(0xfeff);
  expect(body).toContain("你好\twq\t9");
  expect(body.endsWith("\n")).toBe(true);
});

test("single-character pinyin rows are dropped as learning artefacts", () => {
  const text = "你好\tni'hao\t9\n的\tde\t99\n世界\tshi'jie\t8\n";
  const { body, rows } = dictionaryExportPayload("pinyin", "standard", text);
  expect(rows).toBe(2);
  expect(body).toContain("你好");
  expect(body).toContain("世界");
  expect(body).not.toContain("\t de");
  expect(
    body
      .split("\n")
      .filter(Boolean)
      .some((l) => l.startsWith("的")),
  ).toBe(false);
});

test("the word column follows the format, so windows exports are not misread", () => {
  // Windows puts the code first. Reading column 0 there would measure the
  // pinyin, and "de" is two characters, so the row would wrongly survive.
  const windows = "de\t的\t99\nni'hao\t你好\t9\n";
  const { rows, body } = dictionaryExportPayload("pinyin", "windows", windows);
  expect(rows).toBe(1);
  expect(body).toContain("你好");
  expect(body).not.toContain("的");
});

test("other kinds keep their single-character rows", () => {
  // Only the pinyin book accumulates learning artefacts; a one-character quick
  // phrase or wubi entry is a real entry the user made.
  const text = "好\thao\t9\n";
  expect(dictionaryExportPayload("wubi", "standard", text).rows).toBe(1);
  expect(dictionaryExportPayload("quick_phrase", "standard", text).rows).toBe(1);
  expect(dictionaryExportPayload("english", "standard", text).rows).toBe(1);
});

test("an empty result reports nothing to export rather than downloading a blank file", () => {
  expect(dictionaryExportPayload("pinyin", "standard", "").rows).toBe(0);
  expect(dictionaryExportPayload("pinyin", "standard", "\n  \n").rows).toBe(0);
  // A pinyin book holding only single characters is also nothing to export.
  expect(dictionaryExportPayload("pinyin", "standard", "的\tde\t99\n").rows).toBe(0);
  expect(dictionaryExportPayload("pinyin", "standard", "").body).toBe("");
});

test("the complete personal dictionary export follows Apple's kind order and envelope", () => {
  const entries = [
    { kind: "english" as const, key: "hello", value: "Hello", weight: 4 },
    { kind: "quick_phrase" as const, key: "greet", value: "你好", weight: 3 },
    { kind: "pinyin" as const, key: "ni'hao", value: "你好", weight: 2 },
    { kind: "wubi98" as const, key: "wq", value: "你", weight: 5 },
    { kind: "wubi" as const, key: "wq", value: "你", weight: 1 },
  ];
  expect(personalDictionaryExportName()).toBe("水杉用户词库.txt");
  expect(personalDictionaryExportPayload(entries)).toEqual({
    rows: 5,
    body: "# 类别\t编码\t词条\t权重\n拼音\tni'hao\t你好\t2\n86 五笔\twq\t你\t1\n98 五笔\twq\t你\t5\n快捷短语\tgreet\t你好\t3\n英文\thello\tHello\t4\n",
  });
});

test("complete personal dictionary reads every kind page with an empty query", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({
      entries: [{ kind: "pinyin" as const, key: "a", value: "甲", weight: 1 }],
      has_more: true,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "pinyin" as const, key: "b", value: "乙", weight: 2 }],
      has_more: false,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "wubi" as const, key: "wq", value: "你", weight: 3 }],
      has_more: false,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "wubi98" as const, key: "wq", value: "你", weight: 6 }],
      has_more: false,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "quick_phrase" as const, key: "q", value: "快捷", weight: 4 }],
      has_more: false,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "english" as const, key: "hi", value: "Hi", weight: 5 }],
      has_more: false,
    });
  await expect(loadAllPersonalDictionaryEntries({ list })).resolves.toHaveLength(6);
  expect(list).toHaveBeenNthCalledWith(1, 0, 100, "pinyin", "");
  expect(list).toHaveBeenNthCalledWith(2, 1, 100, "pinyin", "");
  expect(list).toHaveBeenNthCalledWith(3, 0, 100, "wubi", "");
  expect(list).toHaveBeenNthCalledWith(4, 0, 100, "wubi98", "");
  expect(list).toHaveBeenNthCalledWith(5, 0, 100, "quick_phrase", "");
  expect(list).toHaveBeenNthCalledWith(6, 0, 100, "english", "");
});

test("complete personal dictionary leaves bundled rows out of the export", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({
      entries: [{ kind: "pinyin" as const, key: "a", value: "甲", weight: 1 }],
      has_more: false,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "wubi" as const, key: "wq", value: "你", weight: 3 }],
      has_more: false,
    })
    .mockResolvedValueOnce({ entries: [], has_more: false })
    .mockResolvedValueOnce({
      entries: [
        {
          kind: "quick_phrase" as const,
          key: "dh",
          value: "电话",
          weight: 1,
          source: "bundled" as const,
        },
        {
          kind: "quick_phrase" as const,
          key: "q",
          value: "快捷",
          weight: 4,
          source: "user" as const,
        },
      ],
      has_more: true,
    })
    .mockResolvedValueOnce({
      entries: [
        {
          kind: "quick_phrase" as const,
          key: "yx",
          value: "邮箱",
          weight: 1,
          source: "bundled" as const,
        },
      ],
      has_more: false,
    })
    .mockResolvedValueOnce({
      entries: [{ kind: "english" as const, key: "hi", value: "Hi", weight: 5 }],
      has_more: false,
    });
  const entries = await loadAllPersonalDictionaryEntries({ list });
  expect(entries.filter((entry) => entry.kind === "quick_phrase")).toEqual([
    { kind: "quick_phrase", key: "q", value: "快捷", weight: 4, source: "user" },
  ]);
  expect(entries).toHaveLength(4);
  expect(list).toHaveBeenNthCalledWith(5, 2, 100, "quick_phrase", "");
  expect(list).toHaveBeenNthCalledWith(6, 0, 100, "english", "");
});

test("complete personal dictionary exports when a kind has no words", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({
      entries: [{ kind: "pinyin" as const, key: "a", value: "甲", weight: 1 }],
      has_more: false,
    })
    .mockResolvedValueOnce({ entries: [], has_more: false })
    .mockResolvedValueOnce({ entries: [], has_more: false })
    .mockResolvedValueOnce({ entries: [], has_more: false })
    .mockResolvedValueOnce({ entries: [], has_more: false });
  await expect(loadAllPersonalDictionaryEntries({ list })).resolves.toEqual([
    { kind: "pinyin", key: "a", value: "甲", weight: 1 },
  ]);
  expect(list).toHaveBeenCalledTimes(5);
});

test("complete personal dictionary reports an empty export without rows", () => {
  expect(personalDictionaryExportPayload([])).toEqual({
    rows: 0,
    body: "# 类别\t编码\t词条\t权重\n",
  });
});

test("refuses a personal export larger than the bridge bound", () => {
  const entries = Array.from({ length: 9_000 }, (_, index) => ({
    kind: "quick_phrase" as const,
    key: `phrase-${index}`,
    value: "词".repeat(4_096),
    weight: 1,
  }));
  expect(() => personalDictionaryExportPayload(entries)).toThrow("dictionary_export_limit");
});

test("stops reading the complete export when its rows exceed the bridge bound", async () => {
  const entries = Array.from({ length: 9_000 }, (_, index) => ({
    kind: "pinyin" as const,
    key: `ni'hao-${index}`,
    value: "词".repeat(4_096),
    weight: 1,
  }));
  const list = vi.fn().mockResolvedValueOnce({ entries, has_more: false });
  await expect(loadAllPersonalDictionaryEntries({ list })).rejects.toThrow(
    "dictionary_export_limit",
  );
  expect(list).toHaveBeenCalledTimes(1);
});

test("dictionary editor hints follow the selected kind", () => {
  expect(dictionaryKindKeyHint("pinyin")).toContain("ni'hao");
  expect(dictionaryKindKeyHint("wubi")).toBe("1–4 个字母");
  expect(dictionaryKindKeyHint("wubi98")).toBe("1–4 个字母");
  expect(dictionaryKindKeyHint("quick_phrase")).toBe("1–32 个字母");
  expect(dictionaryKindKeyHint("english")).toBe("1–64 个字母");
});
