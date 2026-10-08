/**
 * 工具栏 logo 键打开的功能面板：有哪些格子、顺序如何、叫什么。视图按四列两行分页。
 *
 * 顺序是设计稿的十四个格子（`全平台 UI.dc.html` 的功能面板），后接鸿蒙键盘自己的额外项，先例是 `platforms/android/java/app/msime/android/keyboard/FunctionPanelModel.java` 的做法。这里只说明有什么、叫什么；开关是否打开、点按做什么归视图管，按 `id` 对应。
 */
import { HapticStrength, KeyboardFeedback } from "./KeyboardFeedback";
import { KeyboardIconPaths } from "./KeyboardIconPaths";
import { KeyAccessibilityPolicy } from "./input/KeyAccessibilityPolicy";

/** 一个格子。格子画 `glyph`（描边方框里的一个字符）或 `icon`（24 视框的线条图标）其中之一，不会两者都画。 */
export interface FunctionPanelItem {
  readonly id: string;
  /** 格子下方的文字。 */
  readonly label: string;
  /** 读屏器播报的内容，即标签本身，除非单看标签有歧义（繁体输出、按键振动）或键盘已为该操作命名（打开设置、生成高情商回复）。 */
  readonly accessibilityLabel: string;
  readonly glyph?: string;
  /** 来自 `KeyboardIconPaths` 的 SVG 路径数据，用它的 `PANEL_STROKE` 绘制。 */
  readonly icon?: string;
  /** 开关：带「已开启」/「已关闭」，打开时绘制强调色和勾选角标。 */
  readonly toggle: boolean;
}

function toggleGlyph(
  id: string,
  label: string,
  glyph: string,
  accessibilityLabel: string,
): FunctionPanelItem {
  return {
    id: id,
    label: label,
    accessibilityLabel: accessibilityLabel,
    glyph: glyph,
    toggle: true,
  };
}

function toggleIcon(
  id: string,
  label: string,
  icon: string,
  accessibilityLabel: string,
): FunctionPanelItem {
  return { id: id, label: label, accessibilityLabel: accessibilityLabel, icon: icon, toggle: true };
}

function actionGlyph(
  id: string,
  label: string,
  glyph: string,
  accessibilityLabel: string = label,
): FunctionPanelItem {
  return {
    id: id,
    label: label,
    accessibilityLabel: accessibilityLabel,
    glyph: glyph,
    toggle: false,
  };
}

function actionIcon(
  id: string,
  label: string,
  icon: string,
  accessibilityLabel: string = label,
): FunctionPanelItem {
  return {
    id: id,
    label: label,
    accessibilityLabel: accessibilityLabel,
    icon: icon,
    toggle: false,
  };
}

export class FunctionPanelPolicy {
  static readonly COLUMNS: number = 4;
  static readonly ROWS: number = 2;
  static readonly PAGE_SIZE: number = FunctionPanelPolicy.COLUMNS * FunctionPanelPolicy.ROWS;

  static readonly FULL_WIDTH: string = "full_width";
  static readonly CHINESE_PUNCTUATION: string = "chinese_punctuation";
  static readonly FUZZY_PINYIN: string = "fuzzy_pinyin";
  static readonly TRADITIONAL: string = "traditional";
  static readonly HANDWRITING: string = "handwriting";
  static readonly DICTIONARY: string = "dictionary";
  static readonly KEYBOARD_HEIGHT: string = "keyboard_height";
  static readonly SETTINGS: string = "settings";
  static readonly KEY_SOUND: string = "key_sound";
  static readonly VIBRATION: string = "vibration";
  static readonly ONE_HAND: string = "one_hand";
  static readonly PRIVACY: string = "privacy";
  static readonly FEEDBACK: string = "feedback";
  static readonly ABOUT: string = "about";
  static readonly TRANSLATION: string = "translation";
  static readonly SYMBOLS: string = "symbols";
  static readonly LOCAL_INPUT: string = "local_input";
  static readonly VOICE: string = "voice";
  static readonly SMART_REPLY: string = "smart_reply";
  static readonly AI_POLISH: string = "ai_polish";
  static readonly VIBRATION_STRENGTH: string = "vibration_strength";

