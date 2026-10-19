/** 共享扫描器返回的 canvas 几何。宿主只负责布局，不重新解释 TOML。 */
export interface CandidateCanvasInsets {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

export interface CandidateCanvasBorder {
  readonly source: string;
  readonly lightSource?: string | null;
  readonly sliceDip: number[];
  readonly stretchDip?: number[] | null;
  readonly widthDip?: number[] | null;
  readonly fill: boolean;
}

export interface CandidateCanvasDecoration {
  readonly image: string;
  readonly anchor: string;
  readonly offsetXDip: number;
  readonly offsetYDip: number;
  readonly widthDip: number;
  readonly heightDip: number;
  readonly zIndex: number;
}

export interface CandidateCanvasSpec {
  readonly canvas?: boolean;
  readonly borderImage?: CandidateCanvasBorder | null;
  readonly decorations?: CandidateCanvasDecoration[];
  readonly contentInsetsDip?: CandidateCanvasInsets | null;
  readonly outerInsetsDip?: CandidateCanvasInsets | null;
  readonly lineGapDip?: number | null;
}

export interface CandidateCanvasResource {
  readonly relative: string;
  readonly contentType: string;
  readonly bytes: Uint8Array;
}

export interface CandidateCanvasScene extends CandidateCanvasSpec {
  readonly key: string;
  readonly dark: boolean;
  readonly resources: CandidateCanvasResource[];
}

export interface CandidateCanvasRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface CandidateCanvasPatch {
  readonly source: CandidateCanvasRect;
  readonly destination: CandidateCanvasRect;
}

export interface CandidateCanvasGeometry {
  readonly scale: number;
  readonly card: CandidateCanvasRect;
  readonly content: CandidateCanvasInsets;
}

/** 无 ArkUI 依赖的布局与帧时钟；像素切片与 vp 边宽分别计算。 */
export class CandidateCanvasPolicy {
  static readonly MAX_FRAMES: number = 120;
  static readonly MAX_EDGE: number = 2048;
  static readonly MAX_PIXELS: number = 8000000;
  static readonly ZERO: CandidateCanvasInsets = { top: 0, right: 0, bottom: 0, left: 0 };

  static sides(
    values: number[] | null | undefined,
    fallback: CandidateCanvasInsets,
  ): CandidateCanvasInsets {
    if (
      values === null ||
      values === undefined ||
      ![1, 2, 4].includes(values.length) ||
      values.some((value: number): boolean => !Number.isFinite(value) || value < 0)
    ) {
      return fallback;
    }
    return {
      top: values[0],
      right: values.length === 1 ? values[0] : values[1],
      bottom: values.length === 4 ? values[2] : values[0],
      left: values.length === 4 ? values[3] : values.length === 1 ? values[0] : values[1],
    };
  }

  static cuts(size: number, start: number, end: number, origin: number = 0): number[] {
    const extent: number = Number.isFinite(size) ? Math.max(0, size) : 0;
    const a: number = Number.isFinite(start) ? Math.max(0, start) : 0;
    const b: number = Number.isFinite(end) ? Math.max(0, end) : 0;
    const scale: number = a + b > extent && a + b > 0 ? extent / (a + b) : 1;
    return [origin, origin + a * scale, origin + extent - b * scale, origin + extent];
  }

  static slice(border: CandidateCanvasBorder): CandidateCanvasInsets {
    return CandidateCanvasPolicy.sides(
      border.stretchDip ?? border.sliceDip,
      CandidateCanvasPolicy.ZERO,
    );
  }

  static widths(spec: CandidateCanvasSpec): CandidateCanvasInsets {
    const border: CandidateCanvasBorder | null | undefined = spec.borderImage;
    return border === null || border === undefined
      ? CandidateCanvasPolicy.ZERO
      : CandidateCanvasPolicy.sides(border.widthDip, CandidateCanvasPolicy.slice(border));
  }

  static extraWidth(spec: CandidateCanvasSpec | null): number {
    if (spec === null) return 0;
    const insets: CandidateCanvasInsets = CandidateCanvasPolicy.geometry(
      spec,
      10000,
      10000,
      0,
      0,
    ).content;
    return insets.left + insets.right;
  }

  static extraHeight(spec: CandidateCanvasSpec | null, showPreedit: boolean): number {
    if (spec === null) return 0;
    const insets: CandidateCanvasInsets = CandidateCanvasPolicy.geometry(
      spec,
      10000,
      10000,
      0,
      0,
    ).content;
    return insets.top + insets.bottom + (showPreedit ? (spec.lineGapDip ?? 0) : 0);
  }

