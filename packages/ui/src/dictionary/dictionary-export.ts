import {
  DICTIONARY_PAGE_SIZE,
  type DictionaryEntry,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
} from "./dictionary-file";
import { utf8ByteLength } from "../core/text";

/** Keep the complete export within the size a settings bridge can hold at once. */
export const MAX_DICTIONARY_EXPORT_BYTES = 32 * 1024 * 1024;

const PERSONAL_EXPORT_HEADER = "# 类别\t编码\t词条\t权重\n";

const personalDictionaryExportKinds: readonly [LocalDictionaryKind, string][] = [
  ["pinyin", "拼音"],
  ["wubi", "86 五笔"],
  ["wubi98", "98 五笔"],
  ["quick_phrase", "快捷短语"],
  ["english", "英文"],
];

export function personalDictionaryExportName(): string {
  return "水杉用户词库.txt";
}

export function personalDictionaryExportPayload(entries: DictionaryEntry[]): {
  body: string;
  rows: number;
} {
  const rows = personalDictionaryExportKinds.flatMap(([kind, label]) =>
    entries
      .filter((entry) => entry.kind === kind)
      .map((entry) => `${label}\t${entry.key}\t${entry.value}\t${entry.weight}`),
  );
  const body = `${PERSONAL_EXPORT_HEADER}${rows.length ? `${rows.join("\n")}\n` : ""}`;
  if (utf8ByteLength(body) > MAX_DICTIONARY_EXPORT_BYTES) {
    throw new Error("dictionary_export_limit");
  }
  return {
    body,
    rows: rows.length,
  };
}

export async function loadAllPersonalDictionaryEntries(dictionary: {
  list: (
    offset: number,
    limit: number,
    kind?: LocalDictionaryKind,
    query?: string,
  ) => Promise<{ entries: DictionaryEntry[]; has_more: boolean }>;
}): Promise<DictionaryEntry[]> {
  const entries: DictionaryEntry[] = [];
  let bytes = utf8ByteLength(PERSONAL_EXPORT_HEADER);
  for (const [kind, label] of personalDictionaryExportKinds) {
    let offset = 0;
    let hasMore = true;
    while (hasMore && offset <= 1_000_000) {
      const page = await dictionary.list(offset, DICTIONARY_PAGE_SIZE, kind, "");
      for (const entry of page.entries) {
        if (entry.kind !== kind || entry.source === "bundled") continue;
        const row = `${label}\t${entry.key}\t${entry.value}\t${entry.weight}`;
        bytes += utf8ByteLength(row) + 1;
        if (bytes > MAX_DICTIONARY_EXPORT_BYTES) {
          throw new Error("dictionary_export_limit");
        }
        entries.push(entry);
      }
      if (!page.entries.length) {
        hasMore = false;
        break;
      }
      offset += page.entries.length;
      hasMore = page.has_more;
    }
    if (hasMore) throw new Error("dictionary_export_limit");
  }
  return entries;
}

export function dictionaryExportName(kind: LocalDictionaryKind): string {
  const names: Record<LocalDictionaryKind, string> = {
    pinyin: "水杉IME-拼音用户词库.txt",
    wubi: "水杉IME-五笔用户词库.txt",
    wubi98: "水杉IME-98五笔用户词库.txt",
    english: "水杉IME-英文用户词库.txt",
    quick_phrase: "水杉IME-快捷短语用户词库.txt",
  };
  return names[kind];
}

export function dictionaryKindLabel(kind: string): string {
  return (
    {
      pinyin: "全拼",
      wubi: "86 五笔",
      wubi98: "98 五笔",
      english: "英文",
      quick_phrase: "快捷短语",
    }[kind] ?? "词库"
  );
}

export function dictionaryExportPayload(
  kind: LocalDictionaryKind,
  format: LocalDictionaryFormat,
  text: string,
): { body: string; rows: number } {
  const lines = text.split("\n").filter((line) => line.trim().length > 0);
  const wordColumn = format === "windows" ? 1 : 0;
  const kept =
    kind === "pinyin"
      ? lines.filter((line) => {
          const columns = line.split("\t");
          const word = columns[wordColumn]?.trim() ?? "";
          return Array.from(word).length > 1;
        })
      : lines;
  if (!kept.length) return { body: "", rows: 0 };
  const body = "\ufeff" + kept.join("\n") + "\n";
  if (utf8ByteLength(body) > MAX_DICTIONARY_EXPORT_BYTES) {
    throw new Error("dictionary_export_limit");
  }
  return { body, rows: kept.length };
}
