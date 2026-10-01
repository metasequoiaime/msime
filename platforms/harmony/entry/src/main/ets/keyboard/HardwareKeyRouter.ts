/**
 * What to do with a key that came from a real keyboard.
 *
 * On a phone every keystroke is a tap on a key this app drew, so it arrives already decided. On a
 * 2in1 the keys belong to the machine and reach the input method as hardware events, which have to
 * be claimed one at a time: anything not claimed is delivered to the application as if no input
 * method were running.
 *
 * Claiming too much is the failure that matters. An input method that swallows every key makes
 * shortcuts, arrows and passwords stop working, so the rule here is to claim a key only when there
 * is a composition it belongs to, or when it is the letter that would start one.
 *
 * Pure decision, no side effects, so it can be tested without a device or a session.
 */

import { SchemeTraits } from "./SchemeTraits";

/** The subset of the multimodal key event this decision needs. */
export interface HardwareKey {
  readonly keyCode: number;
  /** Resolved by the system, except that a letter's case is not to be trusted: see normalizeLetterCase. */
  readonly unicodeChar: number;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly logoKey: boolean;
  readonly shiftKey: boolean;
}

/** What the host should do with the key. Anything but Release is also a claim on the key. */
export enum HardwareKeyAction {
  /** Not ours: hand it to the application untouched. */
  RELEASE,
  /** A letter or digit for the Engine to spell with. */
  COMPOSE,
  /** An ASCII punctuation mark to resolve with the editor-context policy. */
  PUNCTUATION,
  /** Take back the last letter of the composition. */
  BACKSPACE,
  /** Throw the composition away. */
  CANCEL,
  /** Commit the highlighted candidate. */
  COMMIT,
  /** Commit the letters as typed, without choosing a candidate. */
  COMMIT_RAW,
  /** Commit the highlighted candidate's translation when Ctrl+Enter requests it. */
  COMMIT_TRANSLATION,
  /** Choose the candidate at `index`. */
  SELECT,
  /** Commit the first Han character from the highlighted candidate. */
  WORD_CHARACTER_FIRST,
  /** Commit the last Han character from the highlighted candidate. */
  WORD_CHARACTER_LAST,
  /** Consume a disabled navigation binding without turning it into text. */
  IGNORED,
  NEXT_PAGE,
  PREVIOUS_PAGE,
  NEXT_CANDIDATE,
  PREVIOUS_CANDIDATE,
  MOVE_LEFT,
  MOVE_RIGHT,
  MOVE_HOME,
  MOVE_END,
  DELETE_FORWARD,
  BACKSPACE_SEGMENT,
  MOVE_LEFT_SEGMENT,
  MOVE_RIGHT_SEGMENT,
  /** Delete the candidate at `index` from the user dictionary, the Windows maintenance chord. */
  REMOVE_CANDIDATE,
  /** Throw away the Engine's candidate cache for this session, the other Windows maintenance key. */
  RESET_CACHE,
  /** Start or advance Japanese conversion without committing it. */
  JAPANESE_CONVERT,
  /** Commit converted Japanese, or the kana reading when conversion never started. */
  JAPANESE_COMMIT,
  /** Type the printable ASCII `character` as its fullwidth form, the Windows double-byte mode. */
  WIDEN,
  /** Finish the composition, then type `character` after it: a key that carries text but means nothing to the composition. */
  COMMIT_THEN_TYPE,
  /** Korean: commit the open syllable, then hand the key to the application to do its own work (Return, a caret key, Delete, Tab). */
  COMMIT_THEN_RELEASE,
  /** Korean: list the Hanja of the composing syllable, or close the open list (MSIME_CONVERT_HANJA). */
  CONVERT_HANJA,
}

export interface HardwareKeyDecision {
  readonly action: HardwareKeyAction;
  /** The character to compose with, for COMPOSE. */
  readonly character: number;
  /** Which candidate to take, for SELECT. */
  readonly index: number;
}

/** The shared Windows-compatible candidate navigation bindings. */
export interface HardwareNavigationPreferences {
  readonly minusEqual: boolean;
  readonly commaPeriod: boolean;
  readonly brackets: boolean;
  readonly tab: boolean;
  readonly pageUpDown: boolean;
  readonly mouseWheel: boolean;
  readonly arrows: boolean;
}

/**
 * What is being spelled, for the keys whose meaning depends on it.
 *
 * `'`, `;`, the digits and `+` are punctuation or candidate picks for most compositions and part of the spelling for a few. The Windows host decides per key (`IsManualPinyinSeparatorKey`, `IsMicrosoftShuangpinIngKey`, the U-mode digits), and so does the Linux host's `candidate_input`; the rules below are theirs.
 */
export interface HardwareSpelling {
  /** The Engine's `local_mode`, 'none' outside a local utility mode. */
  readonly localMode: string;
  /** The Engine's `editing_text`. */
  readonly editing: string;
  /** Index into `editing`, which is ASCII. */
  readonly caret: number;
  readonly wubi: boolean;
  readonly microsoftShuangpin: boolean;
  /** Ctrl+Shift+E's English candidate mode, where the Engine spells letters only. */
  readonly englishCandidates: boolean;
  /** The Engine's `spelling_symbols`: the non-letter keys the active local mode takes as input, the digits in U mode and the digits and operators in V mode. */
  readonly spellingSymbols: string;
}

