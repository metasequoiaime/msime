/**
 * Key feedback, and where its settings live.
 *
 * The shared preferences store nothing about this: on iOS it is UserDefaults local to the keyboard,
 * because how a key feels is a property of the device it is being typed on rather than of the
 * account. This keeps the same split, storing the choice beside the keyboard's other state.
 *
 * The strengths are the ones the iOS keyboard offers, in the order it cycles them. 「跟随系统」排在最后：它不由这里定强度，而是按系统的触感反馈设置振动。
 */
export enum HapticStrength {
  LIGHT = "light",
  MEDIUM = "medium",
  HEAVY = "heavy",
  SYSTEM = "system",
}

export interface FeedbackSettings {
  readonly sound: boolean;
  readonly haptics: boolean;
  readonly strength: HapticStrength;
  /** 滑行输入, which is stored here because it too belongs to the device rather than the account: the shared preferences have no key for it. Off unless the user turns it on. */
  readonly glideTyping: boolean;
}

/**
 * 一次按键振动怎么发：预置效果加强度，设备不支持该效果时退回按时长振动。
 *
 * 以前三档只差振动时长（10/20/35 ms）、强度固定，短于马达起振的那几毫秒几乎摸不出差别，用户反映三档一个样。现在每档用不同的预置效果和明显拉开的强度，退回时长振动时也把时长拉开。
 */
export interface HapticPlan {
  /** `@ohos.vibrator` 的预置效果 id，先用 `isSupportEffectSync` 确认设备支持。 */
  readonly effectId: string;
  /** 1..100；0 表示不指定，交给系统。 */
  readonly intensity: number;
  /** 不支持预置效果时按时长振动的毫秒数。 */
  readonly fallbackMillis: number;
  /** 真时用 `usage: 'touch'`，振不振、多强由系统的触感反馈设置决定；假时用 `physicalFeedback`，按用户在这里选的档位振。 */
  readonly followsSystem: boolean;
}

const STRENGTHS: HapticStrength[] = [
  HapticStrength.LIGHT,
  HapticStrength.MEDIUM,
  HapticStrength.HEAVY,
  HapticStrength.SYSTEM,
];

const TITLES: Map<HapticStrength, string> = new Map<HapticStrength, string>([
  [HapticStrength.LIGHT, "轻"],
  [HapticStrength.MEDIUM, "中"],
  [HapticStrength.HEAVY, "强"],
  [HapticStrength.SYSTEM, "系统"],
]);

// 三档自选强度各用一个 SDK 的 `HapticFeedback` 预置效果（API 12 起）：soft 松、sharp 脆、hard 沉，再配上拉开的强度。「跟随系统」用最早就有的 `haptic.clock.timer`（`EffectId.EFFECT_CLOCK_TIMER`，调节计时器的那一下），不带强度。
const PLANS: Map<HapticStrength, HapticPlan> = new Map<HapticStrength, HapticPlan>([
  [
    HapticStrength.LIGHT,
    { effectId: "haptic.effect.soft", intensity: 35, fallbackMillis: 8, followsSystem: false },
  ],
  [
    HapticStrength.MEDIUM,
    { effectId: "haptic.effect.sharp", intensity: 70, fallbackMillis: 20, followsSystem: false },
  ],
  [
    HapticStrength.HEAVY,
    { effectId: "haptic.effect.hard", intensity: 100, fallbackMillis: 40, followsSystem: false },
  ],
  [
    HapticStrength.SYSTEM,
    { effectId: "haptic.clock.timer", intensity: 0, fallbackMillis: 20, followsSystem: true },
  ],
]);

export class KeyboardFeedback {
  /** The shared file is a tiny settings document; refuse oversized input before JSON parsing. */
  static readonly MAX_BYTES: number = 4096;

  static readonly DEFAULTS: FeedbackSettings = {
    sound: false,
    haptics: false,
    strength: HapticStrength.MEDIUM,
    glideTyping: false,
  };

  static title(strength: HapticStrength): string {
    const title: string | undefined = TITLES.get(strength);
    return title === undefined ? "中" : title;
  }

  /** 这一档振动怎么发；认不出的值按中档。 */
  static plan(strength: HapticStrength): HapticPlan {
    const plan: HapticPlan | undefined = PLANS.get(strength);
    return plan === undefined ? (PLANS.get(HapticStrength.MEDIUM) as HapticPlan) : plan;
  }

  /** The next strength in the cycle, which is how one card can offer all four values. */
  static nextStrength(strength: HapticStrength): HapticStrength {
    const index: number = STRENGTHS.indexOf(strength);
    return STRENGTHS[(index + 1) % STRENGTHS.length];
  }

  /**
   * Reads a stored document without trusting it. A value this build does not recognise falls back
   * rather than propagating: feedback settings are not worth failing a keyboard over.
   */
  static parse(document: string | null): FeedbackSettings {
    if (
      document === null ||
      document.length === 0 ||
      document.length > KeyboardFeedback.MAX_BYTES
    ) {
      return KeyboardFeedback.DEFAULTS;
    }
    try {
      const raw: FeedbackSettings = JSON.parse(document) as FeedbackSettings;
      return {
        sound: typeof raw.sound === "boolean" ? raw.sound : KeyboardFeedback.DEFAULTS.sound,
        haptics: typeof raw.haptics === "boolean" ? raw.haptics : KeyboardFeedback.DEFAULTS.haptics,
        strength: STRENGTHS.includes(raw.strength)
          ? raw.strength
          : KeyboardFeedback.DEFAULTS.strength,
        glideTyping:
          typeof raw.glideTyping === "boolean"
            ? raw.glideTyping
            : KeyboardFeedback.DEFAULTS.glideTyping,
      };
    } catch (error) {
      return KeyboardFeedback.DEFAULTS;
    }
  }

  static serialize(settings: FeedbackSettings): string {
    return JSON.stringify(settings);
  }
}
