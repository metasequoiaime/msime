/**
 * Key feedback, and where its settings live.
 *
 * The shared preferences store nothing about this: on iOS it is UserDefaults local to the keyboard,
 * because how a key feels is a property of the device it is being typed on rather than of the
 * account. This keeps the same split, storing the choice beside the keyboard's other state.
 *
 * The strengths are the ones the iOS keyboard offers, in the order it cycles them.
 */
export enum HapticStrength {
  LIGHT = 'light',
  MEDIUM = 'medium',
  HEAVY = 'heavy'
}

export interface FeedbackSettings {
  readonly sound: boolean;
  readonly haptics: boolean;
  readonly strength: HapticStrength;
}

const STRENGTHS: HapticStrength[] = [
  HapticStrength.LIGHT, HapticStrength.MEDIUM, HapticStrength.HEAVY
];

const TITLES: Map<HapticStrength, string> = new Map<HapticStrength, string>([
  [HapticStrength.LIGHT, '轻'],
  [HapticStrength.MEDIUM, '中'],
  [HapticStrength.HEAVY, '强']
]);

/** Milliseconds of vibration per strength, which is all the platform takes. */
const DURATIONS: Map<HapticStrength, number> = new Map<HapticStrength, number>([
  [HapticStrength.LIGHT, 10],
  [HapticStrength.MEDIUM, 20],
  [HapticStrength.HEAVY, 35]
]);

export class KeyboardFeedback {
  /** The shared file is a tiny settings document; refuse oversized input before JSON parsing. */
  static readonly MAX_BYTES: number = 4096;

  static readonly DEFAULTS: FeedbackSettings = {
    sound: false, haptics: false, strength: HapticStrength.MEDIUM
  };

  static title(strength: HapticStrength): string {
    const title: string | undefined = TITLES.get(strength);
    return title === undefined ? '中' : title;
  }

  static duration(strength: HapticStrength): number {
    const duration: number | undefined = DURATIONS.get(strength);
    return duration === undefined ? 20 : duration;
  }

  /** The next strength in the cycle, which is how one card can offer three values. */
  static nextStrength(strength: HapticStrength): HapticStrength {
    const index: number = STRENGTHS.indexOf(strength);
    return STRENGTHS[(index + 1) % STRENGTHS.length];
  }

  /**
   * Reads a stored document without trusting it. A value this build does not recognise falls back
   * rather than propagating: feedback settings are not worth failing a keyboard over.
   */
  static parse(document: string | null): FeedbackSettings {
    if (document === null || document.length === 0 || document.length > KeyboardFeedback.MAX_BYTES) {
      return KeyboardFeedback.DEFAULTS;
    }
    try {
      const raw: FeedbackSettings = JSON.parse(document) as FeedbackSettings;
      return {
        sound: typeof raw.sound === 'boolean' ? raw.sound : KeyboardFeedback.DEFAULTS.sound,
        haptics: typeof raw.haptics === 'boolean' ? raw.haptics : KeyboardFeedback.DEFAULTS.haptics,
        strength: STRENGTHS.includes(raw.strength) ? raw.strength : KeyboardFeedback.DEFAULTS.strength
      };
    } catch (error) {
      return KeyboardFeedback.DEFAULTS;
    }
  }

  static serialize(settings: FeedbackSettings): string {
    return JSON.stringify(settings);
  }
}
