import {
  DICTIONARY_PAGE_SIZE,
  type DictionaryEntry,
  type LocalDictionaryKind,
} from "./dictionary-file";

const personalDictionaryExportKinds: readonly [LocalDictionaryKind, string][] = [
  ["pinyin", "拼音"],
  ["wubi", "五笔"],
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
  return {
    body: `# 类别\t编码\t词条\t权重\n${rows.length ? `${rows.join("\n")}\n` : ""}`,
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
  for (const [kind] of personalDictionaryExportKinds) {
    let offset = 0;
    let hasMore = true;
    while (hasMore && offset <= 1_000_000) {
      const page = await dictionary.list(offset, DICTIONARY_PAGE_SIZE, kind, "");
      entries.push(
        ...page.entries.filter((entry) => entry.kind === kind && entry.source !== "bundled"),
      );
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

export function dictionaryExportName(kind: string): string {
  const names: Record<string, string> = {
    pinyin: "水杉IME-拼音用户词库.txt",
    wubi: "水杉IME-五笔用户词库.txt",
    english: "水杉IME-英文用户词库.txt",
    quick_phrase: "水杉IME-快捷短语用户词库.txt",
  };
  return names[kind] ?? "水杉IME-用户词库.txt";
}

export function dictionaryKindLabel(kind: string): string {
  return (
    {
      pinyin: "全拼",
      wubi: "五笔",
      english: "英文",
      quick_phrase: "快捷短语",
    }[kind] ?? "词库"
  );
}

export function dictionaryExportPayload(
  kind: string,
  format: string,
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
  return { body: "\ufeff" + kept.join("\n") + "\n", rows: kept.length };
}
