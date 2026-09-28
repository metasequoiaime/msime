export type CloudClipboardItemShape = { id: string; text: string };
export type CloudDictionaryEntryShape = { id: string; code: string; word: string };
export type CloudDictionaryCatalogEntryShape = { kind: string; code: string; word: string };
export type CloudCandidateShape = { code: string; canonical_pinyin?: string | null };

export function cloudClipboardItems(value: { items?: unknown }): CloudClipboardItemShape[] {
  return Array.isArray(value.items)
    ? value.items.filter(
        (item): item is CloudClipboardItemShape =>
          Boolean(item) &&
          typeof item === "object" &&
          typeof (item as CloudClipboardItemShape).id === "string" &&
          typeof (item as CloudClipboardItemShape).text === "string",
      )
    : [];
}

export function cloudDictionaryEntries(value: { entries?: unknown }): CloudDictionaryEntryShape[] {
  return Array.isArray(value.entries)
    ? value.entries.filter(
        (entry): entry is CloudDictionaryEntryShape =>
          Boolean(entry) &&
          typeof entry === "object" &&
          typeof (entry as CloudDictionaryEntryShape).id === "string" &&
          typeof (entry as CloudDictionaryEntryShape).code === "string" &&
          typeof (entry as CloudDictionaryEntryShape).word === "string",
      )
    : [];
}

export function cloudDictionaryCatalogEntries(value: {
  catalog_entries?: unknown;
}): CloudDictionaryCatalogEntryShape[] {
  return Array.isArray(value.catalog_entries)
    ? value.catalog_entries.filter(
        (entry): entry is CloudDictionaryCatalogEntryShape =>
          Boolean(entry) &&
          typeof entry === "object" &&
          typeof (entry as CloudDictionaryCatalogEntryShape).kind === "string" &&
          typeof (entry as CloudDictionaryCatalogEntryShape).code === "string" &&
          typeof (entry as CloudDictionaryCatalogEntryShape).word === "string",
      )
    : [];
}

export function candidateMutationCode(candidate: CloudCandidateShape): string {
  return candidate.canonical_pinyin && candidate.canonical_pinyin.length > 0
    ? candidate.canonical_pinyin
    : candidate.code;
}
