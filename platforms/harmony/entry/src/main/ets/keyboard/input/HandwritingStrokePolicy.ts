import { KeyboardGeometry } from "../KeyboardGeometry";

import { TextPolicy } from "../TextPolicy";
export interface HandwritingPoint {
  x: number;
  y: number;
}

export interface HandwritingStroke {
  points: HandwritingPoint[];
}

/** 书写区显示坐标和离线识别器使用的固定坐标。 */
export const HANDWRITING_CANVAS_SIZE: number = 420;
// Match the fixed Apple source. The shared panel contract accepts these bounds and a larger point
// budget, so complex characters and the last four recognition alternatives are not discarded.
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

  /** 书写区可以随键盘宽度变化；显示坐标保留原尺寸，识别前再映射到固定的 420×420 引擎坐标。 */
  static pointInCanvas(x: number, y: number, width: number, height: number): HandwritingPoint {
    const canvasWidth: number =
      Number.isFinite(width) && width > 0 ? width : HANDWRITING_CANVAS_SIZE;
    const canvasHeight: number =
      Number.isFinite(height) && height > 0 ? height : HANDWRITING_CANVAS_SIZE;
    return {
      x: KeyboardGeometry.bounded(Number.isFinite(x) ? x : 0, 0, canvasWidth),
      y: KeyboardGeometry.bounded(Number.isFinite(y) ? y : 0, 0, canvasHeight),
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
    for (const character of TextPolicy.characters(value.replace(/[\u0000-\u001f\u007f]/g, ""))) {
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
