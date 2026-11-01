/**
 * Key feedback, and where its settings live.
 *
 * 共享偏好设置不保存这些：在 iOS 上它们存在键盘本地的 UserDefaults 里，因为按键手感属于正在打字的这台设备，而不属于账号。这里沿用同样的划分，把选择和键盘的其他状态存在一起。
 *
 * The strengths are the ones the iOS keyboard offers, in the order it cycles them. 「跟随系统」排在最后：它不由这里定强度，而是按系统的触感反馈设置振动。
 *
 * 手写板的识别延迟和笔迹也出于同样的原因放在这里：它们描述的是这台设备上手写板的行为，共享偏好设置里也没有对应的键。
 *
 * 按键弹出预览和滑动输入符号及其方向同样如此，Android 把它们存在自己的本地设置文件里（`platform.android.key_popup`、`platform.android.swipe_down_symbols`、`platform.android.swipe_symbols_direction`），默认值相同：都开启，向上滑动。
 *
 * 功能面板的单手模式和隐私模式也是如此，对应 Android 的 `platform.android.one_handed`（关）和 `platform.android.incognito`（关）。隐私模式尤其只属于这台设备：Android 从不同步它，这里的账号同步也只携带三个反馈字段。
 *
 * 浮动键盘的开关和位置同样只在本机，对应 Android 的 `platform.android.floating_keyboard`（关）与 `_x` / `_y`（可移动范围里的千分比，默认居中贴底，见 `FloatingKeyboardPolicy`）：位置与这块屏幕的尺寸有关，带到别的设备上没有意义。
 *
 * 「键盘调整」的底部抬高、宽度和水平位置（`KeyboardAdjustPolicy`）也只在本机，理由相同：它们按这块屏幕和握持方式调。键盘高度仍是共享偏好 `touch_keyboard_height_adjustment`，不在这里。
 */
import { SwipeHintPolicy, SwipeSymbolsDirection } from "./input/SwipeHintPolicy";
import { FloatingKeyboardPolicy } from "./FloatingKeyboardPolicy";
import { KeyboardAdjustPolicy } from "./KeyboardAdjustPolicy";
import { OneHandedMode, OneHandedPolicy } from "./OneHandedPolicy";
import { PrivacyGate } from "./PrivacyGate";

export enum HapticStrength {
  LIGHT = "light",
  MEDIUM = "medium",
  HEAVY = "heavy",
  SYSTEM = "system",
}

/** 手写笔迹的颜色。取值是设置页写入的共享 JSON 拼写。 */
export enum HandwritingStrokeColor {
  /** 皮肤的按键文字颜色，这样无论手写板用什么颜色绘制，笔迹都看得清。 */
  FOLLOW_SKIN = "follow_skin",
  BLACK = "black",
  WHITE = "white",
  BLUE = "blue",
}

export interface FeedbackSettings {
  readonly sound: boolean;
  readonly haptics: boolean;
  readonly strength: HapticStrength;
  /** 滑行输入, which is stored here because it too belongs to the device rather than the account: the shared preferences have no key for it. Off unless the user turns it on. */
  readonly glideTyping: boolean;
  /** 手写板在最后一笔之后等待多久再识别，单位 ms：HANDWRITING_DELAY_MIN_MS..HANDWRITING_DELAY_MAX_MS，步长 HANDWRITING_DELAY_STEP_MS。 */
  readonly handwritingDelayMs: number;
  readonly handwritingStrokeColor: HandwritingStrokeColor;
  /** 笔迹宽度，单位 vp，取 HANDWRITING_STROKE_WIDTH_MIN..HANDWRITING_STROKE_WIDTH_MAX 之间的整数。 */
  readonly handwritingStrokeWidth: number;
  /** 按键弹出预览：按下字母键时上方的气泡。除非关闭，否则开启。 */
  readonly keyPopup: boolean;
  /** 滑动输入符号：在字母键上滑动输入其角标提示。除非关闭，否则开启；长按无论如何都会输入角标提示。 */
  readonly swipeSymbols: boolean;
  /** 滑动的方向。 */
  readonly swipeSymbolsDirection: SwipeSymbolsDirection;
  /** 单手模式：按键靠向哪一侧，或关闭。 */
  readonly oneHanded: OneHandedMode;
  /** 隐私模式：开启期间，这里输入的内容都不会被学习、统计或保存（PrivacyGate）。 */
  readonly incognito: boolean;
  /** 浮动键盘：键盘缩成一块可拖动的小面板（`FloatingKeyboardPolicy`）。 */
  readonly floating: boolean;
  /** 浮动面板的位置：水平、竖直方向可移动范围里的千分比（0–1000）。 */
  readonly floatingX: number;
  readonly floatingY: number;
  /** 键盘调整的底部抬高（vp，0–160）。 */
  readonly bottomLift: number;
  /** 键盘调整的宽度（面板宽的百分比，60–100）。 */
  readonly widthPercent: number;
  /** 键盘调整的水平位置（两侧剩余空间里的千分比，0–1000，500 居中）。 */
  readonly positionX: number;
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

const STROKE_COLORS: HandwritingStrokeColor[] = [
  HandwritingStrokeColor.FOLLOW_SKIN,
  HandwritingStrokeColor.BLACK,
  HandwritingStrokeColor.WHITE,
  HandwritingStrokeColor.BLUE,
];

/** 固定的笔迹颜色，ArkUI 顺序。FOLLOW_SKIN 没有条目：它取皮肤的按键文字颜色。 */
export const HANDWRITING_STROKE_COLORS: Map<HandwritingStrokeColor, string> = new Map<
  HandwritingStrokeColor,
  string
>([
  [HandwritingStrokeColor.BLACK, "#000000"],
  [HandwritingStrokeColor.WHITE, "#FFFFFF"],
  [HandwritingStrokeColor.BLUE, "#2F6FDB"],
]);

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

