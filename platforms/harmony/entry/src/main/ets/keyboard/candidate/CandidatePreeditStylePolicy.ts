import { LocalInputMode, LocalInputModeDefinition } from "../input/LocalInputMode";

/**
 * What the phone's composition line shows under 「候选栏预编辑」, ported from platforms/android/java/app/msime/android/CandidatePreeditStylePolicy.java.
 *
 * `pinyin` shows the spelling; `empty` leaves the line without it, which is the Windows `candidate_window_preedit_style = "empty"` (`preeditVisible=false`). A 2in1 candidate window takes the whole row away, because it only exists while something is composed. The phone line stays reserved and names the keyboard when idle, so there only the spelling goes and the line keeps its height.
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
