/**
 * Automatic capitalization rules, ported from
 * platforms/android/java/app/msime/android/EnglishCapitalizationPolicy.java.
 *
 * Java asks Character.isLetterOrDigit and Character.isWhitespace || isSpaceChar. The equivalents here
 * are Unicode property escapes: \p{L}\p{N} for the first, and \s for the second, which covers the
 * union of those two Java predicates including the no-break space.
 */
export enum CapitalizationMode {
  NONE = "none",
  WORDS = "words",
  SENTENCES = "sentences",
  ALL_CHARACTERS = "all_characters",
}

const LETTER_OR_DIGIT = /[\p{L}\p{N}]/u;
const WHITESPACE = /\s/u;

/** The apostrophe forms that keep a word going rather than starting a new one. */
function isApostrophe(codePoint: number): boolean {
  return codePoint === 0x27 || codePoint === 0x2019;
}

function isClosing(codePoint: number): boolean {
  return (
    codePoint === 0x27 ||
    codePoint === 0x22 ||
    codePoint === 0x2019 ||
    codePoint === 0x201d ||
    codePoint === 0x29 ||
    codePoint === 0x5d ||
    codePoint === 0x7d
  );
}

function isSentenceTerminator(codePoint: number): boolean {
  return (
    codePoint === 0x2e ||
    codePoint === 0x21 ||
    codePoint === 0x3f ||
    codePoint === 0x3002 ||
    codePoint === 0xff01 ||
    codePoint === 0xff1f
  );
}

/** Mirrors Character.codePointBefore: reads one full code point ending at the given offset. */
function codePointBefore(text: string, offset: number): number {
  const low: number = text.charCodeAt(offset - 1);
  if (low >= 0xdc00 && low <= 0xdfff && offset >= 2) {
    const high: number = text.charCodeAt(offset - 2);
    if (high >= 0xd800 && high <= 0xdbff) {
      return (high - 0xd800) * 0x400 + (low - 0xdc00) + 0x10000;
    }
  }
  return low;
}

function charCount(codePoint: number): number {
  return codePoint >= 0x10000 ? 2 : 1;
}

function matches(pattern: RegExp, codePoint: number): boolean {
  return pattern.test(String.fromCodePoint(codePoint));
}

export class EnglishCapitalizationPolicy {
  static shouldShift(mode: CapitalizationMode, contextBeforeInput: string | null): boolean {
    switch (mode) {
      case CapitalizationMode.NONE:
        return false;
      case CapitalizationMode.ALL_CHARACTERS:
        return true;
      case CapitalizationMode.WORDS:
        return EnglishCapitalizationPolicy.shouldShiftWords(contextBeforeInput);
      case CapitalizationMode.SENTENCES:
        return EnglishCapitalizationPolicy.shouldShiftSentences(contextBeforeInput);
      default:
        return false;
    }
  }

  private static shouldShiftWords(context: string | null): boolean {
    if (context === null) {
      return false;
    }
    if (context.length === 0) {
      return true;
    }
    const codePoint: number = codePointBefore(context, context.length);
    if (isApostrophe(codePoint)) {
      return false;
    }
    return !matches(LETTER_OR_DIGIT, codePoint);
  }

  private static shouldShiftSentences(context: string | null): boolean {
    if (context === null) {
      return false;
    }
    if (context.length === 0) {
      return true;
    }
    let offset: number = context.length;
    while (offset > 0) {
      const codePoint: number = codePointBefore(context, offset);
      offset -= charCount(codePoint);
      if (codePoint === 0x0a || codePoint === 0x0d) {
        return true;
      }
      if (matches(WHITESPACE, codePoint) || isClosing(codePoint)) {
        continue;
      }
      return isSentenceTerminator(codePoint);
    }
    return true;
  }
}
