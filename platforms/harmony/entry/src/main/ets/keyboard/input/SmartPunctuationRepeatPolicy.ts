import { TextPolicy } from "../TextPolicy";
/** Pure policy for converting a short repeated ASCII punctuation press to Chinese. */
export interface SmartPunctuationRepeatSnapshot {
  readonly ascii: number;
  /** The scalar the first press committed, ASCII or its full-width twin. */
  readonly committed: number;
  readonly timestamp: number;
  readonly editorGeneration: number;
}

export class SmartPunctuationRepeatPolicy {
  static readonly WINDOW_MS: number = 2000;

  static chineseMark(ascii: number): string | null {
    switch (ascii) {
      case 0x2c:
        return "，";
      case 0x2e:
        return "。";
      case 0x3a:
        return "：";
      default:
        return null;
    }
  }

  static isAsciiAlphanumeric(value: number): boolean {
    return (
      (value >= 0x30 && value <= 0x39) ||
      (value >= 0x41 && value <= 0x5a) ||
      (value >= 0x61 && value <= 0x7a)
    );
  }

  static fullWidthMark(ascii: number): number {
    switch (ascii) {
      case 0x2c:
        return 0xff0c;
      case 0x2e:
        return 0xff0e;
      case 0x3a:
        return 0xff1a;
      default:
        return ascii;
    }
  }

  /** Arm only when the native transition actually committed the requested ASCII mark. */
  static snapshot(
    ascii: number,
    commit: string | null | undefined,
    timestamp: number,
    editorGeneration: number,
  ): SmartPunctuationRepeatSnapshot | null {
    if (this.chineseMark(ascii) === null || typeof commit !== "string" || commit.length === 0) {
      return null;
    }
    const points: number[] = TextPolicy.characters(commit).map(
      (value: string): number => value.codePointAt(0) ?? 0,
    );
    const committed: number | undefined = points[points.length - 1];
    if (committed !== ascii && committed !== this.fullWidthMark(ascii)) return null;
    return {
      ascii: ascii,
      committed: committed,
      timestamp: timestamp,
      editorGeneration: editorGeneration,
    };
  }

  static shouldReplace(
    snapshot: SmartPunctuationRepeatSnapshot | null,
    ascii: number,
    preceding: number,
    timestamp: number,
    editorGeneration: number,
    smartEnabled: boolean,
    repeatEnabled: boolean,
    composing: boolean,
    candidateCount: number,
  ): boolean {
    if (
      !smartEnabled ||
      !repeatEnabled ||
      composing ||
      candidateCount !== 0 ||
      snapshot === null ||
      snapshot.ascii !== ascii ||
      snapshot.editorGeneration !== editorGeneration ||
      preceding !== snapshot.committed
    ) {
      return false;
    }
    return (
      timestamp >= snapshot.timestamp &&
      timestamp - snapshot.timestamp <= this.WINDOW_MS &&
      this.chineseMark(ascii) !== null
    );
  }
}