export const PLAIN_SPELLING: HardwareSpelling = {
  localMode: "none",
  editing: "",
  caret: 0,
  wubi: false,
  microsoftShuangpin: false,
  englishCandidates: false,
  spellingSymbols: "",
};

const KEYCODE_SPACE: number = 2050;
const KEYCODE_ENTER: number = 2054;
const KEYCODE_DEL: number = 2055;
const KEYCODE_ESCAPE: number = 2070;
const KEYCODE_NUMPAD_ENTER: number = 2119;
const KEYCODE_DPAD_UP: number = 2012;
const KEYCODE_DPAD_DOWN: number = 2013;
const KEYCODE_DPAD_LEFT: number = 2014;
const KEYCODE_DPAD_RIGHT: number = 2015;
const KEYCODE_TAB: number = 2049;
const KEYCODE_COMMA: number = 2043;
const KEYCODE_PERIOD: number = 2044;
const KEYCODE_MINUS: number = 2057;
const KEYCODE_EQUALS: number = 2058;
const KEYCODE_LEFT_BRACKET: number = 2059;
const KEYCODE_RIGHT_BRACKET: number = 2060;
const KEYCODE_PAGE_UP: number = 2068;
const KEYCODE_PAGE_DOWN: number = 2069;
const KEYCODE_FORWARD_DEL: number = 2071;
const KEYCODE_MOVE_HOME: number = 2081;
const KEYCODE_MOVE_END: number = 2082;
const KEYCODE_F9: number = 2098;
// KEYCODE_HANJA in @ohos.multimodalInput.keyCode: the Hanja key of a Korean keyboard, which KeyIdPolicy counts as Lang2.
const KEYCODE_HANJA: number = 2614;
// The number row. Matched by key rather than by the resolved character: with Ctrl+Shift+Alt held
// the system resolves nothing useful, and Shift alone would already have turned 1 into '!'.
const KEYCODE_1: number = 2001;
const KEYCODE_8: number = 2008;
const KEYCODE_9: number = 2009;
const KEYCODE_C: number = 2019;
// The numeric keypad. A keypad digit is the same digit, and a 2in1 keyboard that has one is exactly
// the machine whose user reaches for it: the digits are the whole point of these shortcuts, and
// someone with a keypad should not be told the feature is missing. Windows normalises the keypad in
// one place (`normalize_numpad_digit_key`) so every digit path gets it at once; this is that place.
const KEYCODE_NUMPAD_0: number = 2103;
const KEYCODE_NUMPAD_9: number = 2112;
const KEYCODE_NUMPAD_DOT: number = 2114;
const FULL_STOP: number = 0x2e;
const KEYCODE_0: number = 2000;
const DIGIT_ZERO: number = 0x30;
const KEYCODE_SEMICOLON: number = 2062;
const KEYCODE_APOSTROPHE: number = 2063;
const PLUS: number = 0x2b;
const SEMICOLON: number = 0x3b;
const APOSTROPHE: number = 0x27;
// Local modes whose own spellings are split with `'`, as the Linux host's `accepted_apostrophe` lists them.
const APOSTROPHE_LOCAL_MODES: string[] = ["emoji", "kaomoji", "temporary_japanese"];

// Keys that end an open Korean syllable and then do their own work in the application.
const KOREAN_COMMIT_KEYS: number[] = [
  KEYCODE_ENTER,
  KEYCODE_NUMPAD_ENTER,
  KEYCODE_TAB,
  KEYCODE_DEL,
  KEYCODE_FORWARD_DEL,
  KEYCODE_DPAD_UP,
  KEYCODE_DPAD_DOWN,
  KEYCODE_DPAD_LEFT,
  KEYCODE_DPAD_RIGHT,
  KEYCODE_MOVE_HOME,
  KEYCODE_MOVE_END,
  KEYCODE_PAGE_UP,
  KEYCODE_PAGE_DOWN,
];

const RELEASE: HardwareKeyDecision = {
  action: HardwareKeyAction.RELEASE,
  character: 0,
  index: 0,
};

function decision(
  action: HardwareKeyAction,
  character: number = 0,
  index: number = 0,
): HardwareKeyDecision {
  return { action: action, character: character, index: index };
}

function isAsciiPunctuation(value: number): boolean {
  return (
    (value >= 0x21 && value <= 0x2f) ||
    (value >= 0x3a && value <= 0x40) ||
    (value >= 0x5b && value <= 0x60) ||
    (value >= 0x7b && value <= 0x7e)
  );
}

