import { describe, expect, it } from "vitest";
import {
  DICTIONARY_PAGE_SIZE,
  MAX_DICTIONARY_FILE_BYTES,
  decodeDictionaryBytes,
  dictionaryPageStatus,
  parsePersonalDictionaryImport,
  personalDictionaryExample,
  readDictionaryFile,
} from "../../../../packages/ui/src/dictionary/dictionary-file";

const utf8 = (text: string) => new TextEncoder().encode(text);

describe("dictionary file decoding", () => {
  it("reads plain UTF-8 without a marker", () => {
    expect(decodeDictionaryBytes(utf8("nihao\t你好\t100000"))).toBe("nihao\t你好\t100000");
  });

  it("drops the UTF-8 byte order mark instead of leaking it into the first key", () => {
    const bytes = new Uint8Array([0xef, 0xbb, 0xbf, ...utf8("ni\t你")]);
    expect(decodeDictionaryBytes(bytes)).toBe("ni\t你");
  });

  it("reads the UTF-16 files Windows dictionary tools still write", () => {
    const little = new Uint8Array([0xff, 0xfe, 0x60, 0x4f, 0x7d, 0x59]);
    expect(decodeDictionaryBytes(little)).toBe("你好");
    const big = new Uint8Array([0xfe, 0xff, 0x4f, 0x60, 0x59, 0x7d]);
    expect(decodeDictionaryBytes(big)).toBe("你好");
  });

  it("falls back to GB18030 rather than replacement characters", () => {
    // "你好" in GB18030, which is not valid UTF-8.
    const bytes = new Uint8Array([0xc4, 0xe3, 0xba, 0xc3]);
    expect(decodeDictionaryBytes(bytes)).toBe("你好");
  });
});

describe("dictionary file size bound", () => {
  it("reads a 2 MB file and refuses a 33 MB one before reading it", async () => {
    expect(MAX_DICTIONARY_FILE_BYTES).toBe(32 * 1024 * 1024);
    const text = "词条\tcitiao\t100\n".repeat(120_000);
    const accepted = new File([text], "dict.txt");
    expect(accepted.size).toBeGreaterThan(2 * 1024 * 1024);
    await expect(readDictionaryFile(accepted)).resolves.toBe(text);
    const refused = new File([new Uint8Array(33 * 1024 * 1024)], "dict.txt");
    await expect(readDictionaryFile(refused)).rejects.toThrow(
      "文件不能超过 32 MB，请拆分后分别导入。",
    );
  });

  it("holds a caller to its own bound and names it", async () => {
    const file = new File([new Uint8Array(65_537)], "dict.txt");
    await expect(readDictionaryFile(file, 65_536)).rejects.toThrow("文件不能超过 64 KB");
    await expect(readDictionaryFile(file, 1_048_576)).resolves.toHaveLength(65_537);
    const large = new File([new Uint8Array(1_048_577)], "dict.txt");
    await expect(readDictionaryFile(large, 1_048_576)).rejects.toThrow("文件不能超过 1 MB");
  });
});

describe("dictionary pagination status", () => {
  it("reports the range and whether more results follow", () => {
    expect(dictionaryPageStatus(0, DICTIONARY_PAGE_SIZE, true)).toBe("第 1–100 条，后面还有结果");
    expect(dictionaryPageStatus(100, 20, false)).toBe("第 101–120 条");
  });

  it("says so when a page came back empty", () => {
    expect(dictionaryPageStatus(0, 0, false)).toBe("未找到匹配词条");
    expect(dictionaryPageStatus(300, 0, true)).toBe("没有更多结果");
  });
});

describe("personal dictionary JSON", () => {
  it("normalizes Engine codes and accepts the multiline example", () => {
    const entries = parsePersonalDictionaryImport(personalDictionaryExample);
    expect(entries[0].key).toBe("ni'hao");
    expect(entries[3]).toMatchObject({
      kind: "quickPhrase",
      key: "greeting",
      value: "你好！\n很高兴认识你。",
    });
  });

  it("allows tabs only in quick phrases and enforces the Engine byte bound", () => {
    const envelope = (kind: string, value: string) =>
      JSON.stringify({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [{ kind, key: "fixture", value, weight: 100000 }],
      });
    expect(parsePersonalDictionaryImport(envelope("quickPhrase", "first\tsecond"))).toHaveLength(1);
    expect(
      parsePersonalDictionaryImport(envelope("quickPhrase", "你好\n".repeat(300))),
    ).toHaveLength(1);
    expect(() => parsePersonalDictionaryImport(envelope("english", "first\tsecond"))).toThrow();
    expect(() =>
      parsePersonalDictionaryImport(envelope("quickPhrase", "字".repeat(1_366))),
    ).toThrow();
  });

  it("refuses a quick phrase code with a digit, naming the row", () => {
    const file = JSON.stringify({
      format: "msime-personal-dictionary",
      version: 1,
      entries: [
        { kind: "quickPhrase", key: "nh", value: "你好", weight: 100000 },
        { kind: "quickPhrase", key: "nh1", value: "你好", weight: 100000 },
      ],
    });
    expect(() => parsePersonalDictionaryImport(file)).toThrow(
      "第 2 条：词条内容不符合输入引擎规则。",
    );
  });
});
