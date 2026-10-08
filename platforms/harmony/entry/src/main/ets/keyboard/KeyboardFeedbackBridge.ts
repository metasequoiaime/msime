/**
 * 共享设置页面眼中的按键反馈，以及本宿主眼中的按键反馈。
 *
 * 两者在一个词上有分歧。共享 DTO 把最强的触感叫作 `strong`，与页面上显示的一致；键盘自己的枚举叫它 `heavy`，沿用触屏键盘循环卡片移植来源的 Apple 命名。如果直接赋值，页面会保存一个键盘不认识的强度，`KeyboardFeedback.parse` 会悄悄回退——设置看起来保存成功了，按键手感却一点没变。
 *
 * 这里做的都是两种记录之间的转换。两个进程都读取的文件归键盘所有；设置页面只是它的第二个写入方。
 *
 * 单手模式 和 隐私模式 也存在这个文件里，但根本不属于共享结构：只有键盘的功能面板会设置它们。其他所有写入方都经过 `fromShared`，它从作为回退值传入的已存设置里沿用这两项，所以页面或账号同步的保存永远不会背着用户关掉隐私模式。
 */
import { FeedbackSettings, HapticStrength, KeyboardFeedback } from "./KeyboardFeedback";
import { SwipeHintPolicy } from "./input/SwipeHintPolicy";

/** The shared shape, as the page sends and expects it. */
export interface MobileKeyboardFeedback {
  soundEnabled: boolean;
  hapticsEnabled: boolean;
  hapticStrength: string;
  /** 滑行输入. Optional because the account sync writes this file from the three feedback fields alone and must carry the current value over itself; the page always sends it, having loaded it. */
  glideTyping?: boolean;
  /** 手写板的识别延迟、笔迹颜色和笔迹粗细。可选的原因与 `glideTyping` 相同：不知道这些字段的写入方会省略它们，`fromShared` 随后保留回退值里的内容。 */
  handwritingDelayMs?: number;
  handwritingStrokeColor?: string;
  handwritingStrokeWidth?: number;
  /** 按键弹出预览、滑动输入符号及其方向（`down` 或 `up`）。可选的原因同上。 */
  keyPopup?: boolean;
  swipeSymbols?: boolean;
  swipeSymbolsDirection?: string;
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
      handwritingDelayMs: settings.handwritingDelayMs,
      handwritingStrokeColor: settings.handwritingStrokeColor as string,
      handwritingStrokeWidth: settings.handwritingStrokeWidth,
      keyPopup: settings.keyPopup,
      swipeSymbols: settings.swipeSymbols,
      swipeSymbolsDirection: settings.swipeSymbolsDirection as string,
    };
  }

  /**
   * 读取页面发来的内容，逐字段回退而不是整体回退：较新版本的页面可能给出本版本从未见过的强度，这不该成为丢掉旁边两个开关的理由。
   *
   * 缺失或无法读取的字段取 `fallback` 中的值；除非调用方传入已存设置，否则它就是默认值。只知道部分字段的写入方（账号同步只知道三个反馈字段）会传入已存设置，让其余字段在它写入后得以保留。无论回退值是什么，未知强度都按 `MEDIUM` 处理，和以前一样。
   */
  static fromShared(
    value: MobileKeyboardFeedback | null,
    fallback: FeedbackSettings = KeyboardFeedback.DEFAULTS,
  ): FeedbackSettings {
    if (value === null) {
      return fallback;
    }
    return {
      sound: typeof value.soundEnabled === "boolean" ? value.soundEnabled : fallback.sound,
      haptics: typeof value.hapticsEnabled === "boolean" ? value.hapticsEnabled : fallback.haptics,
      strength: KeyboardFeedbackBridge.strength(value.hapticStrength),
      glideTyping:
        typeof value.glideTyping === "boolean" ? value.glideTyping : fallback.glideTyping,
      handwritingDelayMs: KeyboardFeedback.handwritingDelay(
        value.handwritingDelayMs,
        fallback.handwritingDelayMs,
      ),
      handwritingStrokeColor: KeyboardFeedback.handwritingStrokeColor(
        value.handwritingStrokeColor,
        fallback.handwritingStrokeColor,
      ),
      handwritingStrokeWidth: KeyboardFeedback.handwritingStrokeWidth(
        value.handwritingStrokeWidth,
        fallback.handwritingStrokeWidth,
      ),
      keyPopup: typeof value.keyPopup === "boolean" ? value.keyPopup : fallback.keyPopup,
      swipeSymbols:
        typeof value.swipeSymbols === "boolean" ? value.swipeSymbols : fallback.swipeSymbols,
      swipeSymbolsDirection: SwipeHintPolicy.direction(
        value.swipeSymbolsDirection,
        fallback.swipeSymbolsDirection,
      ),
      oneHanded: fallback.oneHanded,
      incognito: fallback.incognito,
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