export class HardwareKeyRouter {
  /**
   * A keypad digit read as the number-row digit it is.
   *
   * Converted unconditionally, which is what the source does with `normalize_numpad_digit_key` and
   * rests on the same assumption: a keypad reports its digit codes only while NumLock is on, and
   * reports navigation codes otherwise, so there is no state in which a digit code means Home. If a
   * device disagreed the digit would win, which is the same way the source would be wrong.
   */
  static normalizeNumpad(key: HardwareKey): HardwareKey {
    if (key.keyCode < KEYCODE_NUMPAD_0 || key.keyCode > KEYCODE_NUMPAD_9) {
      return key;
    }
    const digit: number = key.keyCode - KEYCODE_NUMPAD_0;
    return {
      keyCode: KEYCODE_0 + digit,
      unicodeChar: key.unicodeChar === 0 ? DIGIT_ZERO + digit : key.unicodeChar,
      ctrlKey: key.ctrlKey,
      altKey: key.altKey,
      shiftKey: key.shiftKey,
      logoKey: key.logoKey,
    };
  }

  /**
   * A letter's case decided by the modifiers rather than by `unicodeChar`.
   *
   * A 2in1 reports an upper-case `unicodeChar` for a plain letter key, with neither shift nor caps lock down. The Engine reads an upper-case letter as a help code, which only means something mid-composition, so every first letter typed on a hardware keyboard was declined and the composition never started. Shift and caps lock are both on the event, so the case is rebuilt from them, as the Windows host does from the virtual key and the keyboard state.
   */
  static normalizeLetterCase(key: HardwareKey, capsLock: boolean): HardwareKey {
    const lower: number = key.unicodeChar | 0x20;
    if (lower < 0x61 || lower > 0x7a) {
      return key;
    }
    return {
      keyCode: key.keyCode,
      unicodeChar: key.shiftKey !== capsLock ? lower - 0x20 : lower,
      ctrlKey: key.ctrlKey,
      altKey: key.altKey,
      shiftKey: key.shiftKey,
      logoKey: key.logoKey,
    };
  }

  /**
   * Whether the Engine is holding a composition: a reading, or a phrase piece already chosen. A Ctrl+Backspace that empties the reading of a half-chosen phrase leaves the chosen piece in the composition with no reading, as the Windows `keep_creating_word_after_empty_raw` does, and the next Backspace or Ctrl+Backspace edits it; a test on the reading alone would hand those keys, Enter and Escape to the editor while the preview still shows that piece.
   */
  static composing(editing: string, phrasePrefix: string): boolean {
    return editing.length > 0 || phrasePrefix.length > 0;
  }

