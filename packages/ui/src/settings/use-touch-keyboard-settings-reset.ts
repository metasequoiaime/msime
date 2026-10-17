import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../core/preferences-model";

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
}

/** 「恢复默认」说明和确认框里列出的几项：只列这一页实际给出的控件，例如 macOS 没有键盘高度，桌面宿主没有工具栏按钮，不在文案里提它们。 */
export function touchKeyboardResetSubjects({
  height,
  spacing,
  voiceShortcut,
  toolbar,
}: {
  height: boolean;
  spacing: boolean;
  voiceShortcut: boolean;
  toolbar: boolean;
}) {
  const subjects = [
    height && "高度",
    spacing && "间距",
    voiceShortcut && "顶部语音入口",
    toolbar && "工具栏按钮",
  ].filter((subject): subject is string => Boolean(subject));
  return subjects.length > 1
    ? `${subjects.slice(0, -1).join("、")}和${subjects[subjects.length - 1]}`
    : (subjects[0] ?? "屏幕键盘设置");
}

/** Resets optional touch-keyboard overrides by removing them from the draft. */
export function useTouchKeyboardSettingsReset({
  draft,
  setDraft,
  setError,
  setNotice,
  confirm,
}: UseTouchKeyboardSettingsResetOptions) {
  /** `subjects` 是页面上「恢复默认」说明里列出的几项（见 `touchKeyboardResetSubjects`），确认框用同一句话。 */
  async function resetTouchKeyboardSettings(subjects: string) {
    if (!draft) return;
    const confirmed = await confirm({
      title: "恢复屏幕键盘默认值",
      message: `${subjects}都会回到默认。`,
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
    delete next.harmony_toolbar;
    setDraft(next);
    setError("");
    setNotice("屏幕键盘设置已恢复默认。");
  }

  return resetTouchKeyboardSettings;
}
