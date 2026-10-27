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

  /**
   * 书写区的一个点在识别器 420×420 坐标里的位置：横竖按同一比例缩放（取书写区较长的一边对齐 420），字形不被拉长或压扁。书写区铺满键盘宽度后宽高比约 1.7，分别缩放会把字压扁近一半；引擎再把整个字的外框缩放到自己的画布，同样保持比例。
   */
  static recognitionPoint(x: number, y: number, width: number, height: number): HandwritingPoint {
    const canvasWidth: number =
      Number.isFinite(width) && width > 0 ? width : HANDWRITING_CANVAS_SIZE;
    const canvasHeight: number =
      Number.isFinite(height) && height > 0 ? height : HANDWRITING_CANVAS_SIZE;
    const scale: number = HANDWRITING_CANVAS_SIZE / Math.max(canvasWidth, canvasHeight);
    return HandwritingStrokePolicy.point(
      (Number.isFinite(x) ? x : 0) * scale, (Number.isFinite(y) ? y : 0) * scale);
  }

  /**
   * 墨色是否完全不透明。书写中只补画新的一小段，圆头线段首尾相接处的笔迹会画两遍：不透明的墨色画两遍和一条连续路径看起来一样，半透明的会在每个接点叠出深色圆点，所以只有能确认不透明（`#RGB`、`#RRGGBB`，或透明度为 FF 的 `#ARGB`/`#AARRGGBB`）时才走补画，认不出的格式按半透明处理、整幅重画。
   */
  static opaqueInk(color: string): boolean {
    if (!/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(color)) {
      return false;
    }
    const digits: number = color.length - 1;
    if (digits === 4) {
      return color[1] === "f" || color[1] === "F";
    }
    if (digits === 8) {
      return color.substring(1, 3).toUpperCase() === "FF";
    }
    return true;
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
