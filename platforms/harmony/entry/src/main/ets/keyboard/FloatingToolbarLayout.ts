import { KeyboardGeometry } from "./KeyboardGeometry";

/**
 * Which buttons the floating toolbar carries, how wide that makes it, and what each one says.
 *
 * Ported from platforms/macos/src/FloatingToolbarPanel.mm, including the arithmetic: the bar is as
 * wide as the buttons the user left on, so hiding one narrows it rather than leaving a gap.
 *
 * Pure, so the width the host resizes the panel to and the faces the page draws are decided in one
 * place and cannot drift apart.
 */

const BUTTON_WIDTH_VP: number = 42;
const BUTTON_GAP_VP: number = 10.5;
const EDGE_INSET_VP: number = 10;
const TOOLBAR_HEIGHT_VP: number = 44;
/** Every component the shared record names, plus the gear that is never hidden. */
const MAXIMUM_BUTTONS: number = 9;
const TOOLBAR_SCALE_VALUES: number[] = [0.75, 1, 1.25, 1.5];
const TOOLBAR_FONT_SIZE_MIN: number = 16;
const TOOLBAR_FONT_SIZE_MAX: number = 28;
const TOOLBAR_FONT_SIZE_DEFAULT: number = 24;

export enum ToolbarButton {
  INPUT_MODE,
  PUNCTUATION,
  FULL_WIDTH,
  CHARACTER_SET,
  EMOJI,
  SCREEN_KEYBOARD,
  SETTINGS,
  // Appended rather than placed where they are drawn: PanelSurfaceAction mirrors SCREEN_KEYBOARD by number, and the order on the bar is decided by buttons() below.
  HANDWRITING,
  VOICE,
}

/** What the keyboard is doing, which is what the faces below report. */
export interface ToolbarState {
  readonly english: boolean;
  /** English candidate input — the temporary Shift+letter mode or the Ctrl+Shift+E mode — keeps the Chinese IME active and is shown as En, as on Windows and macOS. */
  readonly temporaryEnglish: boolean;
  /** Japanese is a distinct input scheme; English still takes precedence when dedicated mode is on. */
  readonly japanese: boolean;
  /** Korean, like Japanese, is a scheme of its own with its own face. */
  readonly korean: boolean;
  /** Vietnamese is a language of its own too; Cantonese and Zhuyin are Chinese and wear 中 as Shuangpin and Wubi do. */
  readonly vietnamese?: boolean;
  /** 藏文同样是独立的语言，显示「藏」。 */
  readonly tibetan?: boolean;
  /** Hardware Caps Lock takes precedence over the language face on desktop keyboards. */
  readonly capsLock: boolean;
  readonly chinesePunctuation: boolean;
  readonly fullWidth: boolean;
  readonly traditional: boolean;
}

/** Which buttons the user left on. The gear is not among them; it is never hidden. */
export interface ToolbarComponents {
  readonly englishMode: boolean;
  readonly punctuation: boolean;
  readonly fullwidth: boolean;
  readonly characterSet: boolean;
  readonly emoji: boolean;
  readonly handwriting: boolean;
  readonly screenKeyboard: boolean;
  readonly voice: boolean;
  readonly settings: boolean;
}

export class FloatingToolbarLayout {
  static idleState(): ToolbarState {
    return {
      english: false,
      temporaryEnglish: false,
      japanese: false,
      korean: false,
      vietnamese: false,
      tibetan: false,
      capsLock: false,
      chinesePunctuation: true,
      fullWidth: false,
      traditional: false,
    };
  }

  static allComponents(): ToolbarComponents {
    return {
      englishMode: true,
      punctuation: true,
      fullwidth: true,
      characterSet: true,
      emoji: true,
      handwriting: true,
      screenKeyboard: true,
      voice: true,
      settings: true,
    };
  }

