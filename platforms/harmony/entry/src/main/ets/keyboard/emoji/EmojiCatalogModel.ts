import { utf8Length } from "../Utf8";

/**
 * Paging and recent-selection policy for the Engine-owned emoji catalog, ported from
 * platforms/android/java/app/msime/android/EmojiCatalogModel.java.
 *
 * Lengths are counted in code points, not UTF-16 units: an emoji is routinely several units long and
 * a sequence with a skin tone or a zero-width joiner is longer still.
 */
export const EMOJI_COLUMNS: number = 8;
export const EMOJI_PAGE_SIZE: number = 64;
export const EMOJI_RECENTS_LIMIT: number = 24;
export const EMOJI_RECENTS_MAX_BYTES: number = 16 * 1024;
export const MAX_TEXT_CODE_POINTS: number = 32;
export const MAX_ANNOTATION_CODE_POINTS: number = 1024;
export const MAX_CATALOG_GROUPS: number = 64;
export const MAX_GROUP_CODE_UNITS: number = 128;

export interface EmojiCategory {
  readonly group: string;
  readonly title: string;
}

export interface EmojiItem {
  readonly text: string;
  readonly annotation: string;
  readonly group: string;
}

export interface EmojiSymbolGroup {
  readonly parent: string;
  readonly title: string;
}

export interface EmojiPage {
  readonly items: EmojiItem[];
  readonly nextOffset: number;
  readonly complete: boolean;
}

function codePointCount(text: string): number {
  let count: number = 0;
  for (const _character of text) {
    count++;
  }
  return count;
}

function isUsableGroupText(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_GROUP_CODE_UNITS &&
    value.trim().length > 0
  );
}

/** Bounds and de-duplicates Engine-provided kaomoji group names before they reach ArkUI. */
export function normalizeGroups(values: unknown): string[] {
  if (!Array.isArray(values) || values.length > MAX_CATALOG_GROUPS) {
    return [];
  }
  const groups: string[] = [];
  for (const value of values) {
    if (!isUsableGroupText(value) || groups.includes(value)) {
      continue;
    }
    groups.push(value);
  }
  return groups;
}

/** Bounds, validates, and de-duplicates Engine-provided symbol parent/title pairs. */
export function normalizeSymbolGroups(values: unknown): EmojiSymbolGroup[] {
  if (!Array.isArray(values) || values.length > MAX_CATALOG_GROUPS) {
    return [];
  }
  const groups: EmojiSymbolGroup[] = [];
  for (const value of values) {
    if (value === null || typeof value !== "object") {
      continue;
    }
    const candidate = value as { parent?: unknown; title?: unknown };
    if (!isUsableGroupText(candidate.parent) || !isUsableGroupText(candidate.title)) {
      continue;
    }
    if (
      groups.some(
        (group: EmojiSymbolGroup): boolean =>
          group.parent === candidate.parent && group.title === candidate.title,
      )
    ) {
      continue;
    }
    groups.push({ parent: candidate.parent, title: candidate.title });
  }
  return groups;
}

// Unicode group order; the database row sort order interleaves Symbols and Flags.
const CATEGORIES: EmojiCategory[] = [
  { group: "Smileys and emotion", title: "笑脸" },
  { group: "People and body", title: "人物" },
  { group: "Animals and nature", title: "动物" },
  { group: "Food and drink", title: "食物" },
  { group: "Travel and places", title: "旅行" },
  { group: "Activities", title: "活动" },
  { group: "Objects", title: "物品" },
  { group: "Symbols", title: "符号" },
  { group: "Flags", title: "旗帜" },
];

export class EmojiCatalogModel {
  static categories(): EmojiCategory[] {
    return CATEGORIES;
  }

  static item(text: string, annotation: string, group: string): EmojiItem {
    if (text.length === 0 || codePointCount(text) > MAX_TEXT_CODE_POINTS) {
      throw new Error("Invalid emoji catalog text");
    }
    if (codePointCount(annotation) > MAX_ANNOTATION_CODE_POINTS) {
      throw new Error("Invalid emoji annotation");
    }
    if (group.length === 0 || group.length > 128) {
      throw new Error("Invalid emoji group");
    }
    return { text: text, annotation: annotation, group: group };
  }

  /**
   * A page the Engine returned is checked before it is trusted: an offset that goes backwards, or a
   * page that claims to be incomplete while advancing nowhere, would loop the caller forever.
   */
  static validatePage(
    items: EmojiItem[],
    requestedOffset: number,
    limit: number,
    nextOffset: number,
    complete: boolean,
  ): EmojiPage {
    if (
      requestedOffset < 0 ||
      limit < 1 ||
      limit > 255 ||
      items.length > limit ||
      nextOffset < requestedOffset ||
      nextOffset > requestedOffset + limit ||
      nextOffset > 2147483647 ||
      (!complete && nextOffset === requestedOffset)
    ) {
      throw new Error("Invalid emoji catalog page");
    }
    return { items: items.slice(), nextOffset: nextOffset, complete: complete };
  }

  /** Most-recent first, deduplicated, and bounded without retaining invalid persisted values. */
  static normalizeRecents(stored: string[] | null): string[] {
    const unique: string[] = [];
    if (stored !== null) {
      for (const text of stored) {
        if (
          text.length === 0 ||
          codePointCount(text) > MAX_TEXT_CODE_POINTS ||
          unique.includes(text)
        ) {
          continue;
        }
        unique.push(text);
        if (unique.length === EMOJI_RECENTS_LIMIT) {
          break;
        }
      }
    }
    return unique;
  }

  /**
   * Reads the small app-private recents document without trusting its shape.
   *
   * Recents are convenience state rather than a reason to make the keyboard fail: a malformed or
   * over-sized file is treated as empty, while every value that survives still goes through the
   * same code-point and duplicate bounds as values selected in this process.
   */
  static parseRecents(document: string | null): string[] {
    if (
      document === null ||
      document.length === 0 ||
      utf8Length(document) > EMOJI_RECENTS_MAX_BYTES
    ) {
      return [];
    }
    try {
      const decoded: unknown = JSON.parse(document);
      if (!Array.isArray(decoded)) {
        return [];
      }
      const values: string[] = [];
      for (const value of decoded) {
        if (typeof value === "string") {
          values.push(value);
        }
      }
      return EmojiCatalogModel.normalizeRecents(values);
    } catch {
      return [];
    }
  }

  static serializeRecents(stored: string[] | null): string {
    return JSON.stringify(EmojiCatalogModel.normalizeRecents(stored));
  }

  static recordRecent(stored: string[] | null, selected: string): string[] {
    if (selected.length === 0 || codePointCount(selected) > MAX_TEXT_CODE_POINTS) {
      throw new Error("Invalid recent emoji");
    }
    const reordered: string[] = [selected];
    for (const text of EmojiCatalogModel.normalizeRecents(stored)) {
      if (selected !== text) {
        reordered.push(text);
      }
      if (reordered.length === EMOJI_RECENTS_LIMIT) {
        break;
      }
    }
    return reordered;
  }
}