  /** 固定手机条高度优先保留 28vp 文字；桌面可以传入更大的实际卡片。 */
  static geometry(
    spec: CandidateCanvasSpec,
    width: number,
    height: number,
    minimumWidth: number = 44,
    minimumHeight: number = 28,
  ): CandidateCanvasGeometry {
    const border: CandidateCanvasInsets = CandidateCanvasPolicy.widths(spec);
    const declared: CandidateCanvasInsets = spec.contentInsetsDip ?? CandidateCanvasPolicy.ZERO;
    const content: CandidateCanvasInsets = {
      top: Math.max(border.top, declared.top),
      right: Math.max(border.right, declared.right),
      bottom: Math.max(border.bottom, declared.bottom),
      left: Math.max(border.left, declared.left),
    };
    const outer: CandidateCanvasInsets = spec.outerInsetsDip ?? CandidateCanvasPolicy.ZERO;
    const horizontal: number = content.left + content.right + outer.left + outer.right;
    const vertical: number = content.top + content.bottom + outer.top + outer.bottom;
    const scale: number = Math.min(
      1,
      horizontal > 0 ? Math.max(0, width - minimumWidth) / horizontal : 1,
      vertical > 0 ? Math.max(0, height - minimumHeight) / vertical : 1,
    );
    const x: number[] = CandidateCanvasPolicy.cuts(width, outer.left * scale, outer.right * scale);
    const y: number[] = CandidateCanvasPolicy.cuts(height, outer.top * scale, outer.bottom * scale);
    return {
      scale: scale,
      card: { x: x[1], y: y[1], width: x[2] - x[1], height: y[2] - y[1] },
      content: {
        top: (content.top + outer.top) * scale,
        right: (content.right + outer.right) * scale,
        bottom: (content.bottom + outer.bottom) * scale,
        left: (content.left + outer.left) * scale,
      },
    };
  }

  static patches(
    border: CandidateCanvasBorder,
    imageWidth: number,
    imageHeight: number,
    card: CandidateCanvasRect,
    scale: number,
  ): CandidateCanvasPatch[] {
    const slice: CandidateCanvasInsets = CandidateCanvasPolicy.slice(border);
    const widths: CandidateCanvasInsets = CandidateCanvasPolicy.sides(border.widthDip, slice);
    const sx: number[] = CandidateCanvasPolicy.cuts(imageWidth, slice.left, slice.right);
    const sy: number[] = CandidateCanvasPolicy.cuts(imageHeight, slice.top, slice.bottom);
    const dx: number[] = CandidateCanvasPolicy.cuts(
      card.width,
      widths.left * scale,
      widths.right * scale,
      card.x,
    );
    const dy: number[] = CandidateCanvasPolicy.cuts(
      card.height,
      widths.top * scale,
      widths.bottom * scale,
      card.y,
    );
    const result: CandidateCanvasPatch[] = [];
    for (let row: number = 0; row < 3; row++) {
      for (let column: number = 0; column < 3; column++) {
        if (row === 1 && column === 1 && !border.fill) continue;
        const source: CandidateCanvasRect = {
          x: sx[column],
          y: sy[row],
          width: sx[column + 1] - sx[column],
          height: sy[row + 1] - sy[row],
        };
        const destination: CandidateCanvasRect = {
          x: dx[column],
          y: dy[row],
          width: dx[column + 1] - dx[column],
          height: dy[row + 1] - dy[row],
        };
        if (
          source.width > 0 &&
          source.height > 0 &&
          destination.width > 0 &&
          destination.height > 0
        ) {
          result.push({ source: source, destination: destination });
        }
      }
    }
    return result;
  }

  static decorations(values: CandidateCanvasDecoration[]): CandidateCanvasDecoration[] {
    // 显式保留清单序号，不能依赖设备 JavaScript 版本的排序稳定性。
    const ordered: number[] = values.map(
      (_value: CandidateCanvasDecoration, index: number): number => index,
    );
    ordered.sort((a: number, b: number): number => values[a].zIndex - values[b].zIndex || a - b);
    return ordered.map((index: number): CandidateCanvasDecoration => values[index]);
  }

  static decorationRect(
    value: CandidateCanvasDecoration,
    imageWidth: number,
    imageHeight: number,
    card: CandidateCanvasRect,
    scale: number,
  ): CandidateCanvasRect {
    const width: number = value.widthDip * scale;
    const height: number = value.heightDip * scale;
    const x: number = value.anchor.includes("left")
      ? card.x
      : value.anchor.includes("right")
        ? card.x + card.width - width
        : card.x + (card.width - width) / 2;
    const y: number = value.anchor.includes("top")
      ? card.y
      : value.anchor.includes("bottom")
        ? card.y + card.height - height
        : card.y + (card.height - height) / 2;
    const fit: number =
      imageWidth > 0 && imageHeight > 0 ? Math.min(width / imageWidth, height / imageHeight) : 0;
    return {
      x: x + value.offsetXDip * scale + (width - imageWidth * fit) / 2,
      y: y + value.offsetYDip * scale + (height - imageHeight * fit) / 2,
      width: imageWidth * fit,
      height: imageHeight * fit,
    };
  }