  static readonly HANDWRITING_DELAY_MIN_MS: number = 200;
  static readonly HANDWRITING_DELAY_MAX_MS: number = 1500;
  static readonly HANDWRITING_DELAY_STEP_MS: number = 100;
  static readonly HANDWRITING_STROKE_WIDTH_MIN: number = 1;
  static readonly HANDWRITING_STROKE_WIDTH_MAX: number = 8;

  static readonly DEFAULTS: FeedbackSettings = {
    sound: false,
    haptics: false,
    strength: HapticStrength.MEDIUM,
    glideTyping: false,
    handwritingDelayMs: 600,
    handwritingStrokeColor: HandwritingStrokeColor.FOLLOW_SKIN,
    handwritingStrokeWidth: 3,
    keyPopup: true,
    swipeSymbols: true,
    swipeSymbolsDirection: SwipeSymbolsDirection.UP,
    oneHanded: OneHandedPolicy.DEFAULT,
    incognito: PrivacyGate.DEFAULT,
    floating: false,
    floatingX: FloatingKeyboardPolicy.DEFAULT_X_FRACTION,
    floatingY: FloatingKeyboardPolicy.DEFAULT_Y_FRACTION,
    bottomLift: KeyboardAdjustPolicy.DEFAULT_LIFT_VP,
    widthPercent: KeyboardAdjustPolicy.DEFAULT_WIDTH_PERCENT,
    positionX: KeyboardAdjustPolicy.DEFAULT_POSITION,
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

  /** 对齐到 100 ms 步长并限制在范围内的识别延迟；不是有限数字时取 `fallback`。 */
  static handwritingDelay(
    value: number | null | undefined,
    fallback: number = KeyboardFeedback.DEFAULTS.handwritingDelayMs,
  ): number {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return fallback;
    }
    const step: number = KeyboardFeedback.HANDWRITING_DELAY_STEP_MS;
    const snapped: number = Math.round(value / step) * step;
    return Math.max(
      KeyboardFeedback.HANDWRITING_DELAY_MIN_MS,
      Math.min(snapped, KeyboardFeedback.HANDWRITING_DELAY_MAX_MS),
    );
  }

