/**
 * 触屏键盘的线条图标，以 SVG 路径数据给出，供放在 `Shape` 里的 ArkUI `Path` 使用，`Shape` 的 viewPort 就是图标的 view box。
 *
 * 除品牌标志外，每个图标都画在 24 x 24 的 view box 里，用圆头圆角、无填充的描边。路径取自 `platforms/android/scripts/generate_keyboard_icons.py` 里的 `ICONS`，加号和五个工具栏轮廓取自 `IOS_ICONS`（即 `全平台 UI.dc.html` 中设计稿的 Harmony `tbIcons`），并经该脚本自带的转换器改写成绝对坐标的 moveto / lineto / cubic / close 命令：椭圆弧都已转成三次贝塞尔曲线，相对命令和简写命令都已展开，绘制端只需支持 `M`、`L`、`C` 和 `Z`。
 *
 * 描边宽度以 view box 单位计，紧挨着所属路径定义，调用方不会把路径配错粗细。
 */
export class KeyboardIconPaths {
  /** 所有线条图标共用的 view box。 */
  static readonly VIEW_BOX: number = 24;

  // ---- 功能面板（设计稿 §6.4），描边 `PANEL_STROKE` ----

  static readonly PANEL_STROKE: number = 1.7;
  /** 手写 */
  static readonly HANDWRITING: string =
    "M12 20L21 20M16.4 3.6C16.9359 3.0641 17.717 2.8548 18.449 3.051C19.1811 3.2471 19.7529 3.8189 19.949 4.551C20.1452 5.283 19.9359 6.0641 19.4 6.6L7 19L3 20L4 16Z";
  /** 词库 */
  static readonly LEXICON: string =
    "M2 4L8 4C10.2091 4 12 5.7909 12 8L12 21C12 19.3431 10.6569 18 9 18L2 18ZM22 4L16 4C13.7909 4 12 5.7909 12 8L12 21C12 19.3431 13.3431 18 15 18L22 18Z";
  /** 键盘高度 */
  static readonly KEYBOARD_HEIGHT: string =
    "M12 22L12 16M12 8L12 2M4 12L2 12M10 12L8 12M16 12L14 12M22 12L20 12M15 19L12 22L9 19M15 5L12 2L9 5";
  /** 设置 */
  static readonly SETTINGS: string =
    "M20 7L11 7M14 17L5 17M17 20C18.6569 20 20 18.6569 20 17C20 15.3431 18.6569 14 17 14C15.3431 14 14 15.3431 14 17C14 18.6569 15.3431 20 17 20ZM7 10C8.6569 10 10 8.6569 10 7C10 5.3431 8.6569 4 7 4C5.3431 4 4 5.3431 4 7C4 8.6569 5.3431 10 7 10Z";
  /** 按键音 */
  static readonly KEY_SOUND: string =
    "M11 4.7C11.0042 4.4137 10.8336 4.1538 10.5693 4.0437C10.305 3.9336 10.0003 3.9955 9.8 4.2L6.4 7.6C6.1333 7.8613 5.7733 8.0053 5.4 8L3 8C2.4477 8 2 8.4477 2 9L2 15C2 15.5523 2.4477 16 3 16L5.4 16C5.7733 15.9947 6.1333 16.1387 6.4 16.4L9.8 19.8C10.0003 20.0045 10.305 20.0664 10.5693 19.9563C10.8336 19.8462 11.0042 19.5863 11 19.3ZM16 9C17.3333 10.7778 17.3333 13.2222 16 15M19.4 18.4C21.11 16.7093 22.0723 14.4047 22.0723 12C22.0723 9.5953 21.11 7.2907 19.4 5.6";
  /** 振动 */
  static readonly VIBRATION: string =
    "M2 8L4 10L2 12L4 14L2 16M22 8L20 10L22 12L20 14L22 16M9 5L15 5C15.5523 5 16 5.4477 16 6L16 18C16 18.5523 15.5523 19 15 19L9 19C8.4477 19 8 18.5523 8 18L8 6C8 5.4477 8.4477 5 9 5Z";
  /** 单手模式 */
  static readonly ONE_HAND: string =
    "M18 11L18 6C18 4.8954 17.1046 4 16 4C14.8954 4 14 4.8954 14 6M14 10L14 4C14 2.8954 13.1046 2 12 2C10.8954 2 10 2.8954 10 4L10 6M10 10.5L10 6C10 4.8954 9.1046 4 8 4C6.8954 4 6 4.8954 6 6L6 14M18 8C18 6.8954 18.8954 6 20 6C21.1046 6 22 6.8954 22 8L22 14C22 18.4183 18.4183 22 14 22L12 22C9.2 22 7.5 21.14 6 19.66L2.4 16.06C1.6854 15.2686 1.7174 14.0556 2.4727 13.303C3.2281 12.5503 4.4411 12.5226 5.23 13.24L7 15";
  /** 隐私模式（带对勾的盾牌） */
  static readonly INCOGNITO: string =
    "M20 13C20 18 16.5 20.5 12.34 21.95C12.1222 22.0238 11.8855 22.0203 11.67 21.94C7.5 20.5 4 18 4 13L4 6C4 5.4477 4.4477 5 5 5C7 5 9.5 3.8 11.24 2.28C11.6777 1.9061 12.3223 1.9061 12.76 2.28C14.51 3.81 17 5 19 5C19.5523 5 20 5.4477 20 6ZM9 12L11 14L15 10";
  /** 反馈 */
  static readonly FEEDBACK: string =
    "M21 15C21 16.1046 20.1046 17 19 17L7 17L3 21L3 5C3 3.8954 3.8954 3 5 3L19 3C20.1046 3 21 3.8954 21 5ZM13 8L7 8M17 12L7 12";
  /** 关于 */
  static readonly ABOUT: string =
    "M12 22C17.5228 22 22 17.5228 22 12C22 6.4772 17.5228 2 12 2C6.4772 2 2 6.4772 2 12C2 17.5228 6.4772 22 12 22ZM12 16L12 12M12 8L12.01 8";
  /** AI 润色（Lucide 风格的 sparkles；设计稿没有为它画图标）。 */
  static readonly AI_ASSIST: string =
    "M12 3L13.9 8.1L19 10L13.9 11.9L12 17L10.1 11.9L5 10L10.1 8.1ZM19 15L19.8 17.2L22 18L19.8 18.8L19 21L18.2 18.8L16 18L18.2 17.2Z";
  /** 本地输入（Lucide 风格的 laptop）。 */
  static readonly LOCAL_INPUT: string =
    "M4 6C4 4.8954 4.8954 4 6 4L18 4C19.1046 4 20 4.8954 20 6L20 15L4 15ZM2 19L22 19";
  /** 语音输入（Lucide 风格的 mic，对应 Android 的 `VOICE_RESULT`）。 */
  static readonly VOICE: string =
    "M9 5C9 3.3431 10.3431 2 12 2C13.6569 2 15 3.3431 15 5L15 11C15 12.6569 13.6569 14 12 14C10.3431 14 9 12.6569 9 11ZM5 10C5 13.866 8.134 17 12 17C15.866 17 19 13.866 19 10M12 17L12 21M8 21L16 21";
  /** 振动强度（Lucide 风格的 activity）。 */
  static readonly VIBRATION_STRENGTH: string = "M22 12L18 12L15 21L9 3L6 12L2 12";
  /** 方案选择器里的「添加语言」（Lucide 风格的 plus）。 */
  static readonly PLUS: string = "M12 5L12 19M5 12L19 12";

