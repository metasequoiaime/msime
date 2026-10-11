import { TextPolicy } from "../TextPolicy";
/** Extracts a Han scalar from a candidate for the word-to-character fallback. */
export enum CandidateTextEdge {
  FIRST,
  LAST,
}

export class CandidateTextPolicy {
  static isHanCodePoint(value: number): boolean {
    return (
      value === 0x3007 ||
      (value >= 0x3400 && value <= 0x4dbf) ||
      (value >= 0x4e00 && value <= 0x9fff) ||
      (value >= 0xf900 && value <= 0xfaff) ||
      (value >= 0x20000 && value <= 0x2fa1f) ||
      (value >= 0x30000 && value <= 0x323af)
    );
  }

  static extractHanCharacter(text: string, edge: CandidateTextEdge): string | null {
    if (typeof text !== "string" || text.length === 0) return null;
    let result: string | null = null;
    for (const value of TextPolicy.characters(text)) {
      const codePoint: number = value.codePointAt(0) ?? 0;
      if (!this.isHanCodePoint(codePoint)) continue;
      result = value;
      if (edge === CandidateTextEdge.FIRST) return result;
    }
    return result;
  }
}
