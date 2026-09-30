import { FloatingToolbarLayout } from "./FloatingToolbarLayout";

// The badge is the floating toolbar's size, read from the same floating_toolbar.font_size and scale_percent, as platforms/macos/src/input/InputModeHUDPanel.mm sizes it: the toolbar's (font + 20) x scale height, 0.95 x font glyphs, a 22vp brand mark 6vp before the character, 12vp side insets and a 10vp corner radius, all scaled.
const GLYPH_SCALE: number = 0.95;
const HEIGHT_PADDING_VP: number = 20;
const LOGO_SIDE_VP: number = 22;
const CONTENT_SPACING_VP: number = 6;
const HORIZONTAL_INSET_VP: number = 12;
const CORNER_RADIUS_VP: number = 10;

/** The badge window's size in vp, which the host resizes the panel to and places by. */
export interface InputModeHudSize {
  widthVp: number;
  heightVp: number;
}

/**
 * Shared preference gate for the mode badge shown after a Chinese/English switch.
 *
 * The shared schema serialises `input_mode_hud` as a plain boolean that defaults to on, so absence
 * means a document written before the field existed rather than a user who turned the badge off.
 */
export class InputModeHudPolicy {
  /** Only an explicit no hides it; anything else keeps the shared default-on behaviour. */
  static enabled(value: unknown): boolean {
    return value !== false;
  }

  /** The character's point size: the toolbar's glyph size at the toolbar's scale. Both are bounded the way the toolbar bounds them, so a stale preference cannot size one differently from the other. */
  static glyphFontSize(fontSize: number, scale: number): number {
    return (
      FloatingToolbarLayout.fontSize(fontSize) * FloatingToolbarLayout.scale(scale) * GLYPH_SCALE
    );
  }

  static logoSideVp(scale: number): number {
    return LOGO_SIDE_VP * FloatingToolbarLayout.scale(scale);
  }

  static spacingVp(scale: number): number {
    return CONTENT_SPACING_VP * FloatingToolbarLayout.scale(scale);
  }

  static insetVp(scale: number): number {
    return HORIZONTAL_INSET_VP * FloatingToolbarLayout.scale(scale);
  }

  static cornerRadiusVp(scale: number): number {
    return CORNER_RADIUS_VP * FloatingToolbarLayout.scale(scale);
  }

  /**
   * The window fitted to its content: both insets, the mark and its gap, and one character.
   *
   * 中 and 英 are both full-width, so the character is taken as one em and switching modes never changes the width. macOS measures the glyph instead; one em is the same bound CandidateWidthPolicy uses for a wide code point, and at the default 24 / 100% it gives 75 x 44 where macOS measures 74 x 44.
   */
  static sizeVp(fontSize: number, scale: number): InputModeHudSize {
    const factor: number = FloatingToolbarLayout.scale(scale);
    const glyph: number = InputModeHudPolicy.glyphFontSize(fontSize, scale);
    return {
      widthVp: Math.ceil(
        2 * HORIZONTAL_INSET_VP * factor + (LOGO_SIDE_VP + CONTENT_SPACING_VP) * factor + glyph,
      ),
      heightVp: Math.ceil((FloatingToolbarLayout.fontSize(fontSize) + HEIGHT_PADDING_VP) * factor),
    };
  }
}
