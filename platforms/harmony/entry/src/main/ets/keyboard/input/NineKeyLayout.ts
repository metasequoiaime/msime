/**
 * Display labels and Engine inputs for the quanpin nine-key grid, ported from
 * platforms/android/java/app/msime/android/NineKeyLayout.java.
 *
 * The grid is data, not behaviour: which character each key sends is the Engine's contract, and the
 * labels are what the Apple hosts print for the same keys.
 */
export interface NineKey {
  readonly label: string;
  /** The ASCII character handed to the Engine, not the label. */
  readonly input: string;
  readonly description: string;
}

function key(label: string, input: string, description: string): NineKey {
  return { label: label, input: input, description: description };
}

const ROWS: NineKey[][] = [
  [key("分词", "'", "拼音分词"), key("ABC", "2", "2 ABC"), key("DEF", "3", "3 DEF")],
  [key("GHI", "4", "4 GHI"), key("JKL", "5", "5 JKL"), key("MNO", "6", "6 MNO")],
  [key("PQRS", "7", "7 PQRS"), key("TUV", "8", "8 TUV"), key("WXYZ", "9", "9 WXYZ")],
];

// The same three columns carrying digits instead of letter groups. A typist who chose a grid chose
// three columns, so the digit layer re-labels the grid rather than handing over to the twenty-six
// key face's ten-across symbol rows. The first cell sends 1 rather than the apostrophe it sends
// while spelling: on this layer it is a digit key like the other eight.
const DIGIT_ROWS: NineKey[][] = [
  [key("1", "1", "1"), key("2", "2", "2"), key("3", "3", "3")],
  [key("4", "4", "4"), key("5", "5", "5"), key("6", "6", "6")],
  [key("7", "7", "7"), key("8", "8", "8"), key("9", "9", "9")],
];

// ASCII, as the twenty-six key face sends: the Engine decides whether a comma arrives as , or as ，,
// and it is the only thing that knows, since the answer depends on the composing language and on the
// punctuation lock. iOS hard-codes the Chinese forms here because its grid only ever spells Chinese.
const PUNCTUATION: string[] = [",", ".", "?", "!"];

// What the sidebar offers on the digit layer, where there are no readings to show and the four
// sentence marks are already on the letter layer's sidebar.
const DIGIT_SIDEBAR: string[] = ["@", "#", "/", "-"];

export class NineKeyLayout {
  static rows(): NineKey[][] {
    return ROWS;
  }

  /** The grid as digits. `digits` rather than a boolean on `rows` so the caller reads as a face. */
  static digits(): NineKey[][] {
    return DIGIT_ROWS;
  }

  static punctuation(): string[] {
    return PUNCTUATION;
  }

  static digitPunctuation(): string[] {
    return DIGIT_SIDEBAR;
  }

  /** Literal choices behind a letter key: its printed digit, then each printed letter. */
  static holdOptions(key: NineKey): string[] {
    if (key.input < "2" || key.input > "9" || !/^[A-Z]{3,4}$/.test(key.label)) {
      return [];
    }
    return [key.input].concat(key.label.toLowerCase().split(""));
  }
}
