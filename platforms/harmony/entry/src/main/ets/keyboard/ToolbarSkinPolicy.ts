import { CandidateColors } from './skin/GlobalTheme';
import { KeyboardGeometry } from './KeyboardGeometry';

/** The subset of a toolbar stylesheet that ArkUI can render without executing CSS. */
export interface ToolbarSkin {
  backgroundColor: string;
  borderColor: string;
  dragHandleColor: string;
  dividerColor: string;
  buttonColor: string;
  buttonHoverColor: string;
  settingsColor: string;
  fontFamily: string;
  englishFontFamily: string;
  cornerRadiusVp: number;
}

const DEFAULT_FONT_FAMILY: string = 'Noto Sans SC, Microsoft YaHei, sans-serif';
const DEFAULT_ENGLISH_FONT_FAMILY: string = 'Segoe UI, sans-serif';
// The Harmony key radius, which the bar shares so it reads as part of the same keyboard.
const DEFAULT_CORNER_RADIUS_VP: number = 8;

function stripImportant(value: string): string {
  return value.trim().replace(/\s*!important\s*$/i, '').trim();
}

function color(value: string): string | null {
  const candidate: string = stripImportant(value);
  if (candidate === 'transparent') return candidate;
  if (/^#[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{2})?$/.test(candidate)) {
    return candidate;
  }
  const rgb: RegExpMatchArray | null = candidate.match(
    /^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?:\s*,\s*(0|1|0?\.\d+))?\s*\)$/);
  if (rgb !== null && Number(rgb[1]) <= 255 && Number(rgb[2]) <= 255
      && Number(rgb[3]) <= 255) {
    return candidate;
  }
  return null;
}

function fontFamily(value: string): string | null {
  const candidate: string = stripImportant(value);
  if (candidate.length === 0 || candidate.length > 256 || /[{};<>]|url\s*\(|var\s*\(/i.test(candidate)) {
    return null;
  }
  const names: string[] = candidate.split(',').map((name: string) => name.trim());
  if (names.length === 0 || names.length > 16 || names.some((name: string): boolean => {
    const unquoted: string = name.replace(/^(['"])(.*)\1$/, '$2').trim();
    return unquoted.length === 0 || unquoted.length > 96
      || !/^[\p{L}\p{N} _-]+$/u.test(unquoted);
  })) {
    return null;
  }
  return names.join(', ');
}

function radius(value: string): number | null {
  const candidate: string = stripImportant(value);
  const match: RegExpMatchArray | null = candidate.match(/^(\d+(?:\.\d+)?)px$/i);
  if (match === null) return null;
  const parsed: number = Number(match[1]);
  return Number.isFinite(parsed) ? KeyboardGeometry.bounded(parsed, 0, 32) : null;
}

function declarationColor(value: string): string | null {
  const direct: string | null = color(value);
  if (direct !== null) return direct;
  // A border/background shorthand is safe only when its colour token is itself a supported colour.
  const match: RegExpMatchArray | null = stripImportant(value).match(
    /(?:^|\s)(#[0-9a-fA-F]{3,8}|rgba?\([^)]*\)|transparent)(?:\s|$)/i);
  return match === null ? null : color(match[1]);
}

function selectors(block: string): string[] {
  return block.split(',').map((selector: string) => selector.trim());
}

function has(selector: string, name: string): boolean {
  return new RegExp('(?:^|\\s|>)\\.' + name + '(?::[a-z-]+)?(?:\\s|$)', 'i').test(selector);
}

/**
 * Translate a bounded external toolbar stylesheet into ArkUI values.
 *
 * CSS is deliberately parsed as data: only a small, known selector/property vocabulary crosses the
 * boundary. Nested rules, URLs, variables, animations and arbitrary declarations are ignored rather
 * than interpolated into ArkUI or executed by a WebView.
 */
export class ToolbarSkinPolicy {
  /**
   * The bar's colours from the candidate palette, the way every host derives its toolbar: the surface and outline from `surface` and `border`, the buttons from `text`, the hovered one from `hover`, and the quieter drag handle from `secondary`.
   */
  static base(colors: CandidateColors): ToolbarSkin {
    return {
      backgroundColor: colors.surface,
      borderColor: colors.border,
      dragHandleColor: colors.secondary,
      dividerColor: colors.border,
      buttonColor: colors.text,
      buttonHoverColor: colors.hover,
      settingsColor: colors.text,
      fontFamily: DEFAULT_FONT_FAMILY,
      englishFontFamily: DEFAULT_ENGLISH_FONT_FAMILY,
      cornerRadiusVp: DEFAULT_CORNER_RADIUS_VP
    };
  }

  static fromCss(colors: CandidateColors, source: string | null | undefined): ToolbarSkin {
    const output: ToolbarSkin = ToolbarSkinPolicy.base(colors);
    if (source === null || source === undefined || source.length === 0 || source.length > 128 * 1024) {
      return output;
    }
    const css: string = source.replace(/\/\*[\s\S]*?\*\//g, '');
    const blocks: RegExp = /([^{}]{1,1024})\{([^{}]{0,8192})\}/g;
    let match: RegExpExecArray | null;
    while ((match = blocks.exec(css)) !== null) {
      const names: string[] = selectors(match[1]);
      const body: string = match[2];
      const declarations: RegExp = /([a-z-]{1,48})\s*:\s*([^;]{1,512})(?:;|$)/gi;
      let declaration: RegExpExecArray | null;
      while ((declaration = declarations.exec(body)) !== null) {
        const property: string = declaration[1].toLowerCase();
        const value: string = declaration[2].trim();
        for (const selector of names) {
          if (has(selector, 'status-bar')) {
            if (property === 'background' || property === 'background-color') {
              const parsed: string | null = declarationColor(value);
              if (parsed !== null) output.backgroundColor = parsed;
            } else if (property === 'border' || property === 'border-color') {
              const parsed: string | null = declarationColor(value);
              if (parsed !== null) output.borderColor = parsed;
            } else if (property === 'border-radius') {
              const parsed: number | null = radius(value);
              if (parsed !== null) output.cornerRadiusVp = parsed;
            }
          }
          if (has(selector, 'drag-handle')
              && (property === 'background' || property === 'background-color')) {
            const parsed: string | null = declarationColor(value);
            if (parsed !== null) output.dragHandleColor = parsed;
          }
          if (has(selector, 'divider')
              && (property === 'background' || property === 'background-color')) {
            const parsed: string | null = declarationColor(value);
            if (parsed !== null) output.dividerColor = parsed;
          }
          if (has(selector, 'icon')) {
            if (property === 'color') {
              const parsed: string | null = color(value);
              if (parsed !== null) output.buttonColor = parsed;
            } else if (property === 'background' || property === 'background-color') {
              const parsed: string | null = declarationColor(value);
              if (parsed !== null && /:hover/i.test(selector)) output.buttonHoverColor = parsed;
            } else if (property === 'font-family') {
              const parsed: string | null = fontFamily(value);
              if (parsed !== null) output.fontFamily = parsed;
            }
          }
          if (has(selector, 'lang-label') && property === 'font-family') {
            const parsed: string | null = fontFamily(value);
            if (parsed !== null) output.fontFamily = parsed;
          }
          if (has(selector, 'english-candidate-label') && property === 'font-family') {
            const parsed: string | null = fontFamily(value);
            if (parsed !== null) output.englishFontFamily = parsed;
          }
        }
      }
    }
    return output;
  }
}
