/**
 * Bounded, pinnable clipboard history, ported from
 * platforms/ios/SharedUI/ClipboardHistoryStore.swift.
 *
 * The file is treated as untrusted even though this host wrote it: it lives in the sandbox, but a
 * truncated write or an older build is enough to produce a document this one cannot honour, and the
 * remedy is to say so rather than to show half a history.
 *
 * Reading and writing are supplied by the caller rather than performed here, so the rules can be
 * checked off a device. ClipboardHistoryPolicy holds the per-entry bounds these share.
 */
import { ClipboardHistoryPolicy } from "./ClipboardHistoryPolicy";

// 1000 条记录可能都包含共享存储允许的 40,000 字节文本；JSON 转义会进一步放大文件，解析
// 上限必须覆盖 Rust 存储按最坏情况计算出的边界。
export const MAX_FILE_BYTES: number = 256 * 1024 * 1024;

export interface ClipboardHistoryItem {
  readonly text: string;
  /** Milliseconds since the epoch, which is what orders the unpinned entries. */
  readonly at: number;
  readonly pinned: boolean;
}

export enum ClipboardFailure {
  EMPTY = "剪贴板中没有可保存的文本。",
  TOO_LONG = "单条最多保存 10,000 字，请缩短后重试。",
  FULL = "历史记录已满且全部固定，请先取消固定或删除一条。",
  INVALID_FILE = "历史记录无法读取；原文件已保留。",
  STALE = "记录已在其他窗口中更改，请重试。",
}

export class ClipboardHistoryError extends Error {
  readonly failure: ClipboardFailure;

  constructor(failure: ClipboardFailure) {
    super(failure);
    this.failure = failure;
  }
}

function item(text: string, at: number, pinned: boolean): ClipboardHistoryItem {
  return { text: text, at: at, pinned: pinned };
}

export class ClipboardHistoryStore {
  static readonly LIMIT: number = ClipboardHistoryPolicy.LIMIT;

  /**
   * Pinned entries first, then most recent. A document that breaks the bounds is rejected outright:
   * silently dropping the offending entries would hide that something wrote a file this cannot read.
   */
  static parse(document: string | null): ClipboardHistoryItem[] {
    if (document === null || document.length === 0) {
      return [];
    }
    let raw: ClipboardHistoryItem[];
    try {
      raw = JSON.parse(document) as ClipboardHistoryItem[];
    } catch (error) {
      throw new ClipboardHistoryError(ClipboardFailure.INVALID_FILE);
    }
    if (!Array.isArray(raw) || raw.length > ClipboardHistoryStore.LIMIT) {
      throw new ClipboardHistoryError(ClipboardFailure.INVALID_FILE);
    }
    const items: ClipboardHistoryItem[] = [];
    for (const entry of raw) {
      if (
        entry === null ||
        typeof entry.text !== "string" ||
        !ClipboardHistoryPolicy.acceptable(entry.text) ||
        !Number.isFinite(entry.at) ||
        typeof entry.pinned !== "boolean"
      ) {
        throw new ClipboardHistoryError(ClipboardFailure.INVALID_FILE);
      }
      items.push(item(entry.text, entry.at, entry.pinned));
    }
    return ClipboardHistoryStore.ordered(items);
  }

  static ordered(items: ClipboardHistoryItem[]): ClipboardHistoryItem[] {
    return items.slice().sort((left: ClipboardHistoryItem, right: ClipboardHistoryItem): number => {
      if (left.pinned !== right.pinned) {
        return left.pinned ? -1 : 1;
      }
      return right.at - left.at;
    });
  }

  /**
   * Adds text, or moves it to the front if it is already there.
   *
   * When the list is full the oldest unpinned entry makes room. If every entry is pinned there is
   * nothing to evict, and refusing is the only honest answer: a pin means the user asked for that
   * entry to stay.
   */
  static add(existing: ClipboardHistoryItem[], text: string, now: number): ClipboardHistoryItem[] {
    if (text.trim().length === 0) {
      throw new ClipboardHistoryError(ClipboardFailure.EMPTY);
    }
    if (!ClipboardHistoryPolicy.acceptable(text)) {
      throw new ClipboardHistoryError(ClipboardFailure.TOO_LONG);
    }
    const items: ClipboardHistoryItem[] = existing.slice();
    const index: number = items.findIndex(
      (entry: ClipboardHistoryItem): boolean => entry.text === text,
    );
    if (index >= 0) {
      items[index] = item(text, now, items[index].pinned);
      return ClipboardHistoryStore.ordered(items);
    }
    if (items.length === ClipboardHistoryStore.LIMIT) {
      let oldest: number = -1;
      for (let position: number = 0; position < items.length; position++) {
        if (!items[position].pinned && (oldest === -1 || items[position].at < items[oldest].at)) {
          oldest = position;
        }
      }
      if (oldest === -1) {
        throw new ClipboardHistoryError(ClipboardFailure.FULL);
      }
      items.splice(oldest, 1);
    }
    items.push(item(text, now, false));
    return ClipboardHistoryStore.ordered(items);
  }
}
