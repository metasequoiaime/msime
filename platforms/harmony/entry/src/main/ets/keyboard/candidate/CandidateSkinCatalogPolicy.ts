import { DecorationCandidateStyle, DecorationCandidatePolicy } from "./DecorationCandidatePolicy";
import { CandidateCanvasSpec } from "./CandidateCanvasPolicy";
import { ToolbarImageSpec } from "../ToolbarImagePolicy";

import { KeyboardGeometry } from "../KeyboardGeometry";

/** One mode of a package's `[toolbar]` colours, normalized by the shared scan. */
export interface CandidateSkinToolbarColors {
  readonly background?: string | null;
  readonly border?: string | null;
  readonly handle?: string | null;
  readonly divider?: string | null;
  readonly icon?: string | null;
  readonly hover?: string | null;
}

export interface CandidateSkinToolbar {
  readonly cornerRadiusDip: number | null;
  readonly dark: CandidateSkinToolbarColors;
  readonly light: CandidateSkinToolbarColors;
}

export interface CandidateSkinBackgroundEntry {
  readonly image: string;
  readonly fit: string;
  readonly opacity: number;
}

/** The fields of a `msime_client_skin_catalog` entry this host still reads. The colours are read through the resolved theme instead. */
export interface CandidateSkinPackage extends CandidateCanvasSpec {
  readonly id: string;
  readonly schemaVersion?: number;
  /** `base = "canvas"` 的 v2 标记；host catalog 会把 base 规范成 `system`，所以不能只看 base。只影响九宫格、自由挂件等 canvas 专属几何，不影响 `[animation]`：v2 动画对任何 base 都生效。 */
  readonly canvas?: boolean;
  readonly base: string;
  readonly layouts: string[];
  readonly themes: string[];
  readonly minWidthDip: number;
  readonly cornerRadiusDip?: number | null;
  readonly decorationTopDip: number;
  readonly decorationWidthDip: number;
  /** Set only for a decorated package: the manifest's decoration image, or its preview. */
  readonly decorationImage?: string | null;
  readonly decorationAlign?: string;
  readonly decorationCandidate?: DecorationCandidateStyle | null;
  readonly background?: CandidateSkinBackgroundEntry | null;
  readonly toolbar?: CandidateSkinToolbar;
  readonly toolbarImage?: ToolbarImageSpec;
  readonly toolbarStylesheet?: string | null;
  readonly preview: string | null;
  readonly animation?: CandidateSkinAnimation | null;
}

export type CandidateSkinAnimationPreset =
  | "none"
  | "fade"
  | "fade_scale"
  | "slide_up"
  | "spring"
  | "glow";

export interface CandidateSkinAnimation {
  readonly enter: CandidateSkinAnimationPreset;
  readonly exit: CandidateSkinAnimationPreset;
  readonly selection: CandidateSkinAnimationPreset;
  readonly durationMs: number;
  readonly staggerMs: number;
}

export type CandidateSkinAlign = "left" | "center" | "right";
export type CandidateSkinFit = "cover" | "contain" | "stretch";

export interface CandidateSkinDecoration {
  readonly relative: string;
  readonly topVp: number;
  readonly widthVp: number;
  readonly align: CandidateSkinAlign;
}

export interface CandidateSkinBackground {
  readonly relative: string;
  readonly fit: CandidateSkinFit;
  readonly opacity: number;
}

/**
 * Resolves the safe, non-colour part of a shared skin catalog for the ArkUI presenter: the decoration, the minimum width and the toolbar stylesheet.
 *
 * A package's colours, and whether it is drawn at all in this layout and mode, are no longer decided here: they arrive in the resolved global theme, whose `candidate_skin` is the single signal for drawing the decoration and minimum width. The Rust catalog scanner already bounds package ids and dimensions; this policy still treats every value as optional because a package may leave any of them out.
 */
export class CandidateSkinCatalogPolicy {
  /** How much of a resource's `bytesBase64` [`imageAspectRatio`] needs decoded: 5464 characters are the first 4098 bytes, past the SVG search window and every binary header it reads. */
  static readonly ASPECT_RATIO_HEADER_BASE64: number = 5464;
  /** The base64 length of the shared layer's 8 MiB resource limit. */
  private static readonly MAX_IMAGE_BASE64: number = 11184812;

  static package(packages: CandidateSkinPackage[], id: string): CandidateSkinPackage | null {
    for (const candidate of packages) {
      if (candidate.id === id) {
        return candidate;
      }
    }
    return null;
  }

