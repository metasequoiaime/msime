import type { EmojiCatalogGroup, EmojiCatalogItem } from "../emoji/emoji-catalog";

export function matchesEmojiItem(item: EmojiCatalogItem, query: string): boolean {
  const normalizedQuery = query.toLocaleLowerCase();
  return (
    !query ||
    item.text.includes(query) ||
    item.keywords.toLocaleLowerCase().includes(normalizedQuery)
  );
}

export function clipboardTooltip(text: string): string {
  const preview = text.replace(/[\r\t]/g, " ").replace(/\n+$/, "");
  const characters = Array.from(preview);
  return characters.length > 200 ? `${characters.slice(0, 200).join("")}…` : preview;
}

export function flattenGroups(groups: EmojiCatalogGroup[]): EmojiCatalogItem[] {
  return groups.flatMap((group) => group.items);
}
