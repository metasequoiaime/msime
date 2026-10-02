/**
 * Editor-action labels shared by return-key rendering and dispatch, ported from
 * platforms/android/java/app/msime/android/ReturnKeyAction.java.
 *
 * The numbers are the editor action constants the host receives; they are kept rather than renamed so
 * the mapping stays checkable against what the framework sends.
 */
const GO: number = 2;
const SEARCH: number = 3;
const SEND: number = 4;
const NEXT: number = 5;
const DONE: number = 6;
const PREVIOUS: number = 7;

export enum ReturnDispatch {
  EDITOR = "editor",
  FINISH_COMPOSITION = "finish-composition",
  COMMIT_HIGHLIGHTED = "commit-highlighted",
  COMMIT_READING = "commit-reading",
  /** Korean and Vietnamese: commit the open composition, then let Return do what it does in the editor. */
  FINISH_THEN_EDITOR = "finish-then-editor",
}

export class ReturnKeyAction {
  static dispatch(
    japanese: boolean,
    composing: boolean,
    candidateCount: number,
    japaneseConverted: boolean = false,
    korean: boolean = false,
    vietnamese: boolean = false,
  ): ReturnDispatch {
    // A Vietnamese word is finished text with no list behind it, so Return commits it and still breaks the line or submits, as for a Korean syllable.
    if (vietnamese) {
      return composing ? ReturnDispatch.FINISH_THEN_EDITOR : ReturnDispatch.EDITOR;
    }
    // A Korean syllable is finished text rather than a spelling to confirm, so Return commits it and still breaks the line or submits, as every Korean keyboard does. The one exception is the syllable's open Hanja list, the only candidates Korean has: there Return chooses the highlighted Hanja, which only the session knows (msime_client.h).
    if (korean) {
      if (!composing) {
        return ReturnDispatch.EDITOR;
      }
      return candidateCount > 0
        ? ReturnDispatch.COMMIT_HIGHLIGHTED
        : ReturnDispatch.FINISH_THEN_EDITOR;
    }
    if (japanese && composing) {
      return japaneseConverted ? ReturnDispatch.COMMIT_HIGHLIGHTED : ReturnDispatch.COMMIT_READING;
    }
    if (candidateCount > 0) {
      return ReturnDispatch.COMMIT_HIGHLIGHTED;
    }
    return composing ? ReturnDispatch.FINISH_COMPOSITION : ReturnDispatch.EDITOR;
  }

  static performsEditorAction(action: number, disabled: boolean): boolean {
    if (disabled) {
      return false;
    }
    return (
      action === GO ||
      action === SEARCH ||
      action === SEND ||
      action === NEXT ||
      action === DONE ||
      action === PREVIOUS
    );
  }

  /** A handled composition consumes Return before any editor action or newline is considered. */
  static shouldPerformEditorAction(
    action: number,
    disabled: boolean,
    compositionHandled: boolean,
  ): boolean {
    return !compositionHandled && ReturnKeyAction.performsEditorAction(action, disabled);
  }

  static title(action: number, disabled: boolean): string {
    if (!ReturnKeyAction.performsEditorAction(action, disabled)) {
      return "换行";
    }
    switch (action) {
      case GO:
        return "前往";
      case SEARCH:
        return "搜索";
      case SEND:
        return "发送";
      case NEXT:
        return "下一项";
      case DONE:
        return "完成";
      case PREVIOUS:
        return "上一项";
      default:
        return "换行";
    }
  }
}