  static animation(packages: CandidateSkinPackage[], id: string): CandidateSkinAnimation {
    const defaults: CandidateSkinAnimation = {
      enter: "fade_scale",
      exit: "fade",
      selection: "spring",
      durationMs: 180,
      staggerMs: 14,
    };
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (
      candidate === null ||
      candidate.schemaVersion !== 2 ||
      candidate.animation === null ||
      candidate.animation === undefined
    )
      return defaults;
    const value: CandidateSkinAnimation = candidate.animation;
    const preset = (
      input: CandidateSkinAnimationPreset | undefined,
      fallback: CandidateSkinAnimationPreset,
    ): CandidateSkinAnimationPreset =>
      input === "none" ||
      input === "fade" ||
      input === "fade_scale" ||
      input === "slide_up" ||
      input === "spring" ||
      input === "glow"
        ? input
        : fallback;
    const duration: number =
      Number.isInteger(value.durationMs) && value.durationMs >= 40 && value.durationMs <= 600
        ? value.durationMs
        : defaults.durationMs;
    const stagger: number =
      Number.isInteger(value.staggerMs) && value.staggerMs >= 0 && value.staggerMs <= 80
        ? value.staggerMs
        : defaults.staggerMs;
    return {
      enter: preset(value.enter, defaults.enter),
      exit: preset(value.exit, defaults.exit),
      selection: preset(value.selection, defaults.selection),
      durationMs: duration,
      staggerMs: stagger,
    };
  }

