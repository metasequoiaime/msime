import { KeyboardGeometry } from '../KeyboardGeometry';

export interface HandwritingPoint {
  x: number;
  y: number;
}

export interface HandwritingStroke {
  points: HandwritingPoint[];
}

/** Bounds shared by the keyboard canvas and the OCR snapshot. */
export const HANDWRITING_CANVAS_SIZE: number = 420;
// Match the fixed Apple source. The shared panel contract accepts these bounds (and a larger point
// budget), so complex characters and the last four recognition alternatives need not be discarded
// merely because this host reaches a platform OCR service instead of ML Kit.
export const HANDWRITING_MAX_STROKES: number = 64;
export const HANDWRITING_MAX_POINTS: number = 512;
export const HANDWRITING_MAX_CANDIDATES: number = 12;

export class HandwritingStrokePolicy {
  static point(x: number, y: number): HandwritingPoint {
    return {
      x: KeyboardGeometry.bounded(Number.isFinite(x) ? x : 0, 0, HANDWRITING_CANVAS_SIZE),
      y: KeyboardGeometry.bounded(Number.isFinite(y) ? y : 0, 0, HANDWRITING_CANVAS_SIZE),
    };
  }

  static canRecognize(strokes: HandwritingStroke[]): boolean {
    return (
      strokes.length > 0 &&
      strokes.length <= HANDWRITING_MAX_STROKES &&
      strokes.every(
        (stroke: HandwritingStroke): boolean =>
          stroke.points.length > 0 && stroke.points.length <= HANDWRITING_MAX_POINTS,
      )
    );
  }

  static candidates(value: string, limit: number = HANDWRITING_MAX_CANDIDATES): string[] {
    if (!Number.isFinite(limit) || limit <= 0) {
      return [];
    }
    const count: number = Math.min(Math.floor(limit), HANDWRITING_MAX_CANDIDATES);
    const output: string[] = [];
    for (const character of Array.from(value.replace(/[\u0000-\u001f\u007f]/g, ""))) {
      if (/\s/u.test(character) || output.includes(character)) {
        continue;
      }
      output.push(character);
      if (output.length >= count) {
        break;
      }
    }
    return output;
  }
}