  /** 开启状态角标里的对勾，描边加粗，在 9vp 下也看得清。 */
  static readonly CHECK_STROKE: number = 4;
  static readonly CHECK: string = "M5 12.5L9.5 17L19 7.5";

  // ---- 按键（设计稿 §6.3），描边 `KEY_STROKE` ----

  static readonly KEY_STROKE: number = 1.7;
  static readonly SHIFT: string = "M12 4.5L4.5 12.5L8.5 12.5L8.5 19L15.5 19L15.5 12.5L19.5 12.5Z";
  static readonly CAPS_LOCK: string =
    "M12 4.5L4.5 12.5L8.5 12.5L8.5 16L15.5 16L15.5 12.5L19.5 12.5ZM8.5 19.5L15.5 19.5";
  static readonly BACKSPACE: string =
    "M9 5.5L19.5 5.5C20.3284 5.5 21 6.1716 21 7L21 17C21 17.8284 20.3284 18.5 19.5 18.5L9 18.5L3 12ZM11.5 9.5L16.5 14.5M16.5 9.5L11.5 14.5";
  static readonly RETURN: string =
    "M19 6L19 11.5C19 12.6046 18.1046 13.5 17 13.5L6 13.5M9.5 10L6 13.5L9.5 17";
  /** 空格键上的麦克风。 */
  static readonly MIC: string =
    "M12 5C13.1046 5 14 5.8954 14 7L14 11C14 12.1046 13.1046 13 12 13C10.8954 13 10 12.1046 10 11L10 7C10 5.8954 10.8954 5 12 5ZM8 10.5C8 12.7091 9.7909 14.5 12 14.5C14.2091 14.5 16 12.7091 16 10.5M12 14.5L12 17";

  // ---- 单手模式侧栏（`OneHandedPolicy`），描边 `OneHandedPolicy.ICON_STROKE` ----

  /** 按键靠右时的「换到另一侧」：朝左的箭头（对应 Android 的 `SWAP_SIDE`）。 */
  static readonly SWAP_TO_LEFT: string = "M15 18L9 12L15 6";
  /** 按键靠左时的「换到另一侧」：朝右的箭头。 */
  static readonly SWAP_TO_RIGHT: string = "M9 18L15 12L9 6";
  /** 退出单手（Lucide 风格的 maximize-2，对应 Android 的 `EXIT_ONE_HAND`）。 */
  static readonly EXIT_ONE_HAND: string = "M15 3L21 3L21 9M9 21L3 21L3 15M21 3L14 10M3 21L10 14";

  // ---- 箭头，描边 `CHEVRON_STROKE` ----