  static minWidthVp(packages: CandidateSkinPackage[], id: string): number | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (
      candidate === null ||
      !Number.isFinite(candidate.minWidthDip) ||
      candidate.minWidthDip <= 0
    ) {
      return null;
    }
    return candidate.minWidthDip;
  }

  /** The card radius the package asks for, 0-32 vp; null keeps the host's. */
  static cornerRadiusVp(packages: CandidateSkinPackage[], id: string): number | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    const radius: number | null | undefined = candidate === null ? null : candidate.cornerRadiusDip;
    if (
      radius === null ||
      radius === undefined ||
      !Number.isFinite(radius) ||
      radius < 0 ||
      radius > 32
    ) {
      return null;
    }
    return radius;
  }

  /** The image drawn over the card surface, bounded like the rest of the package; its bytes are loaded separately. */
  static background(packages: CandidateSkinPackage[], id: string): CandidateSkinBackground | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    const entry: CandidateSkinBackgroundEntry | null | undefined =
      candidate === null ? null : candidate.background;
    if (
      entry === null ||
      entry === undefined ||
      typeof entry.image !== "string" ||
      entry.image.length === 0 ||
      !Number.isFinite(entry.opacity) ||
      entry.opacity < 0 ||
      entry.opacity > 1
    ) {
      return null;
    }
    const fit: CandidateSkinFit =
      entry.fit === "contain" ? "contain" : entry.fit === "stretch" ? "stretch" : "cover";
    return { relative: entry.image, fit: fit, opacity: entry.opacity };
  }

  /** The package's toolbar colours and radius, or null when it declares none. */
  static toolbar(packages: CandidateSkinPackage[], id: string | null): CandidateSkinToolbar | null {
    if (id === null) {
      return null;
    }
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    return candidate === null || candidate.toolbar === undefined ? null : candidate.toolbar;
  }

  static toolbarStylesheet(packages: CandidateSkinPackage[], id: string): string | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (
      candidate === null ||
      candidate.toolbarStylesheet === undefined ||
      candidate.toolbarStylesheet === null ||
      candidate.toolbarStylesheet.length === 0
    ) {
      return null;
    }
    return candidate.toolbarStylesheet;
  }

  /** Resolve only a bounded decoration declaration; the image bytes are loaded separately. */
  static decoration(packages: CandidateSkinPackage[], id: string): CandidateSkinDecoration | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    const image: string | null | undefined = candidate === null ? null : candidate.decorationImage;
    if (
      candidate === null ||
      image === null ||
      image === undefined ||
      image.length === 0 ||
      !Number.isFinite(candidate.decorationTopDip) ||
      !Number.isFinite(candidate.decorationWidthDip) ||
      candidate.decorationTopDip <= 0 ||
      candidate.decorationWidthDip <= 0 ||
      candidate.decorationTopDip > 512 ||
      candidate.decorationWidthDip > 1024
    ) {
      return null;
    }
    const align: CandidateSkinAlign =
      candidate.decorationAlign === "left"
        ? "left"
        : candidate.decorationAlign === "center"
          ? "center"
          : "right";
    return {
      relative: image,
      topVp: candidate.decorationTopDip,
      widthVp: candidate.decorationWidthDip,
      align: align,
    };
  }

  /** 只接受共享扫描器的有界装饰词声明；没有装饰图时不保留孤立文字。 */
  static decorationCandidate(
    packages: CandidateSkinPackage[],
    id: string,
  ): DecorationCandidateStyle | null {
    const value: DecorationCandidateStyle | null | undefined = CandidateSkinCatalogPolicy.package(
      packages,
      id,
    )?.decorationCandidate;
    return CandidateSkinCatalogPolicy.decoration(packages, id) !== null &&
      DecorationCandidatePolicy.valid(value)
      ? (value ?? null)
      : null;
  }

  /** Wrap the `bytesBase64` of a `msime_client_skin_resource` answer as an ArkUI image-only data URL. */
  static imageDataUrl(contentType: string, encoded: string): string | null {
    const imageType: boolean =
      contentType === "image/png" ||
      contentType === "image/jpeg" ||
      contentType === "image/gif" ||
      contentType === "image/webp" ||
      contentType === "image/svg+xml" ||
      contentType === "image/x-icon" ||
      contentType === "image/bmp" ||
      contentType === "image/avif";
    if (
      !imageType ||
      typeof encoded !== "string" ||
      encoded.length === 0 ||
      encoded.length > CandidateSkinCatalogPolicy.MAX_IMAGE_BASE64 ||
      CandidateSkinCatalogPolicy.base64ByteLength(encoded) < 0
    ) {
      return null;
    }
    return `data:${contentType};base64,${encoded}`;
  }

  /** The byte length a standard padded base64 string decodes to, or -1 when it is not one. */
  static base64ByteLength(encoded: string): number {
    if (encoded.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(encoded)) {
      return -1;
    }
    const padding: number = encoded.endsWith("==") ? 2 : encoded.endsWith("=") ? 1 : 0;
    return (encoded.length / 4) * 3 - padding;
  }

  /** Return the intrinsic image ratio for a native aspect-ratio modifier. */
  static imageAspectRatio(contentType: string, bytes: number[]): number {
    let width: number = 0;
    let height: number = 0;
    if (
      contentType === "image/png" &&
      bytes.length >= 24 &&
      bytes[0] === 0x89 &&
      bytes[1] === 0x50 &&
      bytes[2] === 0x4e &&
      bytes[3] === 0x47
    ) {
      width = CandidateSkinCatalogPolicy.u32Be(bytes, 16);
      height = CandidateSkinCatalogPolicy.u32Be(bytes, 20);
    } else if (contentType === "image/gif" && bytes.length >= 10) {
      width = CandidateSkinCatalogPolicy.u16Le(bytes, 6);
      height = CandidateSkinCatalogPolicy.u16Le(bytes, 8);
    } else if (contentType === "image/bmp" && bytes.length >= 26) {
      width = CandidateSkinCatalogPolicy.u32Le(bytes, 18);
      height = CandidateSkinCatalogPolicy.u32Le(bytes, 22);
    } else if (contentType === "image/x-icon" && bytes.length >= 8) {
      width = bytes[6] === 0 ? 256 : bytes[6];
      height = bytes[7] === 0 ? 256 : bytes[7];
    } else if (contentType === "image/svg+xml") {
      let source: string = "";
      for (let index: number = 0; index < Math.min(4096, bytes.length); index++) {
        source += String.fromCharCode(bytes[index]);
      }
      const viewBox: RegExpMatchArray | null = source.match(
        /viewBox\s*=\s*["']\s*[-+]?\d+(?:\.\d+)?\s+[-+]?\d+(?:\.\d+)?\s+([\d.]+)\s+([\d.]+)/i,
      );
      if (viewBox !== null) {
        width = Number(viewBox[1]);
        height = Number(viewBox[2]);
      } else {
        const svgWidth: RegExpMatchArray | null = source.match(/\bwidth\s*=\s*["']\s*([\d.]+)/i);
        const svgHeight: RegExpMatchArray | null = source.match(/\bheight\s*=\s*["']\s*([\d.]+)/i);
        width = svgWidth === null ? 0 : Number(svgWidth[1]);
        height = svgHeight === null ? 0 : Number(svgHeight[1]);
      }
    }
    if (
      !Number.isFinite(width) ||
      !Number.isFinite(height) ||
      width <= 0 ||
      height <= 0 ||
      width > 32768 ||
      height > 32768
    ) {
      return 1;
    }
    return KeyboardGeometry.bounded(width / height, 0.05, 20);
  }

  private static u16Le(bytes: number[], offset: number): number {
    return bytes[offset] | (bytes[offset + 1] << 8);
  }

  private static u32Le(bytes: number[], offset: number): number {
    return (
      (bytes[offset] |
        (bytes[offset + 1] << 8) |
        (bytes[offset + 2] << 16) |
        (bytes[offset + 3] << 24)) >>>
      0
    );
  }

  private static u32Be(bytes: number[], offset: number): number {
    return (
      (((bytes[offset] << 24) >>> 0) |
        (bytes[offset + 1] << 16) |
        (bytes[offset + 2] << 8) |
        bytes[offset + 3]) >>>
      0
    );
  }
}
