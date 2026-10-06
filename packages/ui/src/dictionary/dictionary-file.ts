import { utf8ByteLength } from "../core/text";
import { dictionaryFileSizeLabel } from "./dictionary-size";

// Dictionary exports in the wild are not all UTF-8: Windows tools still write
// UTF-16 with a BOM and GB18030. Ported from the shipped settings page so an
// imported file reads the same here.
function decodeUtf8(bytes: Uint8Array, fatal: boolean): string {
  return new TextDecoder("utf-8", { fatal }).decode(bytes);
}

export function decodeDictionaryBytes(bytes: Uint8Array): string {
  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    return decodeUtf8(bytes.subarray(3), true);
  }
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return new TextDecoder("utf-16le").decode(bytes.subarray(2));
  }
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) {
    return new TextDecoder("utf-16be").decode(bytes.subarray(2));
  }
  try {
    return decodeUtf8(bytes, true);
  } catch {
    // A legacy encoding is more likely than a corrupt file; replacement
    // characters are the last resort rather than the first.
    for (const label of ["gb18030", "gbk"]) {
      try {
        return new TextDecoder(label).decode(bytes);
      } catch {
        continue;
      }
    }
    return decodeUtf8(bytes, false);
  }
}

/** The largest dictionary file the settings page reads for the desktop bridge, which sends a larger import to the host as several requests, so this, not the host's 64 KiB per request, is the bound a user meets. The bridge's own bound on the decoded text, `MAX_IMPORT_TEXT_BYTES` in `apps/desktop/src-tauri/src/dictionary_import.rs`, is 1.5 times this, so a file accepted here is never refused there for its size. The Windows settings page reads a file whole without a bound; this one keeps a bound because the whole file is held in the page and then in the bridge. */
export const MAX_DICTIONARY_FILE_BYTES = 32 * 1024 * 1024;

/** The bound for a host whose dictionary client sends a file to the input method in one request instead of batching it the way the desktop bridge does: the mobile hosts and HarmonyOS. It is the bound the page has always applied to them; their own request bound refuses what it cannot take. */
export const UNBATCHED_DICTIONARY_FILE_BYTES = 1_048_576;

/** Read and decode a dictionary file of at most `maxBytes`, refusing a larger one before reading it. A caller whose backend takes less than the desktop bridge passes its own bound. */
export async function readDictionaryFile(
  file: File,
  maxBytes: number = MAX_DICTIONARY_FILE_BYTES,
): Promise<string> {
  if (file.size > maxBytes)
    throw new Error(`文件不能超过 ${dictionaryFileSizeLabel(maxBytes)}，请拆分后分别导入。`);
  return decodeDictionaryBytes(new Uint8Array(await file.arrayBuffer()));
}

export type PersonalDictionaryImportEntry = {
  kind: "pinyin" | "wubi" | "wubi98" | "quickPhrase" | "english";
  key: string;
  value: string;
  weight: number;
};

const personalDictionaryKinds = new Set<PersonalDictionaryImportEntry["kind"]>([
  "pinyin",
  "wubi",
  "wubi98",
  "quickPhrase",
  "english",
]);

function personalEntryIdentity(entry: PersonalDictionaryImportEntry): string {
  return `${entry.kind}:${utf8ByteLength(entry.key)}:${entry.key}${entry.value}`;
}

