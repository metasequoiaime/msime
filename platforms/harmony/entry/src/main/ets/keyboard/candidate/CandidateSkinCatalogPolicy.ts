import { KeyboardGeometry } from '../KeyboardGeometry';

/** The palette tokens an external candidate skin may contribute to the native Harmony card. */
export interface CandidateSkinPaletteTokens {
  readonly accent?: string | null;
  readonly selected?: string | null;
  readonly hover?: string | null;
  readonly surface?: string | null;
  readonly border?: string | null;
  readonly text?: string | null;
  readonly number?: string | null;
  readonly showSelectedBar?: boolean | null;
}

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
  readonly candidate: {
    readonly dark: CandidateSkinPaletteTokens;
    readonly light: CandidateSkinPaletteTokens;
  };
}

export interface CandidateSkinDecoration {
  readonly relative: string;
  readonly topVp: number;
  readonly widthVp: number;
}

/**
 * Resolves the safe, non-CSS part of a shared skin catalog for the ArkUI presenter.
 *
 * The Rust catalog scanner already bounds package ids, dimensions and token lengths. This policy
 * still treats every value as optional because a package may intentionally inherit part of its
 * base skin, and because Harmony must keep working with a catalog produced by an older host.
 */
export class CandidateSkinCatalogPolicy {
  private static readonly BASE64: string =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

  /**
   * Keep manifest values inside the small CSS colour subset accepted by the
   * native presenters. ArkUI receives these strings directly, so an arbitrary
   * token (for example a declaration or url()) must never cross this boundary.
   */
  static color(value: string | null | undefined): string | null {
    if (value === null || value === undefined) {
      return null;
    }
    const color: string = value.trim();
    if (color === 'transparent') {
      return color;
    }
    if (/^#[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{2})?$/.test(color)) {
      return color;
    }
    const rgb: RegExpMatchArray | null = color.match(
      /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*(0|1|0?\.\d+))?\s*\)$/);
    if (rgb !== null && Number(rgb[1]) <= 255 && Number(rgb[2]) <= 255
        && Number(rgb[3]) <= 255) {
      return color;
    }
    return null;
  }

  private static sanitizePalette(palette: CandidateSkinPaletteTokens): CandidateSkinPaletteTokens {
    return {
      accent: CandidateSkinCatalogPolicy.color(palette.accent),
      selected: CandidateSkinCatalogPolicy.color(palette.selected),
      hover: CandidateSkinCatalogPolicy.color(palette.hover),
      surface: CandidateSkinCatalogPolicy.color(palette.surface),
      border: CandidateSkinCatalogPolicy.color(palette.border),
      text: CandidateSkinCatalogPolicy.color(palette.text),
      number: CandidateSkinCatalogPolicy.color(palette.number),
      showSelectedBar: palette.showSelectedBar ?? null
    };
  }

  static package(packages: CandidateSkinPackage[], id: string): CandidateSkinPackage | null {
    for (const candidate of packages) {
      if (candidate.id === id) {
        return candidate;
      }
    }
    return null;
  }

  static palette(packages: CandidateSkinPackage[], id: string,
                 dark: boolean): CandidateSkinPaletteTokens | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (candidate === null) {
      return null;
    }
    return CandidateSkinCatalogPolicy.sanitizePalette(
      dark ? candidate.candidate.dark : candidate.candidate.light);
  }

  static supports(packages: CandidateSkinPackage[], id: string, layout: string,
                  theme: string): boolean {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    return candidate !== null && candidate.layouts.includes(layout) && candidate.themes.includes(theme);
  }

  static base(packages: CandidateSkinPackage[], id: string): string | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    return candidate === null || candidate.base.length === 0 ? null : candidate.base;
  }

  static minWidthVp(packages: CandidateSkinPackage[], id: string): number | null {
    const candidate: CandidateSkinPackage | null = CandidateSkinCatalogPolicy.package(packages, id);
    if (candidate === null || !Number.isFinite(candidate.minWidthDip)
        || candidate.minWidthDip <= 0) {
      return null;
    }
    return candidate.minWidthDip;
  }

  static showSelectedBar(packages: CandidateSkinPackage[], id: string,
                         dark: boolean): boolean | null {
    const palette: CandidateSkinPaletteTokens | null = CandidateSkinCatalogPolicy.palette(
      packages, id, dark);
    return palette === null ? null : palette.showSelectedBar ?? null;
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
