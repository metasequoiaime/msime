import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";

export interface UseClipboardHistoryToggleOptions {
  draft: Preferences | undefined;
  enabled: boolean;
  clear?: () => Promise<void>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  setError: (error: string) => void;
}

/** Updates clipboard-history preference and clears retained local history when opting out. */
export function useClipboardHistoryToggle({
  draft,
  enabled,
  clear,
  setDraft,
  setError,
}: UseClipboardHistoryToggleOptions) {
  return (nextEnabled: boolean) => {
    if (!draft) return;
    setDraft((current) => (current ? { ...current, clipboard_history: nextEnabled } : current));
    if (!nextEnabled && enabled && clear) {
      void clear().catch(() => setError("无法清空剪贴板历史，请稍后重试。"));
    }
  };
}
