/**
 * Privacy and size bounds for text-only clipboard history entries, ported from
 * platforms/android/java/app/msime/android/ClipboardHistoryPolicy.java.
 *
 * The character bound is measured in extended graphemes, matching the shared Rust store. The byte
 * bound is measured in UTF-8, which is what the store writes.
 */
import { utf8Length } from "../Utf8";

const MAX_CHARS: number = 10000;
const MAX_BYTES: number = 40000;

interface GraphemeSegment {
  readonly segment: string;
}

interface GraphemeSegmenter {
  segment(text: string): Iterable<GraphemeSegment>;
}

interface IntlWithSegmenter {
  Segmenter?: new (
    locales?: string | string[],
    options?: { readonly granularity?: string },
  ) => GraphemeSegmenter;
}

const INTL_WITH_SEGMENTER: IntlWithSegmenter =
  typeof Intl === "undefined" ? {} : (Intl as unknown as IntlWithSegmenter);
const MARK = /^\p{M}$/u;

function isRegionalIndicator(codePoint: number): boolean {
  return codePoint >= 0x1f1e6 && codePoint <= 0x1f1ff;
}

function isExtendedPictographic(codePoint: number): boolean {
  return (
    (codePoint >= 0x1f000 && codePoint <= 0x1faff) ||
    (codePoint >= 0x2600 && codePoint <= 0x27bf) ||
    codePoint === 0x00a9 ||
    codePoint === 0x00ae ||
    codePoint === 0x203c ||
    codePoint === 0x2049 ||
    codePoint === 0x2122 ||
    codePoint === 0x2139 ||
    (codePoint >= 0x2194 && codePoint <= 0x2199) ||
    (codePoint >= 0x21a9 && codePoint <= 0x21aa) ||
    (codePoint >= 0x2300 && codePoint <= 0x23ff) ||
    codePoint === 0x24c2 ||
    (codePoint >= 0x25aa && codePoint <= 0x25ff) ||
    (codePoint >= 0x2b00 && codePoint <= 0x2bff) ||
    codePoint === 0x3030 ||
    codePoint === 0x303d ||
    codePoint === 0x3297 ||
    codePoint === 0x3299
  );
}

function isExtend(codePoint: number, character: string): boolean {
  return (
    MARK.test(character) ||
    (codePoint >= 0x1f3fb && codePoint <= 0x1f3ff) ||
    (codePoint >= 0xe0020 && codePoint <= 0xe007f) ||
    codePoint === 0x200c
  );
}

function isControl(codePoint: number): boolean {
  return (
    (codePoint <= 0x001f && codePoint !== 0x000a && codePoint !== 0x000d) ||
    (codePoint >= 0x007f && codePoint <= 0x009f)
  );
}

function isPrepend(codePoint: number): boolean {
  return (
    (codePoint >= 0x0600 && codePoint <= 0x0605) ||
    codePoint === 0x06dd ||
    codePoint === 0x070f ||
    (codePoint >= 0x0890 && codePoint <= 0x0891) ||
    codePoint === 0x08e2 ||
    codePoint === 0x0d4e ||
    codePoint === 0x110bd ||
    codePoint === 0x110cd ||
    (codePoint >= 0x111c2 && codePoint <= 0x111c3) ||
    codePoint === 0x1193f ||
    codePoint === 0x11941 ||
    codePoint === 0x11a3a ||
    (codePoint >= 0x11a84 && codePoint <= 0x11a89) ||
    codePoint === 0x11d46
  );
}

function hangulClass(codePoint: number): "L" | "V" | "T" | "LV" | "LVT" | null {
  if (
    (codePoint >= 0x1100 && codePoint <= 0x115f) ||
    (codePoint >= 0xa960 && codePoint <= 0xa97c)
  ) {
    return "L";
  }
  if (
    (codePoint >= 0x1160 && codePoint <= 0x11a7) ||
    (codePoint >= 0xd7b0 && codePoint <= 0xd7c6)
  ) {
    return "V";
  }
  if (
    (codePoint >= 0x11a8 && codePoint <= 0x11ff) ||
    (codePoint >= 0xd7cb && codePoint <= 0xd7fb)
  ) {
    return "T";
  }
  if (codePoint >= 0xac00 && codePoint <= 0xd7a3) {
    return (codePoint - 0xac00) % 28 === 0 ? "LV" : "LVT";
  }
  return null;
}