  static acceptsImage(width: number, height: number, frames: number, remaining: number): boolean {
    return (
      Number.isInteger(width) &&
      Number.isInteger(height) &&
      Number.isInteger(frames) &&
      width > 0 &&
      height > 0 &&
      width <= CandidateCanvasPolicy.MAX_EDGE &&
      height <= CandidateCanvasPolicy.MAX_EDGE &&
      frames > 0 &&
      frames <= CandidateCanvasPolicy.MAX_FRAMES &&
      width * height * frames <= remaining
    );
  }

  /** 浏览器与 ImageIO 相同：小于等于 10ms 的帧按 100ms，避免恶意图片忙循环。 */
  static delay(milliseconds: number): number {
    return Number.isFinite(milliseconds) && milliseconds > 10 ? milliseconds : 100;
  }

  /** GIF 的扩展计数是额外重复次数；APNG 和 WebP 已经是总轮数。 */
  static loops(resource: CandidateCanvasResource): number {
    const bytes: Uint8Array = resource.bytes;
    if (resource.contentType === "image/gif") {
      if (bytes.length < 13) return 1;
      let cursor: number = 13 + ((bytes[10] & 128) !== 0 ? 3 * (1 << ((bytes[10] & 7) + 1)) : 0);
      while (cursor < bytes.length) {
        const tag: number = bytes[cursor++];
        if (tag === 0x3b) break;
        if (tag === 0x21) {
          const kind: number = bytes[cursor++];
          if (kind === 0xff && bytes[cursor] === 11 && cursor + 16 <= bytes.length) {
            let application: string = "";
            for (let i: number = 1; i <= 11; i++)
              application += String.fromCharCode(bytes[cursor + i]);
            if (
              (application === "NETSCAPE2.0" || application === "ANIMEXTS1.0") &&
              bytes[cursor + 12] === 3 &&
              bytes[cursor + 13] === 1
            ) {
              const repeats: number = bytes[cursor + 14] | (bytes[cursor + 15] << 8);
              return repeats === 0 ? 0 : repeats + 1;
            }
          }
        } else if (tag === 0x2c) {
          if (cursor + 9 >= bytes.length) break;
          const packed: number = bytes[cursor + 8];
          cursor += 9 + ((packed & 128) !== 0 ? 3 * (1 << ((packed & 7) + 1)) : 0);
          cursor++;
        } else {
          break;
        }
        while (cursor < bytes.length) {
          const length: number = bytes[cursor++];
          if (length === 0) break;
          cursor += length;
        }
      }
      return 1;
    }
    if (resource.contentType === "image/png") {
      let cursor: number = 8;
      while (cursor + 12 <= bytes.length) {
        const length: number = CandidateCanvasPolicy.u32(bytes, cursor, false);
        if (length > bytes.length - cursor - 12) break;
        if (
          length === 8 &&
          bytes[cursor + 4] === 97 &&
          bytes[cursor + 5] === 99 &&
          bytes[cursor + 6] === 84 &&
          bytes[cursor + 7] === 76
        ) {
          return CandidateCanvasPolicy.u32(bytes, cursor + 12, false);
        }
        cursor += length + 12;
      }
    }
    if (resource.contentType === "image/webp") {
      let cursor: number = 12;
      while (cursor + 8 <= bytes.length) {
        const length: number = CandidateCanvasPolicy.u32(bytes, cursor + 4, true);
        if (length > bytes.length - cursor - 8) break;
        if (
          length >= 6 &&
          bytes[cursor] === 65 &&
          bytes[cursor + 1] === 78 &&
          bytes[cursor + 2] === 73 &&
          bytes[cursor + 3] === 77
        ) {
          return bytes[cursor + 12] | (bytes[cursor + 13] << 8);
        }
        cursor += length + 8 + (length % 2);
      }
    }
    return 0;
  }

  private static u32(bytes: Uint8Array, index: number, little: boolean): number {
    const a: number = little ? bytes[index + 3] : bytes[index];
    const b: number = little ? bytes[index + 2] : bytes[index + 1];
    const c: number = little ? bytes[index + 1] : bytes[index + 2];
    const d: number = little ? bytes[index] : bytes[index + 3];
    return ((a << 24) | (b << 16) | (c << 8) | d) >>> 0;
  }

  static frame(durations: number[], elapsed: number, loops: number): number {
    if (durations.length === 0) return 0;
    const total: number = durations.reduce(
      (sum: number, duration: number): number => sum + CandidateCanvasPolicy.delay(duration),
      0,
    );
    if (loops > 0 && elapsed >= total * loops) return durations.length - 1;
    let cursor: number = Math.max(0, elapsed) % total;
    for (let index: number = 0; index < durations.length; index++) {
      cursor -= CandidateCanvasPolicy.delay(durations[index]);
      if (cursor < 0) return index;
    }
    return durations.length - 1;
  }
}
