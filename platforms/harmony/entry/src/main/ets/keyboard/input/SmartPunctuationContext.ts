import { TextPolicy } from "../TextPolicy";
/** Bounded editor-context adapter shared by Harmony's punctuation entry points. */
export class SmartPunctuationContext {
  /** Return the scalar immediately before the cursor, or zero when unavailable/invalid. */
  static precedingCodePoint(beforeCursor: string | null | undefined): number {
    if (typeof beforeCursor !== "string" || beforeCursor.length === 0) return 0;
    const points: number[] = TextPolicy.characters(beforeCursor).map(
      (value: string): number => value.codePointAt(0) ?? 0,
    );
    const value: number | undefined = points[points.length - 1];
    if (value === undefined || (value >= 0xd800 && value <= 0xdfff)) return 0;
    return value;
  }
}
