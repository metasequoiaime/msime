import { KeyboardGeometry } from '../KeyboardGeometry';

/**
 * What a hardware key does while the 2in1 emoji panel is open, ported from `EmojiPanel::OnKeyDown` and the panel's search box in MSIME-Windows/server/src/emoji-panel/EmojiPanel.cpp.
 *
 * The Windows panel is its own focused window, so arrows move its selection, Enter or Space inserts the selected item, and printable keys land in its search box instead of the editor. The 2in1 panel lives in the candidate window and the editor keeps focus, so every key arrives here first; without this the arrows moved the document caret and Return put a newline under an open panel. Keys the panel has no use for are left to the editor, the way `OnKeyDown` answers false for them: a chord, Tab, or a Backspace with nothing typed into the search, which deletes in the document the same as the panel's own delete key.
 *
 * A phone never reaches this: its panel is driven by touch, and `KeyboardSession.routePanelKey` answers false there.
 */

const KEYCODE_SPACE: number = 2050;
const KEYCODE_ENTER: number = 2054;
const KEYCODE_DEL: number = 2055;
const KEYCODE_ESCAPE: number = 2070;
const KEYCODE_NUMPAD_ENTER: number = 2119;
const KEYCODE_DPAD_UP: number = 2012;
const KEYCODE_DPAD_DOWN: number = 2013;
const KEYCODE_DPAD_LEFT: number = 2014;
const KEYCODE_DPAD_RIGHT: number = 2015;
const KEYCODE_MOVE_HOME: number = 2081;
const KEYCODE_MOVE_END: number = 2082;

/** A search is a short keyword; anything longer is a key held down. */
export const EMOJI_QUERY_MAX_LENGTH: number = 32;

export interface EmojiPanelKey {
  readonly keyCode: number;
  readonly unicodeChar: number;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly logoKey: boolean;
}

export enum EmojiPanelKeyAction {
  /** Not the panel's: the editor gets it. */
  NONE = 0,
  /** Claimed; `index` is the selection afterwards, which may be where it already was. */
  MOVE = 1,
  /** Insert the item at `index`. */
  ACTIVATE = 2,
  /** Put the panel away. */
  CLOSE = 3,
  /** The search now reads `query`. */
  SEARCH = 4,
}

export interface EmojiPanelKeyDecision {
  readonly action: EmojiPanelKeyAction;
  readonly index: number;
  readonly query: string;
}

function decision(
  action: EmojiPanelKeyAction,
  index: number,
  query: string,
): EmojiPanelKeyDecision {
  return { action: action, index: index, query: query };
}

export class EmojiPanelKeyPolicy {
  /**
   * `count` is how many items the panel shows now (the search results while searching), `columns` how many sit in one row, and `selected` the current selection.
   */
  static decide(
    key: EmojiPanelKey,
    count: number,
    columns: number,
    selected: number,
    query: string,
  ): EmojiPanelKeyDecision {
    const none: EmojiPanelKeyDecision = decision(EmojiPanelKeyAction.NONE, selected, query);
    if (key.ctrlKey || key.altKey || key.logoKey) {
      return none;
    }
    if (key.keyCode === KEYCODE_ESCAPE) {
      // Windows steps back one level before it lets the panel go: a detail page returns to Home. Here the level to step back from is the search, since the tabs are all one level.
      return query.length > 0
        ? decision(EmojiPanelKeyAction.SEARCH, 0, "")
        : decision(EmojiPanelKeyAction.CLOSE, selected, query);
    }
    if (key.keyCode === KEYCODE_DEL) {
      if (query.length === 0) {
        return none;
      }
      const characters: string[] = Array.from(query);
      characters.pop();
      return decision(EmojiPanelKeyAction.SEARCH, 0, characters.join(""));
    }
    const last: number = count - 1;
    const current: number = KeyboardGeometry.bounded(selected, 0, Math.max(last, 0));
    const width: number = Math.max(columns, 1);
    if (EmojiPanelKeyPolicy.isNavigation(key.keyCode)) {
      if (count <= 0) {
        return none;
      }
      let next: number = current;
      if (key.keyCode === KEYCODE_DPAD_LEFT) {
        next = Math.max(current - 1, 0);
      } else if (key.keyCode === KEYCODE_DPAD_RIGHT) {
        next = Math.min(current + 1, last);
      } else if (key.keyCode === KEYCODE_DPAD_UP) {
        next = current >= width ? current - width : 0;
      } else if (key.keyCode === KEYCODE_DPAD_DOWN) {
        next = Math.min(current + width, last);
      } else if (key.keyCode === KEYCODE_MOVE_HOME) {
        next = 0;
      } else {
        next = last;
      }
      return decision(EmojiPanelKeyAction.MOVE, next, query);
    }
    if (
      key.keyCode === KEYCODE_ENTER ||
      key.keyCode === KEYCODE_NUMPAD_ENTER ||
      key.keyCode === KEYCODE_SPACE
    ) {
      return count > 0 ? decision(EmojiPanelKeyAction.ACTIVATE, current, query) : none;
    }
    if (key.unicodeChar > 0x20 && key.unicodeChar !== 0x7f) {
      if (Array.from(query).length >= EMOJI_QUERY_MAX_LENGTH) {
        return decision(EmojiPanelKeyAction.MOVE, current, query);
      }
      return decision(EmojiPanelKeyAction.SEARCH, 0, query + String.fromCodePoint(key.unicodeChar));
    }
    return none;
  }

  /**
   * Whether an item answers a search, the rule the Windows panel and the shared panel (`matchesEmojiItem`) both use: the keyword list without regard to case, or the item's own text as typed.
   */
  static matches(text: string, keywords: string, query: string): boolean {
    if (query.length === 0) {
      return true;
    }
    return text.includes(query) || keywords.toLowerCase().includes(query.toLowerCase());
  }

  private static isNavigation(keyCode: number): boolean {
    return (
      keyCode === KEYCODE_DPAD_LEFT ||
      keyCode === KEYCODE_DPAD_RIGHT ||
      keyCode === KEYCODE_DPAD_UP ||
      keyCode === KEYCODE_DPAD_DOWN ||
      keyCode === KEYCODE_MOVE_HOME ||
      keyCode === KEYCODE_MOVE_END
    );
  }
}
