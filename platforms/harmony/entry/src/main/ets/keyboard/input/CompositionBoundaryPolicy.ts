export enum CompositionBoundaryAction {
  NONE = "none",
  COMMIT_RAW = "commit-raw",
  FINISH_COMPOSITION = "finish-composition",
}

export enum CompositionBoundary {
  MODE_SWITCH = "mode-switch",
  DEACTIVATE = "deactivate",
}

/** How a mode or panel boundary preserves text already entered into the current scheme. */
export class CompositionBoundaryPolicy {
  static action(
    composing: boolean,
    japanese: boolean,
    boundary: CompositionBoundary,
    commitsOnBlur: boolean = false,
  ): CompositionBoundaryAction {
    if (!composing) {
      return CompositionBoundaryAction.NONE;
    }
    // A Korean syllable, a Zhuyin conversion or a Vietnamese word is text the user already wrote, like Japanese kana (`commits_on_blur`); the keys behind it are not something to commit instead.
    if (boundary === CompositionBoundary.DEACTIVATE || japanese || commitsOnBlur) {
      return CompositionBoundaryAction.FINISH_COMPOSITION;
    }
    return CompositionBoundaryAction.COMMIT_RAW;
  }
}
