/**
 * The shared settings page's view of key feedback, and this host's.
 *
 * The two disagree about one word. The shared DTO names the loudest haptic `strong`, matching what
 * the page shows; the keyboard's own enum calls it `heavy`, matching the Apple naming the touch
 * keyboard's cycling card was ported from. Left to a direct assignment the page would save a
 * strength the keyboard does not recognise, which `KeyboardFeedback.parse` would silently fall back
 * from — the setting would appear to save and the keys would go on feeling the same.
 *
 * Everything here is a translation between two records. The file both processes read is the
 * keyboard's; the settings page is simply a second writer of it.
 */
import { FeedbackSettings, HapticStrength, KeyboardFeedback } from "./KeyboardFeedback";

/** The shared shape, as the page sends and expects it. */
export interface MobileKeyboardFeedback {
  soundEnabled: boolean;
  hapticsEnabled: boolean;
  hapticStrength: string;
  /** 滑行输入. Optional because the account sync writes this file from the three feedback fields alone and must carry the current value over itself; the page always sends it, having loaded it. */
  glideTyping?: boolean;
}

export class KeyboardFeedbackBridge {
  static readonly STRONG: string = "strong";

  static toShared(settings: FeedbackSettings): MobileKeyboardFeedback {
    return {
      soundEnabled: settings.sound,
      hapticsEnabled: settings.haptics,
      hapticStrength:
        settings.strength === HapticStrength.HEAVY
          ? KeyboardFeedbackBridge.STRONG
          : (settings.strength as string),
      glideTyping: settings.glideTyping,
    };
  }

  /**
   * Read what the page sent, falling back per field rather than as a whole: a page from a newer
   * build may name a strength this one has never heard of, and that is not a reason to discard the
   * two switches beside it.
   */
  static fromShared(value: MobileKeyboardFeedback | null): FeedbackSettings {
    if (value === null) {
      return KeyboardFeedback.DEFAULTS;
    }
    return {
      sound:
        typeof value.soundEnabled === "boolean"
          ? value.soundEnabled
          : KeyboardFeedback.DEFAULTS.sound,
      haptics:
        typeof value.hapticsEnabled === "boolean"
          ? value.hapticsEnabled
          : KeyboardFeedback.DEFAULTS.haptics,
      strength: KeyboardFeedbackBridge.strength(value.hapticStrength),
      glideTyping:
        typeof value.glideTyping === "boolean"
          ? value.glideTyping
          : KeyboardFeedback.DEFAULTS.glideTyping,
    };
  }

  /** 页面上的档位名换成键盘的档位；「跟随系统」两边都叫 `system`，认不出的值按中档。 */
  static strength(value: string): HapticStrength {
    if (value === KeyboardFeedbackBridge.STRONG || value === HapticStrength.HEAVY) {
      return HapticStrength.HEAVY;
    }
    if (value === HapticStrength.LIGHT) {
      return HapticStrength.LIGHT;
    }
    if (value === HapticStrength.SYSTEM) {
      return HapticStrength.SYSTEM;
    }
    return HapticStrength.MEDIUM;
  }
}