  /**
   * @param composing whether the Engine is holding a composition right now, as `composing` answers it
   * @param chinese whether the Engine would spell with a letter rather than pass it through
   * @param numberRowSelection the shared `number_row_selection` preference: true, its default, has 1 through 9 pick a candidate off the visible page; false gives the number row back to the application, a digit then ending the composition and being typed after it
   * @param chinesePunctuationInEnglish whether punctuation is still the keyboard's in English mode, which the Windows host does when `punctuation_lock` is Chinese (`ResolvePunctuationOpen`)
   * @param korean whether letters go to the Korean Hangul automaton, which routes on rules of its own; see routeKorean
   * @param hanjaList whether the scheme's openable candidate list is open: the composing Korean syllable's Hanja, or the Zhuyin list
   * @param scheme the Engine scheme number whose own key rules are in force, or -1 while English or a local mode takes the keys; Zhuyin and Vietnamese route on rules of their own, see routeZhuyin and routeKorean
   */
  static route(
    key: HardwareKey,
    composing: boolean,
    chinese: boolean,
    numberRowSelection: boolean = true,
    navigation: HardwareNavigationPreferences = {
      minusEqual: true,
      commaPeriod: true,
      brackets: false,
      tab: true,
      pageUpDown: true,
      mouseWheel: false,
      arrows: true,
    },
    hasHighlightedTranslation: boolean = false,
    japanese: boolean = false,
    wordCharacter: string = "disabled",
    hasHighlightedCandidate: boolean = false,
    spelling: HardwareSpelling = PLAIN_SPELLING,
    fullWidth: boolean = false,
    chinesePunctuationInEnglish: boolean = false,
    korean: boolean = false,
    hanjaList: boolean = false,
    scheme: number = -1,
  ): HardwareKeyDecision {
    // Applied once, before anything reads the key, so no digit path can be left out of it. The
    // resolved character is filled in as well as the code: with Ctrl+Shift+Alt held the system
    // resolves nothing, which is why the chord matches on the code, but candidate selection reads
    // the character and a keypad digit does not always carry one.
    key = HardwareKeyRouter.normalizeNumpad(key);
    if (korean || scheme === SchemeTraits.VIETNAMESE) {
      return HardwareKeyRouter.routeKorean(
        key,
        composing,
        hanjaList,
        numberRowSelection,
        navigation,
        spelling,
        korean,
      );
    }
    if (scheme === SchemeTraits.ZHUYIN) {
      const zhuyinDecision: HardwareKeyDecision | undefined = HardwareKeyRouter.routeZhuyin(
        key,
        composing,
        hanjaList,
        numberRowSelection,
        navigation,
        spelling,
      );
      if (zhuyinDecision !== undefined) {
        return zhuyinDecision;
      }
    }
    // Japanese romaji reserves an unmodified minus for the long-vowel mark. It is a composition key even before the first kana exists. '=' and shifted '-' are never paging keys in Japanese (`IsJapaneseDisabledPagingKey` on Windows): mid-composition they are punctuation that commits the highlighted candidate first, and with nothing composed they are the application's.
    if (
      japanese &&
      !key.ctrlKey &&
      !key.altKey &&
      !key.logoKey &&
      key.keyCode === KEYCODE_MINUS &&
      !key.shiftKey &&
      key.unicodeChar === 0x2d
    ) {
      return decision(HardwareKeyAction.COMPOSE, 0x2d);
    }
    // The Windows maintenance chord: Ctrl+Shift+Alt+1..8 deletes the candidate in that slot from
    // the user dictionary. On a 2in1 it is the only way to reach that action, the long press the
    // touch keyboard uses being a gesture a hardware keyboard has no equivalent for. Whether the
    // slot exists and whether its candidate may be deleted at all are the caller's to check; this
    // only says which key was pressed.
    if (
      composing &&
      key.ctrlKey &&
      key.shiftKey &&
      key.altKey &&
      !key.logoKey &&
      key.keyCode >= KEYCODE_1 &&
      key.keyCode <= KEYCODE_8
    ) {
      return decision(HardwareKeyAction.REMOVE_CANDIDATE, 0, key.keyCode - KEYCODE_1);
    }
    // The cache belongs to the session rather than to a composition, so unlike the slot keys this
    // one answers whether or not something is being spelled — which is also when a stale candidate
    // list is most likely to be what the user is staring at.
    if (key.ctrlKey && key.shiftKey && key.altKey && !key.logoKey && key.keyCode === KEYCODE_C) {
      return decision(HardwareKeyAction.RESET_CACHE);
    }
    // Windows reserves Ctrl+Backspace/Left/Right for editing one Engine segment at a time. Other
    // modifier chords belong to the application, even in the middle of a composition.
    if (composing && key.ctrlKey && !key.altKey && !key.logoKey && !key.shiftKey) {
      // The Windows host claims Ctrl+Enter whenever candidates are up and answers `NavigationIgnored` when there is no translation to commit (`HandleTranslationCommitKey`), so the key is eaten rather than reaching an application that sends a message on it with the spelling still open.
      if (key.keyCode === KEYCODE_ENTER || key.keyCode === KEYCODE_NUMPAD_ENTER) {
        return decision(
          hasHighlightedTranslation
            ? HardwareKeyAction.COMMIT_TRANSLATION
            : HardwareKeyAction.IGNORED,
        );
      }
      if (key.keyCode === KEYCODE_DEL) {
        return decision(HardwareKeyAction.BACKSPACE_SEGMENT);
      }
      if (key.keyCode === KEYCODE_DPAD_LEFT) {
        return decision(HardwareKeyAction.MOVE_LEFT_SEGMENT);
      }
      if (key.keyCode === KEYCODE_DPAD_RIGHT) {
        return decision(HardwareKeyAction.MOVE_RIGHT_SEGMENT);
      }
    }
    if (key.ctrlKey || key.altKey || key.logoKey) {
      return RELEASE;
    }
    // The keypad's decimal point is always an ASCII '.', which is what the Windows host does with `VK_DECIMAL` (`KeyHandler.cpp`, "Numpad decimal should always commit ASCII '.'"): someone typing a number on the keypad wants 3.14, not 3。14. Mid-composition it finishes the composition first, as that host does.
    if (key.keyCode === KEYCODE_NUMPAD_DOT) {
      // Unless the composition spells with it: in V mode the keypad's point is the decimal point of the number being typed.
      if (composing && HardwareKeyRouter.spells(spelling, FULL_STOP)) {
        return decision(HardwareKeyAction.COMPOSE, FULL_STOP);
      }
      return composing ? decision(HardwareKeyAction.COMMIT_THEN_TYPE, FULL_STOP) : RELEASE;
    }
    if (composing) {
      if (japanese && key.keyCode === KEYCODE_SPACE) {
        return decision(HardwareKeyAction.JAPANESE_CONVERT);
      }
      if (japanese && (key.keyCode === KEYCODE_ENTER || key.keyCode === KEYCODE_NUMPAD_ENTER)) {
        return decision(HardwareKeyAction.JAPANESE_COMMIT);
      }
      if (key.keyCode === KEYCODE_DEL) {
        return decision(HardwareKeyAction.BACKSPACE);
      }
      if (key.keyCode === KEYCODE_FORWARD_DEL) {
        return decision(HardwareKeyAction.DELETE_FORWARD);
      }
      if (key.keyCode === KEYCODE_MOVE_HOME) {
        return decision(HardwareKeyAction.MOVE_HOME);
      }
      if (key.keyCode === KEYCODE_MOVE_END) {
        return decision(HardwareKeyAction.MOVE_END);
      }
      if (key.keyCode === KEYCODE_DPAD_LEFT) {
        return decision(HardwareKeyAction.MOVE_LEFT);
      }
      if (key.keyCode === KEYCODE_DPAD_RIGHT) {
        return decision(HardwareKeyAction.MOVE_RIGHT);
      }
      if (key.keyCode === KEYCODE_ESCAPE) {
        return decision(HardwareKeyAction.CANCEL);
      }
      // Ahead of the word-character keys, navigation and the number row, all of which would otherwise claim these keys: Shift+= is the `+` of `U+`, a digit in U or V mode is part of the input rather than a pick, and V mode's `-` and `.` are an operator and a decimal point rather than paging keys.
      const spellingDecision: HardwareKeyDecision | undefined = HardwareKeyRouter.spellingKey(
        key,
        spelling,
      );
      if (spellingDecision !== undefined) {
        return spellingDecision;
      }
      if (hasHighlightedCandidate && !key.shiftKey && wordCharacter === "brackets") {
        if (key.keyCode === KEYCODE_LEFT_BRACKET) {
          return decision(HardwareKeyAction.WORD_CHARACTER_FIRST);
        }
        if (key.keyCode === KEYCODE_RIGHT_BRACKET) {
          return decision(HardwareKeyAction.WORD_CHARACTER_LAST);
        }
      }
      if (hasHighlightedCandidate && !key.shiftKey && wordCharacter === "minus_equal") {
        if (key.keyCode === KEYCODE_MINUS) {
          return decision(HardwareKeyAction.WORD_CHARACTER_FIRST);
        }
        if (key.keyCode === KEYCODE_EQUALS) {
          return decision(HardwareKeyAction.WORD_CHARACTER_LAST);
        }
      }
      if (key.keyCode === KEYCODE_SPACE) {
        return decision(HardwareKeyAction.COMMIT);
      }
      if (key.keyCode === KEYCODE_ENTER || key.keyCode === KEYCODE_NUMPAD_ENTER) {
        // Enter on an open composition means "these letters, as I typed them" — the same thing the
        // 重输-adjacent raw commit means on the touch keyboard. The editor's own return action is
        // what Enter does when nothing is being composed, and that is the untouched path below.
        return decision(HardwareKeyAction.COMMIT_RAW);
      }
      const japanesePunctuation: boolean =
        japanese && (key.keyCode === KEYCODE_MINUS || key.keyCode === KEYCODE_EQUALS);
      const navigationDecision: HardwareKeyDecision | undefined = japanesePunctuation
        ? undefined
        : HardwareKeyRouter.navigation(key, navigation);
      if (navigationDecision !== undefined) {
        return navigationDecision;
      }
      // 1 through 9 pick a candidate off the strip while something is being spelled, which is what
      // the number row is for on every desktop input method.
      if (key.unicodeChar >= 0x31 && key.unicodeChar <= 0x39) {
        if (!numberRowSelection) {
          return decision(HardwareKeyAction.COMMIT_THEN_TYPE, key.unicodeChar);
        }
        return decision(HardwareKeyAction.SELECT, 0, key.unicodeChar - 0x31);
      }
    }
    // Only letters start a composition. A digit or a punctuation mark on an empty composition is
    // just that character, and the application can insert it without us in the way.
    const letter: boolean =
      (key.unicodeChar >= 0x61 && key.unicodeChar <= 0x7a) ||
      (key.unicodeChar >= 0x41 && key.unicodeChar <= 0x5a);
    if (!letter) {
      // A Chinese hardware keyboard owns punctuation, composing or not, just as the touch keyboard does. Mid-composition the mark ends it: the shared runtime finishes with the highlighted candidate before translating the mark, which is what the source does in `IsCommitWithHighlightedCandidatePunctuationInCandidateMode`. Releasing it instead left the composition open and dropped the mark into the editor ahead of the letters still being spelled. Navigation punctuation has already been consumed above while composing, so the keys a preference turned into paging keys still page; modifiers remain application-owned. Japanese punctuation is the application's only while nothing is composed: mid-composition it commits the highlighted candidate first, as the source's list of such marks includes Japanese '=', '_' and '+'.
      if (
        (((chinese || chinesePunctuationInEnglish) && !japanese) || (japanese && composing)) &&
        isAsciiPunctuation(key.unicodeChar)
      ) {
        return decision(HardwareKeyAction.PUNCTUATION, key.unicodeChar);
      }
      // Any other character the composition has no use for — a 0, a digit when the number row does not pick — still ends it first. Released as it was, the application inserted the digit while the letters were still open and the commit that followed landed after it: nihao then 0 gave 0你好. The Windows host finalizes the composition before the key goes on (`FUNCTION_FINALIZE_TEXTSTORE` in `IsVirtualKeyNeed`).
      if (composing && key.unicodeChar >= 0x20 && key.unicodeChar <= 0x7e) {
        return decision(HardwareKeyAction.COMMIT_THEN_TYPE, key.unicodeChar);
      }
      return HardwareKeyRouter.passThrough(key, composing, fullWidth);
    }
    // English mode spells nothing the application could not spell itself, so its letters are left
    // alone; the Engine is only worth interrupting for when it turns letters into something else.
    if (!composing && !chinese) {
      return HardwareKeyRouter.passThrough(key, composing, fullWidth);
    }
    return decision(HardwareKeyAction.COMPOSE, key.unicodeChar);
  }

