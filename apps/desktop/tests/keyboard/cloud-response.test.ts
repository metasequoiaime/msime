import { describe, expect, it } from "vitest";
import {
  cloudDictionaryCatalogEntries,
  cloudDictionaryEntries,
  cloudResponseInteger,
} from "../../../../packages/ui/src/keyboard/cloud-response";

describe("cloud dictionary response validation", () => {
  it("accepts only safe integer response metadata", () => {
    expect(cloudResponseInteger(0)).toBe(true);
    expect(cloudResponseInteger(Number.MAX_SAFE_INTEGER)).toBe(true);
    expect(cloudResponseInteger(1.5)).toBe(false);
    expect(cloudResponseInteger(Number.MAX_SAFE_INTEGER + 1)).toBe(false);
    expect(cloudResponseInteger("1")).toBe(false);
  });

  it("rejects non-integer and unsafe dictionary numbers", () => {
    const entries = cloudDictionaryEntries({
      entries: [
        { id: "valid", kind: "pinyin", code: "he", word: "合成", weight: 1, revision: 2 },
        { id: "fractional-weight", kind: "pinyin", code: "he", word: "合成", weight: 1.5, revision: 2 },
        { id: "fractional-revision", kind: "pinyin", code: "he", word: "合成", weight: 1, revision: 2.5 },
        {
          id: "unsafe-weight",
          kind: "pinyin",
          code: "he",
          word: "合成",
          weight: Number.MAX_SAFE_INTEGER + 1,
          revision: 2,
        },
      ],
    });
    expect(entries.map((entry) => entry.id)).toEqual(["valid"]);

    const catalog = cloudDictionaryCatalogEntries({
      catalog_entries: [
        { kind: "pinyin", code: "he", word: "合成", weight: 1 },
        { kind: "pinyin", code: "he", word: "合成", weight: 1.25 },
        { kind: "pinyin", code: "he", word: "合成", weight: Number.MAX_SAFE_INTEGER + 1 },
      ],
    });
    expect(catalog).toHaveLength(1);
    expect(catalog[0].weight).toBe(1);
  });
});
