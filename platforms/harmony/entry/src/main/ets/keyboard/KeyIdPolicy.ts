/**
 * Key ids for the per-key press counts the shared typing-statistics store keeps (`record_keys`), and the in-memory batch they wait in until they are written.
 *
 * The ids are W3C `KeyboardEvent.code` names plus a few on-screen-only ones. This list must equal `KEY_IDS` in `crates/client-core/src/typing_statistics.rs` exactly: the store rejects a whole batch that carries one id it does not know, so an id invented here would silently cost every press around it. A key with no mapping is not counted rather than filed under a guess.
 *
 * Only counts leave this file. No order, no timestamps and no text: a batch is a day and a count per key.
 */
export const KEY_IDS: readonly string[] = [
  "KeyA",
  "KeyB",
  "KeyC",
  "KeyD",
  "KeyE",
  "KeyF",
  "KeyG",
  "KeyH",
  "KeyI",
  "KeyJ",
  "KeyK",
  "KeyL",
  "KeyM",
  "KeyN",
  "KeyO",
  "KeyP",
  "KeyQ",
  "KeyR",
  "KeyS",
  "KeyT",
  "KeyU",
  "KeyV",
  "KeyW",
  "KeyX",
  "KeyY",
  "KeyZ",
  "Digit0",
  "Digit1",
  "Digit2",
  "Digit3",
  "Digit4",
  "Digit5",
  "Digit6",
  "Digit7",
  "Digit8",
  "Digit9",
  "Backquote",
  "Minus",
  "Equal",
  "BracketLeft",
  "BracketRight",
  "Backslash",
  "Semicolon",
  "Quote",
  "Comma",
  "Period",
  "Slash",
  "IntlBackslash",
  "IntlRo",
  "IntlYen",
  "Lang1",
  "Lang2",
  "Convert",
  "NonConvert",
  "KanaMode",
  "Space",
  "Enter",
  "Backspace",
  "Tab",
  "Escape",
  "Delete",
  "Insert",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "CapsLock",
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "Fn",
  "ContextMenu",
  "F1",
  "F2",
  "F3",
  "F4",
  "F5",
  "F6",
  "F7",
  "F8",
  "F9",
  "F10",
  "F11",
  "F12",
  "Numpad0",
  "Numpad1",
  "Numpad2",
  "Numpad3",
  "Numpad4",
  "Numpad5",
  "Numpad6",
  "Numpad7",
  "Numpad8",
  "Numpad9",
  "NumpadDecimal",
  "NumpadEnter",
  "NumpadAdd",
  "NumpadSubtract",
  "NumpadMultiply",
  "NumpadDivide",
  "NumLock",
  "Nine0",
  "Nine1",
  "Nine2",
  "Nine3",
  "Nine4",
  "Nine5",
  "Nine6",
  "Nine7",
  "Nine8",
  "Nine9",
  "SoftPunctuation",
  "SoftSymbol",
  "SoftLayer",
  "SoftLanguage",
  "SoftGlobe",
  "SoftEmoji",
  "SoftVoice",
  // 全拼 14 键的各键，按键面上的字母命名（`FourteenQW` 是 Q、W 合在一起的键）；不记到 `KeyQ` 这些 26 键字母上，否则会混进 26 键的热力图。
  "FourteenQW",
  "FourteenER",
  "FourteenTY",
  "FourteenUI",
  "FourteenOP",
  "FourteenAS",
  "FourteenDF",
  "FourteenGH",
  "FourteenJK",
  "FourteenL",
  "FourteenZX",
  "FourteenCV",
  "FourteenBN",
  "FourteenM",
];

const KNOWN: Set<string> = new Set<string>(KEY_IDS);

// The ANSI key that types each printable ASCII character, shifted or not, which is what a symbol-layer key on the soft keyboard is counted as.
const PUNCTUATION_KEYS: Map<string, string> = new Map<string, string>([
  [" ", "Space"],
  ["`", "Backquote"],
  ["~", "Backquote"],
  ["-", "Minus"],
  ["_", "Minus"],
  ["=", "Equal"],
  ["+", "Equal"],
  ["[", "BracketLeft"],
  ["{", "BracketLeft"],
  ["]", "BracketRight"],
  ["}", "BracketRight"],
  ["\\", "Backslash"],
  ["|", "Backslash"],
  [";", "Semicolon"],
  [":", "Semicolon"],
  ["'", "Quote"],
  ['"', "Quote"],
  [",", "Comma"],
  ["<", "Comma"],
  [".", "Period"],
  [">", "Period"],
  ["/", "Slash"],
  ["?", "Slash"],
  ["!", "Digit1"],
  ["@", "Digit2"],
  ["#", "Digit3"],
  ["$", "Digit4"],
  ["%", "Digit5"],
  ["^", "Digit6"],
  ["&", "Digit7"],
  ["*", "Digit8"],
  ["(", "Digit9"],
  [")", "Digit0"],
]);

