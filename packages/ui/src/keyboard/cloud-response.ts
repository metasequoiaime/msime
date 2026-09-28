export type CloudClipboardItemShape = { id: string; text: string };
export type CloudDictionaryEntryShape = { id: string; code: string; word: string };

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
