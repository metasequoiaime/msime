/**
 * 工具栏 logo 键打开的功能面板：有哪些格子、顺序如何、叫什么。视图按四列分页，手机竖屏和平板三行（键区太矮时退成两行）、手机横屏两行，磁贴按可用高度等比缩放（`pickerGrid`）。
 *
 * 顺序是设计稿的十四个格子（`全平台 UI.dc.html` 的功能面板），后接鸿蒙键盘自己的额外项，先例是 `platforms/android/java/app/msime/android/keyboard/FunctionPanelModel.java` 的做法。这里只说明有什么、叫什么；开关是否打开、点按做什么归视图管，按 `id` 对应。
 */
import { HapticStrength, KeyboardFeedback } from "./KeyboardFeedback";
import { KeyboardIconPaths } from "./KeyboardIconPaths";
import { KeyAccessibilityPolicy } from "./input/KeyAccessibilityPolicy";

/** 输入方式和功能面板一页的排法，见 `FunctionPanelPolicy.pickerGrid`。 */
export interface PickerGrid {
  /** 每页几行。 */
  readonly rows: number;
  /** 磁贴整体缩放的比例，不超过 1：磁贴里的图标、字形、字号和间距都乘以它。 */
  readonly scale: number;
  /** 缩放后的行高（vp），即满尺寸行高乘以 `scale`；面板比这高时视图让各行平分多出来的高度。 */
  readonly rowHeight: number;
  /** 缩放后的行距（vp）。 */
  readonly rowsGap: number;
}

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
  /** 两行一页：皮肤选择器，以及手机横屏时的功能面板和输入方式。 */
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
  static readonly FLOATING: string = "floating";
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
  static readonly PROMOTION: string = "promotion";

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
    actionIcon(FunctionPanelPolicy.KEYBOARD_HEIGHT, "键盘调整", KeyboardIconPaths.KEYBOARD_HEIGHT),
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
    actionGlyph(FunctionPanelPolicy.PROMOTION, "宣传", "吆"),
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
    // 浮动键盘（#10225），名称与 Android 的 `FunctionPanelModel.Id.FLOATING` 相同。设计稿的十四格之后是鸿蒙自己的额外项，新增的排在最后，不挪动用户熟悉的位置。
    toggleIcon(FunctionPanelPolicy.FLOATING, "浮动键盘", KeyboardIconPaths.FLOATING, "浮动键盘"),
  ];

  /** 三行一页时满尺寸的行距（vp）。 */
  static readonly THREE_ROW_GAP_VP: number = 12;
  /** 两行一页时满尺寸的行距（vp）。 */
  static readonly TWO_ROW_GAP_VP: number = 16;
  /** 缩放后磁贴高度的下限（vp），即触摸目标的最小尺寸；三行要缩到比它还矮时改为两行。 */
  static readonly MIN_TILE_VP: number = 44;

  /**
   * 输入方式和功能面板每页几行、磁贴缩放多少，与 Android `PagedTileGrid.pickerGrid` 和 iOS `KeyboardPagedGridView.pickerLayout` 是同一条规则、同一组常数：按网格实际能占的高度排，永远不超出面板；放不下满尺寸时整体等比缩小磁贴，只有三行要缩到磁贴矮于 44vp 时才退成两行。手机横屏只排两行。
   *
   * 三行的满尺寸高度是三个行高加两份 12vp 行距，两行是两个行高加一份 16vp 行距；比例取可用高度与满尺寸之比，最大为 1。两行的比例不低于 44 与行高之比，行高本身不到 44 时不缩小。鸿蒙的键区默认 197vp（四行 44vp 加三份 7vp 行距），最矮 176vp（高度调整 -12、行距 4），去掉上方留白、页点和下方留白 28vp 后，默认高度下三行磁贴缩到约 0.94，最矮时三行会矮于 44vp，退成满尺寸的两行。
   *
   * `gridVp` 是网格的行能占的高度（面板高度去掉上方留白、页点和下方留白），`rowHeightVp` 是满尺寸的行高。
   */
  static pickerGrid(phoneLandscape: boolean, gridVp: number, rowHeightVp: number): PickerGrid {
    if (!phoneLandscape) {
      const full3: number = 3 * rowHeightVp + 2 * FunctionPanelPolicy.THREE_ROW_GAP_VP;
      const scale3: number = Math.min(1, gridVp / full3);
      if (rowHeightVp * scale3 >= FunctionPanelPolicy.MIN_TILE_VP - 1e-6) {
        return FunctionPanelPolicy.scaledGrid(
          3,
          scale3,
          rowHeightVp,
          FunctionPanelPolicy.THREE_ROW_GAP_VP,
        );
      }
    }
    const full2: number = 2 * rowHeightVp + FunctionPanelPolicy.TWO_ROW_GAP_VP;
    const minScale: number = Math.min(1, FunctionPanelPolicy.MIN_TILE_VP / rowHeightVp);
    const scale2: number = Math.max(minScale, Math.min(1, gridVp / full2));
    return FunctionPanelPolicy.scaledGrid(
      2,
      scale2,
      rowHeightVp,
      FunctionPanelPolicy.TWO_ROW_GAP_VP,
    );
  }

  private static scaledGrid(
    rows: number,
    scale: number,
    rowHeightVp: number,
    gapVp: number,
  ): PickerGrid {
    return { rows: rows, scale: scale, rowHeight: rowHeightVp * scale, rowsGap: gapVp * scale };
  }

  /** 把用户文字发给大模型的两个格子（高情商回复、AI 润色），总闸 `AiGenerationGate` 关着时不列出。 */
  static readonly AI_GENERATION_TILES: readonly string[] = [
    FunctionPanelPolicy.SMART_REPLY,
    FunctionPanelPolicy.AI_POLISH,
  ];

  // 静态常量：每次渲染拿到同一个数组，分页网格不会因此重建。
  private static readonly ITEMS_WITHOUT_AI_GENERATION: FunctionPanelItem[] =
    FunctionPanelPolicy.ITEMS.filter(
      (item: FunctionPanelItem) => !FunctionPanelPolicy.AI_GENERATION_TILES.includes(item.id),
    );

  /** 手机和平板不显示「本地输入」入口；2in1 仍保留实体键盘对应的设置和入口。 */
  private static readonly ITEMS_WITHOUT_LOCAL_INPUT: FunctionPanelItem[] =
    FunctionPanelPolicy.ITEMS.filter(
      (item: FunctionPanelItem) => item.id !== FunctionPanelPolicy.LOCAL_INPUT,
    );

  private static readonly ITEMS_WITHOUT_AI_OR_LOCAL_INPUT: FunctionPanelItem[] =
    FunctionPanelPolicy.ITEMS_WITHOUT_AI_GENERATION.filter(
      (item: FunctionPanelItem) => item.id !== FunctionPanelPolicy.LOCAL_INPUT,
    );

  /** 所有格子，按页面顺序。手机和平板隐藏本地输入入口；`aiGeneration` 为 false 时去掉 AI 生成类磁贴。 */
  static items(aiGeneration: boolean = true, localInputModes: boolean = true): FunctionPanelItem[] {
    if (!localInputModes) {
      return aiGeneration
        ? FunctionPanelPolicy.ITEMS_WITHOUT_LOCAL_INPUT
        : FunctionPanelPolicy.ITEMS_WITHOUT_AI_OR_LOCAL_INPUT;
    }
    return aiGeneration
      ? FunctionPanelPolicy.ITEMS
      : FunctionPanelPolicy.ITEMS_WITHOUT_AI_GENERATION;
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