  static readonly STATE_ON: string = "已开启";
  static readonly STATE_OFF: string = "已关闭";
  static readonly STATE_UNAVAILABLE: string = "不可用";

  private static readonly ITEMS: FunctionPanelItem[] = [
    toggleGlyph(FunctionPanelPolicy.FULL_WIDTH, "全角", "全", "全角"),
    toggleGlyph(FunctionPanelPolicy.CHINESE_PUNCTUATION, "中文标点", "，", "中文标点"),
    toggleGlyph(FunctionPanelPolicy.FUZZY_PINYIN, "模糊音", "≈", "模糊音"),
    toggleGlyph(FunctionPanelPolicy.TRADITIONAL, "繁体", "繁", "繁体输出"),
    toggleIcon(FunctionPanelPolicy.HANDWRITING, "手写", KeyboardIconPaths.HANDWRITING, "手写"),
    actionIcon(FunctionPanelPolicy.DICTIONARY, "词库", KeyboardIconPaths.LEXICON),
    actionIcon(FunctionPanelPolicy.KEYBOARD_HEIGHT, "键盘高度", KeyboardIconPaths.KEYBOARD_HEIGHT),
    actionIcon(
      FunctionPanelPolicy.SETTINGS,
      "设置",
      KeyboardIconPaths.SETTINGS,
      KeyAccessibilityPolicy.settings(),
    ),
    toggleIcon(FunctionPanelPolicy.KEY_SOUND, "按键音", KeyboardIconPaths.KEY_SOUND, "按键音"),
    toggleIcon(FunctionPanelPolicy.VIBRATION, "振动", KeyboardIconPaths.VIBRATION, "按键振动"),
    toggleIcon(FunctionPanelPolicy.ONE_HAND, "单手模式", KeyboardIconPaths.ONE_HAND, "单手模式"),
    toggleIcon(FunctionPanelPolicy.PRIVACY, "隐私模式", KeyboardIconPaths.INCOGNITO, "隐私模式"),
    actionIcon(FunctionPanelPolicy.FEEDBACK, "反馈", KeyboardIconPaths.FEEDBACK),
    actionIcon(FunctionPanelPolicy.ABOUT, "关于", KeyboardIconPaths.ABOUT),
    toggleGlyph(FunctionPanelPolicy.TRANSLATION, "候选翻译", "译", "候选翻译"),
    actionGlyph(FunctionPanelPolicy.SYMBOLS, "符号", "符"),
    actionIcon(FunctionPanelPolicy.LOCAL_INPUT, "本地输入", KeyboardIconPaths.LOCAL_INPUT),
    actionIcon(
      FunctionPanelPolicy.VOICE,
      "语音输入",
      KeyboardIconPaths.VOICE,
      KeyAccessibilityPolicy.voice(),
    ),
    actionGlyph(
      FunctionPanelPolicy.SMART_REPLY,
      "高情商回复",
      "回",
      KeyAccessibilityPolicy.reply(),
    ),
    actionIcon(FunctionPanelPolicy.AI_POLISH, "AI 润色", KeyboardIconPaths.AI_ASSIST),
    actionIcon(
      FunctionPanelPolicy.VIBRATION_STRENGTH,
      "振动强度",
      KeyboardIconPaths.VIBRATION_STRENGTH,
    ),
  ];

  /** 所有格子，按页面顺序。 */
  static items(): FunctionPanelItem[] {
    return FunctionPanelPolicy.ITEMS;
  }

  /** 格子下方的文字：振动强度带上当前强度（振动强度 中），其他格子显示各自的标签。 */
  static label(item: FunctionPanelItem, strength: HapticStrength): string {
    if (item.id === FunctionPanelPolicy.VIBRATION_STRENGTH) {
      return item.label + " " + KeyboardFeedback.title(strength);
    }
    return item.label;
  }

  /** 格子的无障碍状态：禁用时为「不可用」，开关为「已开启」/「已关闭」，操作为空。 */
  static state(item: FunctionPanelItem, on: boolean, enabled: boolean): string {
    if (!enabled) {
      return FunctionPanelPolicy.STATE_UNAVAILABLE;
    }
    if (!item.toggle) {
      return "";
    }
    return on ? FunctionPanelPolicy.STATE_ON : FunctionPanelPolicy.STATE_OFF;
  }
}