  /**
   * The keys Zhuyin decides differently from the other Chinese schemes, or undefined for a key that takes the shared path.
   *
   * The Dachen layout puts bopomofo on digits and on `, . / ; -`, and the Engine lists the ones it takes in each state as its spelling symbols: with nothing composed the phonetic digits start a syllable rather than being typed, and while composing the tone digits and the phonetic marks spell rather than pick, page or end the composition. Space is tone 1 or opens the list, which the shared Space path already reaches through the select command, and the Shift marks (`<` gives ，) go through the shared punctuation path, which the Engine hands to the editor. The conversion has no caret inside it (`locks_caret`), so the caret keys and the segment chords commit it and then do their own work in the application, as for a Korean syllable. Down with the list closed opens it, libchewing's key for the list, whatever the arrow binding says, since there is no highlight for it to move yet; with the list open the keys reach it as the Korean Hanja list's do.
   */
  private static routeZhuyin(
    key: HardwareKey,
    composing: boolean,
    listOpen: boolean,
    numberRowSelection: boolean,
    navigation: HardwareNavigationPreferences,
    spelling: HardwareSpelling,
  ): HardwareKeyDecision | undefined {
    const modified: boolean = key.ctrlKey || key.altKey || key.logoKey;
    if (!composing) {
      // A phonetic digit; the phonetic marks already reach the Engine through the punctuation path, which hands a spelling symbol back to it as input.
      if (
        !modified &&
        HardwareKeyRouter.spells(spelling, key.unicodeChar) &&
        !isAsciiPunctuation(key.unicodeChar)
      ) {
        return decision(HardwareKeyAction.COMPOSE, key.unicodeChar);
      }
      return undefined;
    }
    if (key.ctrlKey && !key.altKey && !key.logoKey && !key.shiftKey) {
      if (
        key.keyCode === KEYCODE_DEL ||
        key.keyCode === KEYCODE_DPAD_LEFT ||
        key.keyCode === KEYCODE_DPAD_RIGHT
      ) {
        return decision(HardwareKeyAction.COMMIT_THEN_RELEASE);
      }
      return undefined;
    }
    if (modified) {
      return undefined;
    }
    if (listOpen) {
      const listDecision: HardwareKeyDecision | undefined = HardwareKeyRouter.routeHanjaList(
        key,
        numberRowSelection,
        navigation,
      );
      if (listDecision !== undefined) {
        return listDecision;
      }
    } else if (key.keyCode === KEYCODE_DPAD_DOWN && !key.shiftKey) {
      return decision(HardwareKeyAction.CONVERT_HANJA);
    }
    if (HardwareKeyRouter.spells(spelling, key.unicodeChar)) {
      return decision(HardwareKeyAction.COMPOSE, key.unicodeChar);
    }
    // Shift+1 is `!`, a mark that commits the conversion with it, not a pick from a list that is not open.
    if (key.shiftKey && key.keyCode >= KEYCODE_1 && key.keyCode <= KEYCODE_9) {
      return isAsciiPunctuation(key.unicodeChar)
        ? decision(HardwareKeyAction.PUNCTUATION, key.unicodeChar)
        : undefined;
    }
    if (
      key.keyCode === KEYCODE_DPAD_LEFT ||
      key.keyCode === KEYCODE_DPAD_RIGHT ||
      key.keyCode === KEYCODE_MOVE_HOME ||
      key.keyCode === KEYCODE_MOVE_END ||
      key.keyCode === KEYCODE_FORWARD_DEL
    ) {
      return decision(HardwareKeyAction.COMMIT_THEN_RELEASE);
    }
    return undefined;
  }

