import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";

export interface TouchKeyboardSettingsResetConfirmOptions {
  title: string;
  message: string;
  confirmLabel: string;
}

export interface UseTouchKeyboardSettingsResetOptions {
  draft: Preferences | undefined;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  setError: (error: string) => void;
  setNotice: (notice: string) => void;
  confirm: (options: TouchKeyboardSettingsResetConfirmOptions) => Promise<boolean>;
  /** False where the host's keyboard draws no voice button at the top (macOS), so the confirmation does not name a setting the page does not show. */
  voiceShortcut?: boolean;
}

/** Resets optional touch-keyboard overrides by removing them from the draft. */
export function useTouchKeyboardSettingsReset({
  draft,
  setDraft,
  setError,
  setNotice,
  confirm,
  voiceShortcut = true,
}: UseTouchKeyboardSettingsResetOptions) {
  async function resetTouchKeyboardSettings() {
    if (!draft) return;
    const confirmed = await confirm({
      title: "恢复屏幕键盘默认值",
      message: voiceShortcut
        ? "高度、间距、顶部语音入口和工具栏按钮都会回到默认。"
        : "高度、间距和工具栏按钮都会回到默认。",
      confirmLabel: "恢复",
    });
    if (!confirmed || !draft) return;
    const next = { ...draft };
    // Delete optional fields instead of storing current defaults. This keeps reset forward-compatible
    // when a host changes its fallback values.
    delete next.touch_key_spacing_tenths;
    delete next.touch_row_spacing_tenths;
    delete next.touch_keyboard_height_adjustment;
    delete next.touch_voice_shortcut;
    delete next.touch_toolbar;
    setDraft(next);
    setError("");
    setNotice("屏幕键盘设置已恢复默认。");
  }

  return resetTouchKeyboardSettings;
}
