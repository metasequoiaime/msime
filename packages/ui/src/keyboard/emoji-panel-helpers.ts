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

/** Prefers the first CJK keyword for a compact tooltip label, then the first token. */
export function emojiDisplayName(keywords: string | undefined, fallback = ""): string {
  const tokens = (keywords ?? "").split(/\s+/).filter(Boolean);
  if (!tokens.length) return fallback;
  const cjk = tokens.find((token) => /[\u3400-\u9fff\uf900-\ufaff]/.test(token));
  return cjk ?? tokens[0];
}