  /**
   * A key on the Korean Hangul automaton, which has no Chinese punctuation and no candidates to pick, page or navigate until the composing syllable's Hanja list opens.
   *
   * Vietnamese takes the same path with `korean` false: a word composes from its letters in the case they were typed, VNI's mark digits spell while a word is composing (the Engine lists them as spelling symbols), punctuation is ASCII, and every other key ends the word the way it ends a syllable. It has no list, so the Hanja key is not claimed.
   *
   * Letters always compose, in the case the caller normalized them to (Shift gives ㄲ ㄸ ㅃ ㅆ ㅉ ㅒ ㅖ). With nothing composed every other key is the application's, punctuation included: Korean writes it as half-width ASCII, so the application typing the key is exactly right, and fullwidth does not apply. With a syllable open, Backspace takes one jamo back and Escape discards the syllable; a punctuation mark goes through the Engine, which commits the syllable and the mark as one; Space and the other printable keys commit the syllable and are typed after it by the keyboard, so their order against the commit is not left to the editor; and Return, the caret keys, Delete, Tab and the page keys, with or without a modifier, commit the syllable and then do their own work in the application. Any other chord, and a modifier on its own, leaves the syllable open, as it does for every other scheme.
   *
   * The Hanja key (a Korean keyboard's own, or F9 as on the Linux and Android hosts) lists the composing syllable's Hanja and closes the list again; see routeHanjaList for the keys that reach the list while it is open. With nothing composed the Hanja key is the application's like every other key.
   */
  private static routeKorean(
    key: HardwareKey,
    composing: boolean,
    hanjaList: boolean,
    numberRowSelection: boolean,
    navigation: HardwareNavigationPreferences,
    spelling: HardwareSpelling = PLAIN_SPELLING,
    korean: boolean = true,
  ): HardwareKeyDecision {
    const character: number = key.unicodeChar;
    const modified: boolean = key.ctrlKey || key.altKey || key.logoKey;
    const letter: boolean =
      (character >= 0x61 && character <= 0x7a) || (character >= 0x41 && character <= 0x5a);
    if (!modified && letter) {
      return decision(HardwareKeyAction.COMPOSE, character);
    }
    if (!composing) {
      return RELEASE;
    }
    if (!modified && HardwareKeyRouter.spells(spelling, character)) {
      return decision(HardwareKeyAction.COMPOSE, character);
    }
    // Claimed while composing whatever the Engine answers: a lone jamo has no Hanja, and the key handed on would reach the editor beside a syllable still composing (msime_client.h).
    if (
      korean &&
      !modified &&
      !key.shiftKey &&
      (key.keyCode === KEYCODE_HANJA || key.keyCode === KEYCODE_F9)
    ) {
      return decision(HardwareKeyAction.CONVERT_HANJA);
    }
    if (hanjaList && !modified) {
      const listDecision: HardwareKeyDecision | undefined = HardwareKeyRouter.routeHanjaList(
        key,
        numberRowSelection,
        navigation,
      );
      if (listDecision !== undefined) {
        return listDecision;
      }
    }
    if (!modified) {
      if (key.keyCode === KEYCODE_DEL) {
        return decision(HardwareKeyAction.BACKSPACE);
      }
      if (key.keyCode === KEYCODE_ESCAPE) {
        return decision(HardwareKeyAction.CANCEL);
      }
      if (isAsciiPunctuation(character)) {
        return decision(HardwareKeyAction.PUNCTUATION, character);
      }
      if (character >= 0x20 && character <= 0x7e) {
        return decision(HardwareKeyAction.COMMIT_THEN_TYPE, character);
      }
    }
    if (KOREAN_COMMIT_KEYS.indexOf(key.keyCode) >= 0) {
      return decision(HardwareKeyAction.COMMIT_THEN_RELEASE);
    }
    return RELEASE;
  }