// OpenHarmony `KeyCode` values (@ohos.multimodalInput.keyCode) for the keys a physical keyboard has a W3C name for. Codes without an equivalent in the shared list (media, browser, STAR, POUND, AT, PLUS, numpad comma, equals and parentheses, standalone katakana or hiragana) are left out and not counted.
const HARDWARE_KEYS: Map<number, string> = new Map<number, string>([
  [2012, "ArrowUp"],
  [2013, "ArrowDown"],
  [2014, "ArrowLeft"],
  [2015, "ArrowRight"],
  [2043, "Comma"],
  [2044, "Period"],
  [2045, "AltLeft"],
  [2046, "AltRight"],
  [2047, "ShiftLeft"],
  [2048, "ShiftRight"],
  [2049, "Tab"],
  [2050, "Space"],
  [2054, "Enter"],
  [2055, "Backspace"],
  [2056, "Backquote"],
  [2057, "Minus"],
  [2058, "Equal"],
  [2059, "BracketLeft"],
  [2060, "BracketRight"],
  [2061, "Backslash"],
  [2062, "Semicolon"],
  [2063, "Quote"],
  [2064, "Slash"],
  [2067, "ContextMenu"],
  [2068, "PageUp"],
  [2069, "PageDown"],
  [2070, "Escape"],
  [2071, "Delete"],
  [2072, "ControlLeft"],
  [2073, "ControlRight"],
  [2074, "CapsLock"],
  [2076, "MetaLeft"],
  [2077, "MetaRight"],
  [2078, "Fn"],
  [2081, "Home"],
  [2082, "End"],
  [2083, "Insert"],
  [2102, "NumLock"],
  [2113, "NumpadDivide"],
  [2114, "NumpadMultiply"],
  [2115, "NumpadSubtract"],
  [2116, "NumpadAdd"],
  [2117, "NumpadDecimal"],
  [2119, "NumpadEnter"],
  // JIS and ISO keys: 半角/全角 sits where Backquote does, 102ND is the extra key beside left Shift.
  [2601, "Backquote"],
  [2602, "IntlBackslash"],
  [2603, "IntlRo"],
  [2606, "Convert"],
  [2607, "KanaMode"],
  [2608, "NonConvert"],
  [2613, "Lang1"],
  [2614, "Lang2"],
  [2615, "IntlYen"],
]);

const KEYCODE_0: number = 2000;
const KEYCODE_A: number = 2017;
const KEYCODE_F1: number = 2090;
const KEYCODE_NUMPAD_0: number = 2103;

export class KeyIdPolicy {
  static readonly SPACE: string = "Space";
  static readonly ENTER: string = "Enter";
  static readonly BACKSPACE: string = "Backspace";
  static readonly SHIFT: string = "ShiftLeft";
  static readonly SOFT_PUNCTUATION: string = "SoftPunctuation";
  static readonly SOFT_SYMBOL: string = "SoftSymbol";
  static readonly SOFT_LAYER: string = "SoftLayer";
  static readonly SOFT_LANGUAGE: string = "SoftLanguage";
  static readonly SOFT_EMOJI: string = "SoftEmoji";
  static readonly SOFT_VOICE: string = "SoftVoice";

  /**
   * The key a soft twenty-six-key or symbol-layer key stands for, from the ASCII character on its cap: a letter is its letter key in either case, a digit its digit-row key, and a mark the ANSI key that types it shifted or not (`!` is Digit1). Anything else, a CJK mark included, has no key and is not counted.
   */
  static character(character: string): string | null {
    if (character.length !== 1) return null;
    const code: number = character.charCodeAt(0);
    if ((code >= 0x61 && code <= 0x7a) || (code >= 0x41 && code <= 0x5a)) {
      return `Key${character.toUpperCase()}`;
    }
    if (code >= 0x30 && code <= 0x39) return `Digit${character}`;
    return PUNCTUATION_KEYS.get(character) ?? null;
  }

  /**
   * 九键网格中的一格，按其上印的数字定位。字母层的第一格在 2in1 网格上发送音节分隔符而不是 1，在触摸网格上作为 @# 打开符号面板（Android 的 `KeyPressIds.forNineKeyDigit(1)`），无论哪种，都是印着 1 的那一格。
   */
  static nineKey(input: string): string | null {
    if (input === "'" || input === "@") return "Nine1";
    if (input.length === 1 && input >= "0" && input <= "9") return `Nine${input}`;
    return null;
  }