/** Fallback for runtimes without Intl.Segmenter; follows the extended-grapheme break rules used by the shared store. */
function fallbackGraphemeCount(text: string): number {
  let count: number = 0;
  let previousCodePoint: number | null = null;
  let regionalIndicatorRun: number = 0;
  let extendedPictographicBeforeZwj: boolean = false;
  let joinAfterZwj: boolean = false;

  for (const character of text) {
    const codePoint: number = character.codePointAt(0) ?? 0;
    const currentExtend: boolean = isExtend(codePoint, character);
    const currentZwj: boolean = codePoint === 0x200d;
    const currentControl: boolean = isControl(codePoint);
    let noBreak: boolean = false;

    if (previousCodePoint !== null) {
      const previousControl: boolean = isControl(previousCodePoint);
      noBreak = previousCodePoint === 0x000d && codePoint === 0x000a;
      if (!noBreak && !previousControl && !currentControl) {
        noBreak =
          currentExtend ||
          currentZwj ||
          isPrepend(previousCodePoint) ||
          (joinAfterZwj && isExtendedPictographic(codePoint));
        if (!noBreak) {
          const previousHangul: ReturnType<typeof hangulClass> = hangulClass(previousCodePoint);
          const currentHangul: ReturnType<typeof hangulClass> = hangulClass(codePoint);
          noBreak =
            previousHangul === "L" &&
            (currentHangul === "L" || currentHangul === "V" || currentHangul === "LV" || currentHangul === "LVT");
          noBreak ||=
            (previousHangul === "LV" || previousHangul === "V") &&
            (currentHangul === "V" || currentHangul === "T");
          noBreak ||=
            (previousHangul === "LVT" || previousHangul === "T") && currentHangul === "T";
        }
        if (!noBreak && isRegionalIndicator(codePoint)) {
          noBreak = isRegionalIndicator(previousCodePoint) && regionalIndicatorRun % 2 === 1;
        }
      }
    }

    if (!noBreak) {
      count++;
    }

    if (isRegionalIndicator(codePoint)) {
      regionalIndicatorRun++;
    } else {
      regionalIndicatorRun = 0;
    }
    if (currentZwj) {
      joinAfterZwj = extendedPictographicBeforeZwj;
    } else if (currentExtend) {
      // Keep the pictographic context across variation selectors and marks.
    } else {
      joinAfterZwj = false;
      extendedPictographicBeforeZwj = isExtendedPictographic(codePoint);
    }
    previousCodePoint = codePoint;
  }
  return count;
}

/** Count the same extended graphemes as the shared Rust clipboard store. */
function extendedGraphemeCount(text: string): number {
  const Segmenter: IntlWithSegmenter["Segmenter"] = INTL_WITH_SEGMENTER.Segmenter;
  if (Segmenter !== undefined) {
    const segmenter: GraphemeSegmenter = new Segmenter(undefined, { granularity: "grapheme" });
    let count: number = 0;
    for (const _segment of segmenter.segment(text)) {
      count++;
    }
    return count;
  }
  return fallbackGraphemeCount(text);
}

export class ClipboardHistoryPolicy {
  static readonly LIMIT: number = 50;
  static readonly MAX_CHARS: number = MAX_CHARS;
  static readonly MAX_BYTES: number = MAX_BYTES;

  static acceptable(text: string | null): boolean {
    return (
      text !== null &&
      text.trim() !== "" &&
      !text.includes("\u0000") &&
      extendedGraphemeCount(text) <= MAX_CHARS &&
      utf8Length(text) <= MAX_BYTES
    );
  }
}