  /** 四舍五入到整数 vp 并限制在范围内的笔迹宽度；不是有限数字时取 `fallback`。 */
  static handwritingStrokeWidth(
    value: number | null | undefined,
    fallback: number = KeyboardFeedback.DEFAULTS.handwritingStrokeWidth,
  ): number {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return fallback;
    }
    return Math.max(
      KeyboardFeedback.HANDWRITING_STROKE_WIDTH_MIN,
      Math.min(Math.round(value), KeyboardFeedback.HANDWRITING_STROKE_WIDTH_MAX),
    );
  }

  /** 本构建认识的笔迹颜色，其他值一律取 `fallback`。 */
  static handwritingStrokeColor(
    value: string | null | undefined,
    fallback: HandwritingStrokeColor = KeyboardFeedback.DEFAULTS.handwritingStrokeColor,
  ): HandwritingStrokeColor {
    for (const color of STROKE_COLORS) {
      if (value === color) {
        return color;
      }
    }
    return fallback;
  }

  /** 要绘制的笔迹颜色：FOLLOW_SKIN 取皮肤的按键文字颜色，否则取 HANDWRITING_STROKE_COLORS 中的固定颜色。 */
  static strokeInk(color: HandwritingStrokeColor, skinForeground: string): string {
    const fixed: string | undefined = HANDWRITING_STROKE_COLORS.get(color);
    return fixed === undefined ? skinForeground : fixed;
  }

  /**
   * 读取存储的文档，但不信任其内容。本构建不认识的值会回退为默认值而不是继续传递：不值得因为反馈设置让键盘失败。
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
        handwritingDelayMs: KeyboardFeedback.handwritingDelay(raw.handwritingDelayMs),
        handwritingStrokeColor: KeyboardFeedback.handwritingStrokeColor(raw.handwritingStrokeColor),
        handwritingStrokeWidth: KeyboardFeedback.handwritingStrokeWidth(raw.handwritingStrokeWidth),
        keyPopup:
          typeof raw.keyPopup === "boolean" ? raw.keyPopup : KeyboardFeedback.DEFAULTS.keyPopup,
        swipeSymbols:
          typeof raw.swipeSymbols === "boolean"
            ? raw.swipeSymbols
            : KeyboardFeedback.DEFAULTS.swipeSymbols,
        swipeSymbolsDirection: SwipeHintPolicy.direction(raw.swipeSymbolsDirection),
        oneHanded: OneHandedPolicy.parse(raw.oneHanded),
        incognito:
          typeof raw.incognito === "boolean" ? raw.incognito : KeyboardFeedback.DEFAULTS.incognito,
        floating:
          typeof raw.floating === "boolean" ? raw.floating : KeyboardFeedback.DEFAULTS.floating,
        floatingX: FloatingKeyboardPolicy.fraction(
          raw.floatingX,
          KeyboardFeedback.DEFAULTS.floatingX,
        ),
        floatingY: FloatingKeyboardPolicy.fraction(
          raw.floatingY,
          KeyboardFeedback.DEFAULTS.floatingY,
        ),
        bottomLift: KeyboardAdjustPolicy.lift(raw.bottomLift),
        widthPercent: KeyboardAdjustPolicy.widthPercent(raw.widthPercent),
        positionX: KeyboardAdjustPolicy.position(raw.positionX),
      };
    } catch (error) {
      return KeyboardFeedback.DEFAULTS;
    }
  }

  /**
   * 键盘一次读取设置文件的结果：文件不存在时为默认值；文件存在却没读到内容（例如设置应用正以写后改名替换它，或文件不安全）时为 `null`，由调用方保留上次读到的值，这样一次读取失败不会把隐私模式当成关闭。
   */
  static fromRead(exists: boolean, document: string | null): FeedbackSettings | null {
    if (!exists) {
      return KeyboardFeedback.DEFAULTS;
    }
    return document === null ? null : KeyboardFeedback.parse(document);
  }

  /** `settings` 换上浮动键盘的开关和位置，其余原样保留。功能面板磁贴、拖动条的停靠键和拖动松手都经它写回。 */
  static withFloating(
    settings: FeedbackSettings,
    floating: boolean,
    floatingX: number,
    floatingY: number,
  ): FeedbackSettings {
    return {
      sound: settings.sound,
      haptics: settings.haptics,
      strength: settings.strength,
      glideTyping: settings.glideTyping,
      handwritingDelayMs: settings.handwritingDelayMs,
      handwritingStrokeColor: settings.handwritingStrokeColor,
      handwritingStrokeWidth: settings.handwritingStrokeWidth,
      keyPopup: settings.keyPopup,
      swipeSymbols: settings.swipeSymbols,
      swipeSymbolsDirection: settings.swipeSymbolsDirection,
      oneHanded: settings.oneHanded,
      incognito: settings.incognito,
      floating: floating,
      floatingX: FloatingKeyboardPolicy.fraction(floatingX, settings.floatingX),
      floatingY: FloatingKeyboardPolicy.fraction(floatingY, settings.floatingY),
      bottomLift: settings.bottomLift,
      widthPercent: settings.widthPercent,
      positionX: settings.positionX,
    };
  }

  /** `settings` 换上键盘调整的底部抬高、宽度和水平位置，其余原样保留。键盘调整确认或以别的方式离开时经它写回；范围外的值钳回来，不是数时保留原值。 */
  static withAdjust(
    settings: FeedbackSettings,
    bottomLift: number,
    widthPercent: number,
    positionX: number,
  ): FeedbackSettings {
    return {
      sound: settings.sound,
      haptics: settings.haptics,
      strength: settings.strength,
      glideTyping: settings.glideTyping,
      handwritingDelayMs: settings.handwritingDelayMs,
      handwritingStrokeColor: settings.handwritingStrokeColor,
      handwritingStrokeWidth: settings.handwritingStrokeWidth,
      keyPopup: settings.keyPopup,
      swipeSymbols: settings.swipeSymbols,
      swipeSymbolsDirection: settings.swipeSymbolsDirection,
      oneHanded: settings.oneHanded,
      incognito: settings.incognito,
      floating: settings.floating,
      floatingX: settings.floatingX,
      floatingY: settings.floatingY,
      bottomLift: KeyboardAdjustPolicy.lift(bottomLift, settings.bottomLift),
      widthPercent: KeyboardAdjustPolicy.widthPercent(widthPercent, settings.widthPercent),
      positionX: KeyboardAdjustPolicy.position(positionX, settings.positionX),
    };
  }

  static serialize(settings: FeedbackSettings): string {
    return JSON.stringify(settings);
  }
}