  /** 全拼 14 键的一键，按键面上的字母（`FourteenKey.letters`，如 `qw`、`l`）定位；不是 14 键的键面时不计。 */
  static fourteenKey(letters: string): string | null {
    const id: string = `Fourteen${letters.toUpperCase()}`;
    return KNOWN.has(id) ? id : null;
  }

  /**
   * A cell of the Japanese kana grid by its index in `JapaneseNineKeyLayout.keys()`, the same on the digit layer: あ through ら sit where 1 to 9 sit on a phone keypad, わ where 0 sits, and the 、。？！ cell beside it is side punctuation. The mapping iOS (`TypingKeyID.japaneseKana`) and Android (`KeyPressIds.forJapaneseKeyIndex`) use.
   */
  static japaneseKana(index: number): string | null {
    if (index >= 0 && index <= 8) return `Nine${index + 1}`;
    if (index === 9) return "Nine0";
    if (index === 10) return KeyIdPolicy.SOFT_PUNCTUATION;
    return null;
  }

  /** A physical key by its OpenHarmony key code. */
  static hardware(code: number): string | null {
    if (code >= KEYCODE_A && code < KEYCODE_A + 26) {
      return `Key${String.fromCharCode(0x41 + code - KEYCODE_A)}`;
    }
    if (code >= KEYCODE_0 && code <= KEYCODE_0 + 9) return `Digit${code - KEYCODE_0}`;
    if (code >= KEYCODE_F1 && code < KEYCODE_F1 + 12) return `F${code - KEYCODE_F1 + 1}`;
    if (code >= KEYCODE_NUMPAD_0 && code <= KEYCODE_NUMPAD_0 + 9) {
      return `Numpad${code - KEYCODE_NUMPAD_0}`;
    }
    return HARDWARE_KEYS.get(code) ?? null;
  }
}

/** One write: the local day the presses happened on and how many times each key went down. */
export interface KeyPressFlush {
  day: string;
  keys: Record<string, number>;
}

/**
 * Press counts held in memory until they are worth a write. The store takes a file lock and rewrites the whole document, so a write per key is out of the question.
 *
 * The day is the one each press happened on, never the day of the write: a press before midnight that is written after it still belongs to the day before, so a new day hands back the old day's counts before it starts its own.
 */
export class KeyPressBatch {
  /** Presses that trigger a write without waiting for the timer. */
  static readonly FLUSH_PRESSES: number = 256;
  /** How long counts may wait in memory, so a quiet session still lands within half a minute. */
  static readonly FLUSH_INTERVAL_MS: number = 30000;

  private day: string = "";
  private counts: Map<string, number> = new Map<string, number>();
  private presses: number = 0;

  /** Counts one press of `id` on `day`, and returns whatever is now due to be written, oldest day first. */
  add(id: string, day: string): KeyPressFlush[] {
    const due: KeyPressFlush[] = [];
    if (!KNOWN.has(id)) return due;
    if (this.presses > 0 && day !== this.day) {
      const previous: KeyPressFlush | null = this.drain();
      if (previous !== null) due.push(previous);
    }
    this.day = day;
    this.counts.set(id, (this.counts.get(id) ?? 0) + 1);
    this.presses += 1;
    if (this.presses >= KeyPressBatch.FLUSH_PRESSES) {
      const full: KeyPressFlush | null = this.drain();
      if (full !== null) due.push(full);
    }
    return due;
  }

  /** Everything held, emptied; null when nothing was. */
  drain(): KeyPressFlush | null {
    if (this.presses === 0) return null;
    const keys: Record<string, number> = {};
    this.counts.forEach((count: number, id: string): void => {
      keys[id] = count;
    });
    const flush: KeyPressFlush = { day: this.day, keys: keys };
    this.clear();
    return flush;
  }

  /** Drops everything held without writing it, for when recording was turned off. */
  clear(): void {
    this.counts = new Map<string, number>();
    this.presses = 0;
    this.day = "";
  }

  pending(): number {
    return this.presses;
  }
}

/**
 * Which physical keys are down, so a held key counts once. OpenHarmony repeats key-down while a key is held and its key event carries no repeat flag; the release is what makes the next key-down a new press.
 */
export class HeldKeys {
  private readonly down: Set<number> = new Set<number>();

  /** Whether this key-down is a new press rather than a repeat of one still held. */
  press(code: number): boolean {
    if (this.down.has(code)) return false;
    this.down.add(code);
    return true;
  }

  release(code: number): void {
    this.down.delete(code);
  }

  /** Forgets every key, for when the releases will not arrive: the editor or the keyboard went away. */
  reset(): void {
    this.down.clear();
  }
}