  /**
   * A key that means something to the open Hanja list of a Korean syllable, or undefined for one that keeps its plain Korean meaning.
   *
   * Space and Return choose the highlighted Hanja; Return sends the candidate command because only the session knows the highlight (msime_client.h). The number row picks from the visible page while `number_row_selection` is on; with it off a digit commits the syllable and is typed as it is without the list. Up and Down, the page keys and Tab move through the list as their navigation bindings say, and Left and Right move the highlight while the arrow binding is on, since a syllable has no caret inside it to move. A binding turned off leaves its key with its plain Korean meaning rather than eating it, and so do Home and End. The marks - = [ ] , . stay punctuation rather than paging or taking a character from a word: a Hanja is one character already, and the Engine closes the list and writes the Hangul with the mark, as every other host does with the list open. Backspace and Escape need nothing here: the Engine has them close the list and keep the syllable.
   */
  private static routeHanjaList(
    key: HardwareKey,
    numberRowSelection: boolean,
    navigation: HardwareNavigationPreferences,
  ): HardwareKeyDecision | undefined {
    const code: number = key.keyCode;
    if (code === KEYCODE_SPACE || code === KEYCODE_ENTER || code === KEYCODE_NUMPAD_ENTER) {
      return decision(HardwareKeyAction.COMMIT);
    }
    if (key.unicodeChar >= 0x31 && key.unicodeChar <= 0x39) {
      return numberRowSelection
        ? decision(HardwareKeyAction.SELECT, 0, key.unicodeChar - 0x31)
        : decision(HardwareKeyAction.COMMIT_THEN_TYPE, key.unicodeChar);
    }
    if (code === KEYCODE_DPAD_LEFT || code === KEYCODE_DPAD_RIGHT) {
      if (!navigation.arrows) {
        return undefined;
      }
      return decision(
        code === KEYCODE_DPAD_LEFT
          ? HardwareKeyAction.PREVIOUS_CANDIDATE
          : HardwareKeyAction.NEXT_CANDIDATE,
      );
    }
    if (
      code !== KEYCODE_DPAD_UP &&
      code !== KEYCODE_DPAD_DOWN &&
      code !== KEYCODE_PAGE_UP &&
      code !== KEYCODE_PAGE_DOWN &&
      code !== KEYCODE_TAB
    ) {
      return undefined;
    }
    const navigationDecision: HardwareKeyDecision | undefined = HardwareKeyRouter.navigation(
      key,
      navigation,
    );
    return navigationDecision === undefined ||
      navigationDecision.action === HardwareKeyAction.IGNORED
      ? undefined
      : navigationDecision;
  }

  /**
   * A key nothing above wants: the application's, unless fullwidth is on. Windows eats every printable ASCII key while the double-byte mode is on and there is no candidate list, and inserts its fullwidth form instead (`KeyEventSink.cpp` `IsDoubleSingleByte`, `' '` to `'~'`). Space becomes the ideographic space, as it does everywhere else fullwidth applies. Modifier chords were released before this is reached, so a Ctrl+C still copies.
   */
  private static passThrough(
    key: HardwareKey,
    composing: boolean,
    fullWidth: boolean,
  ): HardwareKeyDecision {
    if (fullWidth && !composing && key.unicodeChar >= 0x20 && key.unicodeChar <= 0x7e) {
      return decision(HardwareKeyAction.WIDEN, key.unicodeChar);
    }
    return RELEASE;
  }

