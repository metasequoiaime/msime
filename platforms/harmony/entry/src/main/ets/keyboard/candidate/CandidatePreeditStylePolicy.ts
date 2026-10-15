import { LocalInputMode, LocalInputModeDefinition } from "../input/LocalInputMode";

/**
 * 组字行画出来的组字，移植自 platforms/android/java/app/msime/android/CandidatePreeditStylePolicy.java。
 *
 * `showPreedit` 为真时画拼写；为假时不画，即 Windows 的 `candidate_window_preedit_style = "empty"`（`preeditVisible=false`）。2in1 的候选窗口照「候选窗口预编辑」，不画时整行拿掉，因为窗口只在组字时存在。手机没有这个设置：拼写写在输入框里时为假（见 InlinePreeditPolicy.hidesStripReading），这一行保留高度、空闲时显示键盘名，只是不画拼写。
 *
 * Two things that share the line are deliberately not governed by the setting. The already-chosen part of a phrase stays: the runtime keeps it out of the document (`phrase_preedit`) so the user can keep spelling the rest, and hiding it here too would leave characters the user has picked visible nowhere. A local mode's trigger stays as well, because it names the running mode rather than being composed input; what the mode spells beyond it is hidden like any other spelling.
 */
export interface VisibleComposition {
  readonly text: string;
  /** Whether the segment caret is drawn: a hidden spelling has no caret to edit. */
  readonly caret: boolean;
}

export class CandidatePreeditStylePolicy {
  static visible(
    showPreedit: boolean,
    phrasePrefix: string,
    editing: string,
    localMode: string,
  ): VisibleComposition {
    const prefix: string = phrasePrefix ?? "";
    const text: string = editing ?? "";
    if (showPreedit) {
      return { text: text, caret: true };
    }
    const spelling: string = text.startsWith(prefix) ? text.substring(prefix.length) : text;
    const mode: LocalInputModeDefinition | null = LocalInputMode.fromPreferenceKey(localMode);
    if (mode !== null && spelling === mode.trigger) {
      return { text: prefix + spelling, caret: false };
    }
    return { text: prefix, caret: false };
  }
}