  static readonly CHEVRON_STROKE: number = 1.8;
  /** 工具栏的「收起」键，按 `COLLAPSE_SIZE_VP` 绘制。 */
  static readonly COLLAPSE: string = "M6 9L12 15L18 9";
  static readonly COLLAPSE_SIZE_VP: number = 21;
  /** 候选栏的展开箭头，按 `EXPAND_SIZE_VP` 绘制，候选网格展开时旋转 180°。 */
  static readonly EXPAND: string = "M6 9.5L12 15.5L18 9.5";
  static readonly EXPAND_SIZE_VP: number = 20;

  // ---- 工具栏（设计稿的 Harmony `tbIcons`），描边 `TOOLBAR_STROKE`，尺寸 `TOOLBAR_SIZE_VP` ----

  static readonly TOOLBAR_STROKE: number = 1.45;
  static readonly TOOLBAR_SIZE_VP: number = 23;
  /** 表情 */
  static readonly TOOLBAR_EMOJI: string =
    "M12 21C16.9706 21 21 16.9706 21 12C21 7.0294 16.9706 3 12 3C7.0294 3 3 7.0294 3 12C3 16.9706 7.0294 21 12 21ZM8.5 14C8.5 14 9.8 15.8 12 15.8C14.2 15.8 15.5 14 15.5 14M9 9.5L9.01 9.5M15 9.5L15.01 9.5";
  /** 常用语 */
  static readonly TOOLBAR_PHRASE: string =
    "M5 4L19 4C20.1046 4 21 4.8954 21 6L21 15C21 16.1046 20.1046 17 19 17L9 17L4 21L4 6C3.8947 5.1919 4.2903 4.4007 5 4ZM8.5 9L15.5 9M8.5 12.5L13 12.5";
  /** 剪贴板 */
  static readonly TOOLBAR_CLIPBOARD: string =
    "M15 4.5L17 4.5C18.1046 4.5 19 5.3954 19 6.5L19 19C19 20.1046 18.1046 21 17 21L7 21C5.8954 21 5 20.1046 5 19L5 6.5C5 5.3954 5.8954 4.5 7 4.5L9 4.5M9.5 3L14.5 3C14.7761 3 15 3.2239 15 3.5L15 5.5C15 5.7761 14.7761 6 14.5 6L9.5 6C9.2239 6 9 5.7761 9 5.5L9 3.5C9 3.2239 9.2239 3 9.5 3ZM9 11.5L15 11.5M9 15.5L13 15.5";
  /** 皮肤 */
  static readonly TOOLBAR_SKIN: string =
    "M12 3C7.0294 3 3 7.0294 3 12C3 16.9706 7.0294 21 12 21C13.1 21 13.7 20.2 13.7 19.2C13.7 18.7 13.5 18.3 13.2 17.9C12.7315 17.3766 12.6103 16.6284 12.8896 15.9838C13.1689 15.3393 13.7977 14.9161 14.5 14.9L17 14.9C19.2091 14.9 21 13.1091 21 10.9C21 6.4 17 3 12 3ZM7.5 11.5L7.51 11.5M9.5 7.5L9.51 7.5M14.5 7.5L14.51 7.5M17 11L17.01 11";
  /** 输入方式 */
  static readonly TOOLBAR_SCHEME: string =
    "M4.5 5.5L19.5 5.5C20.6046 5.5 21.5 6.3954 21.5 7.5L21.5 16.5C21.5 17.6046 20.6046 18.5 19.5 18.5L4.5 18.5C3.3954 18.5 2.5 17.6046 2.5 16.5L2.5 7.5C2.5 6.3954 3.3954 5.5 4.5 5.5ZM6.5 9.5L6.51 9.5M9.5 9.5L9.51 9.5M12.5 9.5L12.51 9.5M15.5 9.5L15.51 9.5M17.5 12.5L17.51 12.5M6.5 12.5L6.51 12.5M8.5 15L15.5 15";

  // ---- 品牌标志（`assets/msime_frame.svg` 及设计稿叠在其上的描边），view box 为 `BRAND_VIEW_WIDTH` x `BRAND_VIEW_HEIGHT` ----

  static readonly BRAND_VIEW_WIDTH: number = 116;
  static readonly BRAND_VIEW_HEIGHT: number = 132;
  /** 外框（设计稿写作 `M5.84314 5.8335H109.843V125.833H5.84314Z`），用皮肤的 `logoBg` 填充。 */
  static readonly BRAND_FRAME: string =
    "M5.84314 5.8335L109.843 5.8335L109.843 125.833L5.84314 125.833Z";
  /** 叠在外框上的描边：白色，宽 `BRAND_STROKE_WIDTH`，圆头。 */
  static readonly BRAND_STROKE: string =
    "M80.394 18.8335L34.3451 36.489L80.394 49.7306L34.3451 71.7999C77.8789 79.1564 118.8 85.1887 31.8431 113.088";
  static readonly BRAND_STROKE_WIDTH: number = 9;
  static readonly BRAND_STROKE_COLOR: string = "#FFFFFF";
}