  /** In the order the Apple toolbar puts them, with the gear last when it is shown at all. */
  static buttons(components: ToolbarComponents): ToolbarButton[] {
    const chosen: ToolbarButton[] = [];
    if (components.englishMode) {
      chosen.push(ToolbarButton.INPUT_MODE);
    }
    if (components.punctuation) {
      chosen.push(ToolbarButton.PUNCTUATION);
    }
    if (components.fullwidth) {
      chosen.push(ToolbarButton.FULL_WIDTH);
    }
    if (components.characterSet) {
      chosen.push(ToolbarButton.CHARACTER_SET);
    }
    if (components.emoji) {
      chosen.push(ToolbarButton.EMOJI);
    }
    if (components.handwriting) {
      chosen.push(ToolbarButton.HANDWRITING);
    }
    if (components.screenKeyboard) {
      chosen.push(ToolbarButton.SCREEN_KEYBOARD);
    }
    if (components.voice) {
      chosen.push(ToolbarButton.VOICE);
    }
    // Optional like the rest of them. It used to be the one button nobody could turn off, which
    // made the shared 设置 switch a control with no outcome on this host.
    if (components.settings) {
      chosen.push(ToolbarButton.SETTINGS);
    }
    return chosen;
  }

  /**
   * Each button wears the state it is in, not the one it would switch to.
   *
   * 中 while Chinese is being composed, not 英. A toolbar is a readout first and a control second,
   * and a row of buttons naming what they would do says nothing about what is happening now.
   */
  static face(button: ToolbarButton, state: ToolbarState): string {
    switch (button) {
      case ToolbarButton.INPUT_MODE:
        return state.capsLock
          ? "A"
          : state.temporaryEnglish
            ? "En"
            : state.english
              ? "英"
              : state.japanese
                ? "日"
                : state.korean
                  ? "한"
                  : state.vietnamese === true
                    ? "越"
                    : state.tibetan === true
                      ? "藏"
                      : "中";
      case ToolbarButton.PUNCTUATION:
        return state.chinesePunctuation ? "。" : ".";
      case ToolbarButton.FULL_WIDTH:
        return state.fullWidth ? "全" : "半";
      case ToolbarButton.CHARACTER_SET:
        return state.traditional ? "繁" : "简";
      // These open a surface rather than reporting a state, so their faces never change.
      case ToolbarButton.EMOJI:
        return "☺";
      case ToolbarButton.HANDWRITING:
        return "✍";
      case ToolbarButton.SCREEN_KEYBOARD:
        return "⌨";
      case ToolbarButton.VOICE:
        return "🎙";
      default:
        return "⚙";
    }
  }

  /** 宽度随显示出来的按钮个数变。齿轮总在,所以至少是一个。 */
  static widthVp(buttonCount: number): number {
    const count: number = KeyboardGeometry.bounded(buttonCount, 1, MAXIMUM_BUTTONS);
    return Math.round(2 * EDGE_INSET_VP + count * BUTTON_WIDTH_VP + (count - 1) * BUTTON_GAP_VP);
  }

  /** Match the Windows toolbar's four supported scale steps. */
  static scale(value: number): number {
    if (
      !Number.isFinite(value) ||
      value < TOOLBAR_SCALE_VALUES[0] ||
      value > TOOLBAR_SCALE_VALUES[TOOLBAR_SCALE_VALUES.length - 1]
    ) {
      return 1;
    }
    let nearest: number = 1;
    let distance: number = Math.abs(value - nearest);
    for (const candidate of TOOLBAR_SCALE_VALUES) {
      const candidateDistance: number = Math.abs(value - candidate);
      if (candidateDistance < distance) {
        nearest = candidate;
        distance = candidateDistance;
      }
    }
    return nearest;
  }

  /** Keep a stale or hand-written preference from producing an unreadable toolbar. */
  static fontSize(value: number): number {
    if (
      !Number.isInteger(value) ||
      value < TOOLBAR_FONT_SIZE_MIN ||
      value > TOOLBAR_FONT_SIZE_MAX
    ) {
      return TOOLBAR_FONT_SIZE_DEFAULT;
    }
    return value;
  }

  static buttonWidthVp(scale: number = 1): number {
    return BUTTON_WIDTH_VP * FloatingToolbarLayout.scale(scale);
  }

  static buttonGapVp(scale: number = 1): number {
    return BUTTON_GAP_VP * FloatingToolbarLayout.scale(scale);
  }

  static edgeInsetVp(scale: number = 1): number {
    return EDGE_INSET_VP * FloatingToolbarLayout.scale(scale);
  }

  static heightVp(scale: number = 1): number {
    return TOOLBAR_HEIGHT_VP * FloatingToolbarLayout.scale(scale);
  }
}
