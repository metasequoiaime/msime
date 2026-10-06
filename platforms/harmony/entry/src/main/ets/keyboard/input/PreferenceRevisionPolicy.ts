/**
 * Whether the settings page needs to be told the document moved under it.
 *
 * The keyboard and the settings window are two processes of the same application, and the keyboard
 * writes preferences of its own: the toolbar's 简繁, 标点 and 全半角 buttons each save. A settings
 * window open at the same time never learned, so the first save it attempted came back a conflict —
 * the shared store compares revisions precisely so a stale window cannot overwrite what the keyboard
 * just wrote. Safe, but the user is told only that something clashed, and the desktop hosts never
 * get there because they deliver the new document instead.
 *
 * HarmonyOS has no cross-process listener for this, so the page is refreshed when its window comes
 * back to the front — which is when a user who just used the keyboard is looking at it again.
 *
 * The revision is what decides, not the visit. Refreshing on every activation would announce a
 * change that did not happen, and the page answers an announcement with "设置已被其他窗口修改" when
 * it holds unsaved edits: a warning that costs the user their draft for nothing.
 */

export class PreferenceRevisionPolicy {
  /**
   * Whether the document on disk has moved past what the page was last given.
   *
   * A revision that went backwards counts as a change too: the only ways it can happen are a
   * restored profile or a rewritten document, and both mean the page is holding something that no
   * longer describes the file.
   */
  static changed(observed: number, current: number): boolean {
    if (
      !Number.isSafeInteger(observed) ||
      !Number.isSafeInteger(current) ||
      current < 0
    ) {
      return false;
    }
    return observed !== current;
  }

  /**
   * The revision to remember after the page has been handed a document.
   *
   * Anything unusable leaves the previous value in place rather than replacing it with a number the
   * next comparison would read as a change.
   */
  static observe(previous: number, reported: number): number {
    return Number.isSafeInteger(reported) && reported >= 0 ? reported : previous;
  }
}
