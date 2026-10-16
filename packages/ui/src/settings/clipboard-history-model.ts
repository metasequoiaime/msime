export type ClipboardHistoryEntry = { text: string; timestampMs: number; pinned: boolean };

export type ClipboardHistoryClient = {
  clear(): Promise<void>;
  list?(): Promise<ClipboardHistoryEntry[]>;
  sync?(): Promise<ClipboardHistoryEntry[]>;
  copy?(text: string): Promise<void>;
  remove?(text: string): Promise<void>;
  setPinned?(text: string, pinned: boolean): Promise<void>;
};