  /** Whether the Engine takes `character` as input in this state. Never in the English candidate mode, which spells letters only. */
  private static spells(spelling: HardwareSpelling, character: number): boolean {
    return (
      !spelling.englishCandidates &&
      character > 0x20 &&
      character < 0x7f &&
      spelling.spellingSymbols.indexOf(String.fromCharCode(character)) >= 0
    );
  }

  private static spellingKey(
    key: HardwareKey,
    spelling: HardwareSpelling,
  ): HardwareKeyDecision | undefined {
    // An English word has no syllables, code points or shuangpin finals; the Engine takes letters only there, so these keys stay punctuation.
    if (spelling.englishCandidates) {
      return undefined;
    }
    if (spelling.spellingSymbols.length > 0) {
      // A local mode that spells with more than letters: U mode's hexadecimal digits, V mode's digits and operators. What the Engine lists is input, decided by the character the key typed, so V mode's Shift+9 is its `(`. Shift+1..9 picks otherwise, as on Windows, since the plain digits are taken.
      if (HardwareKeyRouter.spells(spelling, key.unicodeChar)) {
        return decision(HardwareKeyAction.COMPOSE, key.unicodeChar);
      }
      if (key.shiftKey && key.keyCode >= KEYCODE_1 && key.keyCode <= KEYCODE_9) {
        return decision(HardwareKeyAction.SELECT, 0, key.keyCode - KEYCODE_1);
      }
      // `U+1F600` as well as `u1f600`: the plus is only part of the spelling straight after the U.
      if (
        spelling.localMode === "unicode" &&
        key.unicodeChar === PLUS &&
        spelling.editing === "U"
      ) {
        return decision(HardwareKeyAction.COMPOSE, PLUS);
      }
      return undefined;
    }
    if (key.shiftKey) {
      return undefined;
    }
    // The manual syllable separator, `xi'an` rather than `xian`. Wubi codes have no syllables to separate, and at the very start there is nothing to separate yet.
    if (
      key.keyCode === KEYCODE_APOSTROPHE &&
      key.unicodeChar === APOSTROPHE &&
      spelling.caret > 0 &&
      ((spelling.localMode === "none" && !spelling.wubi) ||
        APOSTROPHE_LOCAL_MODES.indexOf(spelling.localMode) >= 0)
    ) {
      return decision(HardwareKeyAction.COMPOSE, APOSTROPHE);
    }
    // Microsoft shuangpin puts the `ing` final on `;`, so it is a letter exactly when it would be the second key of a syllable: an odd number of keys since the last separator.
    if (
      spelling.microsoftShuangpin &&
      key.keyCode === KEYCODE_SEMICOLON &&
      key.unicodeChar === SEMICOLON
    ) {
      const caret: number = Math.min(spelling.caret, spelling.editing.length);
      const separator: number = caret === 0 ? -1 : spelling.editing.lastIndexOf("'", caret - 1);
      if ((caret - (separator + 1)) % 2 === 1) {
        return decision(HardwareKeyAction.COMPOSE, SEMICOLON);
      }
    }
    return undefined;
  }

  private static navigation(
    key: HardwareKey,
    preferences: HardwareNavigationPreferences,
  ): HardwareKeyDecision | undefined {
    let previous: boolean = false;
    let enabled: boolean;
    if (key.keyCode === KEYCODE_DPAD_UP || key.keyCode === KEYCODE_DPAD_DOWN) {
      if (preferences.arrows) {
        return decision(
          key.keyCode === KEYCODE_DPAD_UP
            ? HardwareKeyAction.PREVIOUS_CANDIDATE
            : HardwareKeyAction.NEXT_CANDIDATE,
        );
      }
      enabled = false;
    } else if (key.keyCode === KEYCODE_PAGE_UP || key.keyCode === KEYCODE_PAGE_DOWN) {
      previous = key.keyCode === KEYCODE_PAGE_UP;
      enabled = preferences.pageUpDown;
    } else if (key.keyCode === KEYCODE_TAB) {
      previous = key.shiftKey;
      enabled = preferences.tab;
    } else if (key.keyCode === KEYCODE_MINUS || key.keyCode === KEYCODE_EQUALS) {
      previous = key.keyCode === KEYCODE_MINUS;
      enabled = preferences.minusEqual;
    } else if (key.keyCode === KEYCODE_COMMA || key.keyCode === KEYCODE_PERIOD) {
      previous = key.keyCode === KEYCODE_COMMA;
      enabled = preferences.commaPeriod;
    } else if (key.keyCode === KEYCODE_LEFT_BRACKET || key.keyCode === KEYCODE_RIGHT_BRACKET) {
      previous = key.keyCode === KEYCODE_LEFT_BRACKET;
      enabled = preferences.brackets;
    } else {
      return undefined;
    }
    return decision(
      enabled
        ? previous
          ? HardwareKeyAction.PREVIOUS_PAGE
          : HardwareKeyAction.NEXT_PAGE
        : HardwareKeyAction.IGNORED,
    );
  }
}
