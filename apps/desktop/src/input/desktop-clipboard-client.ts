import { invoke } from "@tauri-apps/api/core";
import type { ClipboardHistoryClient, ClipboardHistoryEntry } from "@msime/ui";

export function createDesktopClipboardClient(): Required<ClipboardHistoryClient> {
  return {
    clear: () => invoke<void>("clear_clipboard_history"),
    list: () => invoke<ClipboardHistoryEntry[]>("list_clipboard_history"),
    sync: () => invoke<ClipboardHistoryEntry[]>("sync_clipboard_history"),
    copy: (text) => invoke<void>("copy_text", { text }),
    remove: (text) => invoke<void>("remove_clipboard_history", { text }),
    setPinned: (text, pinned) => invoke<void>("set_clipboard_history_pinned", { text, pinned }),
  };
}
