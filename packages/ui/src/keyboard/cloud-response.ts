export type CloudClipboardItemShape = { id: string; text: string };
export type CloudDictionaryEntryShape = {
  id: string;
  kind: string;
  code: string;
  word: string;
  weight: number;
  revision: number;
};
export type CloudDictionaryCatalogEntryShape = {
  kind: string;
  code: string;
  word: string;
  weight: number;
};
export type CloudCandidateShape = { code: string; canonical_pinyin?: string | null };

export function cloudResponseText(value: {
  text?: unknown;
  content?: unknown;
}): string | undefined {
  return typeof value.text === "string"
    ? value.text
    : typeof value.content === "string"
      ? value.content
      : undefined;
}

export function cloudResponseRequest<T extends { status: string }>(value: {
  request?: unknown;
}): T | null {
  return value.request &&
    typeof value.request === "object" &&
    typeof (value.request as T).status === "string"
    ? (value.request as T)
    : null;
}

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

export function cloudDictionaryEntries<
  T extends CloudDictionaryEntryShape = CloudDictionaryEntryShape,
>(value: { entries?: unknown }): T[] {
  return Array.isArray(value.entries)
    ? value.entries.filter(
        (entry): entry is T =>
          Boolean(entry) &&
          typeof entry === "object" &&
          typeof (entry as T).id === "string" &&
          typeof (entry as T).kind === "string" &&
          typeof (entry as T).code === "string" &&
          typeof (entry as T).word === "string" &&
          typeof (entry as T).weight === "number" &&
          typeof (entry as T).revision === "number",
      )
    : [];
}

export function cloudDictionaryCatalogEntries<
  T extends CloudDictionaryCatalogEntryShape = CloudDictionaryCatalogEntryShape,
>(value: { catalog_entries?: unknown }): T[] {
  return Array.isArray(value.catalog_entries)
    ? value.catalog_entries.filter(
        (entry): entry is T =>
          Boolean(entry) &&
          typeof entry === "object" &&
          typeof (entry as T).kind === "string" &&
          typeof (entry as T).code === "string" &&
          typeof (entry as T).word === "string" &&
          typeof (entry as T).weight === "number",
      )
    : [];
}

export function candidateMutationCode(candidate: CloudCandidateShape): string {
  return candidate.canonical_pinyin && candidate.canonical_pinyin.length > 0
    ? candidate.canonical_pinyin
    : candidate.code;
}
