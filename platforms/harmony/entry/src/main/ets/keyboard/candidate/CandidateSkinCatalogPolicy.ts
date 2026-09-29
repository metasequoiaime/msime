import { KeyboardGeometry } from '../KeyboardGeometry';

/** The fields of a `msime_client_skin_catalog` entry this host still reads. The colours are read through the resolved theme instead. */
export interface CandidateSkinPackage {
  readonly id: string;
  readonly base: string;
  readonly layouts: string[];
  readonly themes: string[];
  readonly minWidthDip: number;
  readonly decorationTopDip: number;
  readonly decorationWidthDip: number;
  readonly toolbarStylesheet?: string | null;
  readonly preview: string | null;
}

export interface CandidateSkinDecoration {
  readonly relative: string;
  readonly topVp: number;
  readonly widthVp: number;
}

/**
 * Resolves the safe, non-colour part of a shared skin catalog for the ArkUI presenter: the decoration, the minimum width and the toolbar stylesheet.
 *
 * A package's colours, and whether it is drawn at all in this layout and mode, are no longer decided here: they arrive in the resolved global theme, whose `candidate_skin` is the single signal for drawing the decoration and minimum width. The Rust catalog scanner already bounds package ids and dimensions; this policy still treats every value as optional because a package may leave any of them out.
 */
export class CandidateSkinCatalogPolicy {
  private static readonly BASE64: string =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

  static package(packages: CandidateSkinPackage[], id: string): CandidateSkinPackage | null {
    for (const candidate of packages) {
      if (candidate.id === id) {
        return candidate;
      }
    }
    return null;
  }

  static minWidthVp(packages: CandidateSkinPackage[], id: string): number | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (candidate === null || !Number.isFinite(candidate.minWidthDip)
        || candidate.minWidthDip <= 0) {
      return null;
    }
    return candidate.minWidthDip;
  }

  static toolbarStylesheet(packages: CandidateSkinPackage[], id: string): string | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (candidate === null || candidate.toolbarStylesheet === undefined
        || candidate.toolbarStylesheet === null || candidate.toolbarStylesheet.length === 0) {
      return null;
    }
    return candidate.toolbarStylesheet;
  }

  /** Resolve only a bounded decoration declaration; the image bytes are loaded separately. */
  static decoration(packages: CandidateSkinPackage[], id: string): CandidateSkinDecoration | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (candidate === null || candidate.preview === null || candidate.preview === undefined
        || candidate.preview.length === 0
        || !Number.isFinite(candidate.decorationTopDip)
        || !Number.isFinite(candidate.decorationWidthDip)
        || candidate.decorationTopDip <= 0 || candidate.decorationWidthDip <= 0
        || candidate.decorationTopDip > 512 || candidate.decorationWidthDip > 1024) {
      return null;
    }
    return {
      relative: candidate.preview,
      topVp: candidate.decorationTopDip,
      widthVp: candidate.decorationWidthDip
    };
  }

  /** Convert validated image bytes into an ArkUI image-only data URL. */
  static imageDataUrl(contentType: string, bytes: number[]): string | null {
    const imageType: boolean = contentType === 'image/png' || contentType === 'image/jpeg'
      || contentType === 'image/gif' || contentType === 'image/webp'
      || contentType === 'image/svg+xml' || contentType === 'image/x-icon'
      || contentType === 'image/bmp' || contentType === 'image/avif';
    if (!imageType || !Array.isArray(bytes) || bytes.length === 0 || bytes.length > 8 * 1024 * 1024) {
      return null;
    }
    for (const byte of bytes) {
      if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
        return null;
      }
    }
    let encoded: string = '';
    for (let index: number = 0; index < bytes.length; index += 3) {
      const first: number = bytes[index];
      const second: number = index + 1 < bytes.length ? bytes[index + 1] : 0;
      const third: number = index + 2 < bytes.length ? bytes[index + 2] : 0;
      encoded += CandidateSkinCatalogPolicy.BASE64[(first >> 2) & 0x3f];
      encoded += CandidateSkinCatalogPolicy.BASE64[((first & 0x03) << 4) | (second >> 4)];
      encoded += index + 1 < bytes.length
        ? CandidateSkinCatalogPolicy.BASE64[((second & 0x0f) << 2) | (third >> 6)] : '=';
      encoded += index + 2 < bytes.length
        ? CandidateSkinCatalogPolicy.BASE64[third & 0x3f] : '=';
    }
    return `data:${contentType};base64,${encoded}`;
  }

  /** Return the intrinsic image ratio for a native aspect-ratio modifier. */
  static imageAspectRatio(contentType: string, bytes: number[]): number {
    let width: number = 0;
    let height: number = 0;
    if (contentType === 'image/png' && bytes.length >= 24
        && bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) {
      width = CandidateSkinCatalogPolicy.u32Be(bytes, 16);
      height = CandidateSkinCatalogPolicy.u32Be(bytes, 20);
    } else if (contentType === 'image/gif' && bytes.length >= 10) {
      width = CandidateSkinCatalogPolicy.u16Le(bytes, 6);
      height = CandidateSkinCatalogPolicy.u16Le(bytes, 8);
    } else if (contentType === 'image/bmp' && bytes.length >= 26) {
      width = CandidateSkinCatalogPolicy.u32Le(bytes, 18);
      height = CandidateSkinCatalogPolicy.u32Le(bytes, 22);
    } else if (contentType === 'image/x-icon' && bytes.length >= 8) {
      width = bytes[6] === 0 ? 256 : bytes[6];
      height = bytes[7] === 0 ? 256 : bytes[7];
    } else if (contentType === 'image/svg+xml') {
      let source: string = '';
      for (let index: number = 0; index < Math.min(4096, bytes.length); index++) {
        source += String.fromCharCode(bytes[index]);
      }
      const viewBox: RegExpMatchArray | null = source.match(
        /viewBox\s*=\s*["']\s*[-+]?\d+(?:\.\d+)?\s+[-+]?\d+(?:\.\d+)?\s+([\d.]+)\s+([\d.]+)/i);
      if (viewBox !== null) {
        width = Number(viewBox[1]);
        height = Number(viewBox[2]);
      } else {
        const svgWidth: RegExpMatchArray | null = source.match(
          /\bwidth\s*=\s*["']\s*([\d.]+)/i);
        const svgHeight: RegExpMatchArray | null = source.match(
          /\bheight\s*=\s*["']\s*([\d.]+)/i);
        width = svgWidth === null ? 0 : Number(svgWidth[1]);
        height = svgHeight === null ? 0 : Number(svgHeight[1]);
      }
    }
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0
        || width > 32768 || height > 32768) {
      return 1;
    }
    return KeyboardGeometry.bounded(width / height, 0.05, 20);
  }

  private static u16Le(bytes: number[], offset: number): number {
    return bytes[offset] | (bytes[offset + 1] << 8);
  }

  private static u32Le(bytes: number[], offset: number): number {
    return (bytes[offset] | (bytes[offset + 1] << 8)
      | (bytes[offset + 2] << 16) | (bytes[offset + 3] << 24)) >>> 0;
  }

  private static u32Be(bytes: number[], offset: number): number {
    return (((bytes[offset] << 24) >>> 0) | (bytes[offset + 1] << 16)
      | (bytes[offset + 2] << 8) | bytes[offset + 3]) >>> 0;
  }
}