/** Parse the Apple-compatible bounded personal dictionary envelope before previewing it. */
export function parsePersonalDictionaryImport(text: string): PersonalDictionaryImportEntry[] {
  if (utf8ByteLength(text) > 1_048_576) throw new Error("文件不能超过 1 MB。");
  let file: unknown;
  try {
    file = JSON.parse(text);
  } catch {
    throw new Error("文件格式不支持，请按示例 JSON 文件填写。");
  }
  if (!file || typeof file !== "object" || Array.isArray(file))
    throw new Error("文件格式不支持，请按示例 JSON 文件填写。");
  const envelope = file as { format?: unknown; version?: unknown; entries?: unknown };
  if (
    envelope.format !== "msime-personal-dictionary" ||
    envelope.version !== 1 ||
    !Array.isArray(envelope.entries)
  ) {
    throw new Error("文件格式不支持，请按示例 JSON 文件填写。");
  }
  if (envelope.entries.length < 1 || envelope.entries.length > 128) {
    throw new Error("每次导入需要 1–128 条词条，请将较大的词库拆分后导入。");
  }
  const identities = new Set<string>();
  const entries = envelope.entries.map((raw, index): PersonalDictionaryImportEntry => {
    if (!raw || typeof raw !== "object" || Array.isArray(raw))
      throw new Error(`第 ${index + 1} 条：词条格式无效。`);
    const value = raw as Record<string, unknown>;
    const kind = value.kind;
    const key = value.key;
    const word = value.value;
    const weight = value.weight;
    if (
      typeof kind !== "string" ||
      !personalDictionaryKinds.has(kind as PersonalDictionaryImportEntry["kind"]) ||
      typeof key !== "string" ||
      typeof word !== "string" ||
      !Number.isSafeInteger(weight) ||
      (weight as number) < 0
    ) {
      throw new Error(`第 ${index + 1} 条：词条格式无效。`);
    }
    const normalizedKey =
      kind === "pinyin" ? key.toLowerCase().replaceAll(" ", "'") : key.toLowerCase();
    const entry = {
      kind: kind as PersonalDictionaryImportEntry["kind"],
      key: normalizedKey,
      value: word,
      weight: weight as number,
    };
    const keyBytes = utf8ByteLength(normalizedKey);
    const keyValid =
      entry.kind === "pinyin"
        ? normalizedKey.length > 0 && keyBytes <= 512 && /^[a-z']+$/.test(normalizedKey)
        : entry.kind === "wubi" || entry.kind === "wubi98"
          ? normalizedKey.length > 0 && keyBytes <= 4 && /^[a-z]+$/.test(normalizedKey)
          : entry.kind === "quickPhrase"
            ? normalizedKey.length > 0 && keyBytes <= 32 && /^[a-z]+$/.test(normalizedKey)
            : normalizedKey.length > 0 && keyBytes <= 64 && /^[a-z'-]+$/.test(normalizedKey);
    if (
      !keyValid ||
      word.length === 0 ||
      utf8ByteLength(word) > 4096 ||
      (entry.kind === "quickPhrase"
        ? /[\u0000-\u0008\u000b-\u001f\u007f]/.test(word)
        : /[\u0000-\u001f\u007f]/.test(word)) ||
      (weight as number) < 1 ||
      (weight as number) > 100_000_000
    ) {
      throw new Error(`第 ${index + 1} 条：词条内容不符合输入引擎规则。`);
    }
    const identity = personalEntryIdentity(entry);
    if (identities.has(identity))
      throw new Error(`第 ${index + 1} 条与前面的词条重复，请删除重复项后重试。`);
    identities.add(identity);
    return entry;
  });
  return entries;
}

export const personalDictionaryExample = JSON.stringify(
  {
    format: "msime-personal-dictionary",
    version: 1,
    entries: [
      { kind: "pinyin", key: "ni hao", value: "你好", weight: 100000 },
      { kind: "wubi", key: "wq", value: "你", weight: 100000 },
      { kind: "english", key: "hello", value: "Hello", weight: 100000 },
      { kind: "quickPhrase", key: "greeting", value: "你好！\n很高兴认识你。", weight: 100000 },
    ],
  },
  null,
  2,
);

export const DICTIONARY_PAGE_SIZE = 100;

export type LocalDictionaryKind = "pinyin" | "wubi" | "wubi98" | "quick_phrase" | "english";
export type LocalDictionaryFormat = "standard" | "windows" | "rime" | "hans";
export type DictionaryEntry = {
  kind: LocalDictionaryKind;
  key: string;
  value: string;
  weight: number;
  source?: "user" | "bundled";
};

/// Status line for one page of results, matching the shipped pager.
export function dictionaryPageStatus(offset: number, count: number, hasMore: boolean): string {
  if (!count) return "没有更多结果";
  return `第 ${offset + 1}–${offset + count} 条${hasMore ? "，后面还有结果" : ""}`;
}
