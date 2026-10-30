/**
 * 单手模式：按键贴向哪一侧，功能面板磁贴和侧边栏的两个按钮把它切换成什么，以及开启时按键区域如何划分。
 *
 * 移植自本键盘所遵循的先例 Android：存储值及其默认值是 `AndroidLocalSettings` 中的 `platform.android.one_handed`（`off`、`left` 或 `right`，默认 `off`），磁贴的点按和长按是 `MSIMEInputService.toggleOneHanded`，侧边栏是 `ImeFrame` 加 `OneHandGutterView`。侧边栏位于按键让开的那一侧，占宽度的 15%；工具栏和候选条保持全宽。侧边栏的尺寸取自设计稿（全平台 UI.dc.html 的单手按键区）：40vp 按钮间隔 14vp，20vp 图标描边 1.9，侧边栏与按键间距 6vp，按下缩放 .9。
 *
 * 大屏上（平板、折叠屏内屏）沿用手机的 15% 会让按键有上千 vp 宽，单手够不着（#10202）。所以大屏上「侧栏 + 间距 + 按键」这一块按按键 360vp 算（`blockWidthVp`），贴到按键所在的那一侧，另一侧空着，与 Android 的 `KeyboardFormFactorPolicy.oneHandedBlockWidthDp` 相同。
 *
 * 大屏上空出来的那一侧可以放侧边面板（共享偏好 `touch_one_handed_side_panel`，#10203）：第一版只有数字键盘，贴在屏幕边上，宽度见 `sidePadWidthVp`，什么时候画见 `sidePadShown`。
 *
 * 只有按键会变窄。替换按键的面板（功能面板、表情、剪贴板等）全宽绘制，就像 Android 的侧边栏在没有按键行显示时隐藏一样。2in1 在条带上不绘制自己的触摸按键，所以该模式在那里从不生效。
 */
export enum OneHandedMode {
  OFF = "off",
  LEFT = "left",
  RIGHT = "right",
}

/** 大屏单手模式空出来的那一侧放什么（共享偏好 `touch_one_handed_side_panel`）：空着，或数字键盘。以后的文本编辑面板会是另一个取值，本版本读到它按空着画。 */
export enum OneHandedSidePanel {
  OFF = "off",
  NUMBER_PAD = "number_pad",
}

const MODES: OneHandedMode[] = [OneHandedMode.OFF, OneHandedMode.LEFT, OneHandedMode.RIGHT];

export class OneHandedPolicy {
  static readonly DEFAULT: OneHandedMode = OneHandedMode.OFF;
  /** 侧边栏占按键区域宽度的百分比（`OneHandGutterView.GUTTER_FRACTION`）。 */
  static readonly RAIL_PERCENT: number = 15;
  /** 侧边栏与按键之间的间距。 */
  static readonly RAIL_GAP_VP: number = 6;
  static readonly BUTTON_VP: number = 40;
  /** 两个按钮之间的间距。 */
  static readonly BUTTON_GAP_VP: number = 14;
  static readonly ICON_VP: number = 20;
  static readonly ICON_STROKE: number = 1.9;
  static readonly PRESSED_SCALE: number = 0.9;
  /** 大屏上按键最宽多少，与 Android 的 `ONE_HANDED_KEYS_MAX_WIDTH_DP` 相同。 */
  static readonly KEYS_MAX_WIDTH_VP: number = 360;
  /** 侧边数字键盘最宽多少：三列每列约 100vp，与九键数字层的键差不多大，与 Android、iPad 相同。 */
  static readonly SIDE_PAD_MAX_WIDTH_VP: number = 300;
  /** 侧边数字键盘最窄多少：三列每列约 44vp 的最小可按宽度，再窄就不画。 */
  static readonly SIDE_PAD_MIN_WIDTH_VP: number = 150;

  /**
   * 单手模式下「侧栏 + 间距 + 按键」这一块的宽度（vp）。返回 0 表示手机：铺满整行，侧栏照旧占 15%。
   * 大屏上按键取 `KEYS_MAX_WIDTH_VP`，侧栏仍占这一块的 15%；屏幕窄到放不下时整行都给它。
   */
  static blockWidthVp(screenWidthVp: number, largeScreen: boolean): number {
    if (!largeScreen) {
      return 0;
    }
    const block: number = Math.round(
      (OneHandedPolicy.KEYS_MAX_WIDTH_VP + OneHandedPolicy.RAIL_GAP_VP) /
        (1 - OneHandedPolicy.RAIL_PERCENT / 100),
    );
    return screenWidthVp > 0 ? Math.min(screenWidthVp, block) : block;
  }

  /**
   * 大屏单手时侧边数字键盘的宽度（vp），0 表示不画。`contentWidthVp` 是按键这一行可用的宽度（屏幕宽减去键盘根的左右内边距）。
   *
   * 空出来的宽度是可用宽度减去「侧栏 + 间距 + 按键」这一块（`blockWidthVp`），再减去面板与侧栏之间至少留出的一道 `RAIL_GAP_VP`，与 iPad 的算法相同。不到 `SIDE_PAD_MIN_WIDTH_VP` 时三列键太窄，不画；更宽时最多 `SIDE_PAD_MAX_WIDTH_VP`，多出来的是不接触摸的空白。手机（块宽为 0）和宽度未知时返回 0。
   */
  static sidePadWidthVp(contentWidthVp: number, largeScreen: boolean): number {
    const block: number = OneHandedPolicy.blockWidthVp(contentWidthVp, largeScreen);
    if (block === 0 || contentWidthVp <= 0) {
      return 0;
    }
    const side: number = contentWidthVp - block - OneHandedPolicy.RAIL_GAP_VP;
    if (side < OneHandedPolicy.SIDE_PAD_MIN_WIDTH_VP) {
      return 0;
    }
    return Math.min(side, OneHandedPolicy.SIDE_PAD_MAX_WIDTH_VP);
  }

  /** `touch_one_handed_side_panel` 的值；不认识的（包括以后的 `text_edit`）和缺键都按空着。 */
  static sidePanel(value: unknown): OneHandedSidePanel {
    return value === OneHandedSidePanel.NUMBER_PAD
      ? OneHandedSidePanel.NUMBER_PAD
      : OneHandedSidePanel.OFF;
  }

  /**
   * 侧边数字键盘此刻画不画，下面几条都满足才画：
   *
   * - 偏好是数字键盘；
   * - 单手模式此刻生效（`active`，2in1 不生效；鸿蒙没有浮动键盘，分离式键盘画着时 `KeyboardView.oneHandActive` 为假）；
   * - 大屏且空出来的地方放得下（`sidePadWidthVp` 大于 0）；
   * - 主键区画的是九键一族（拼音九键、九键双拼、笔画）的字母层：123 层、英文、本地模式、注音、假名九键、手写、14/18 键和 26 键都不画。
   *
   * 还有一条不在这里：符号、表情、剪贴板这些面板盖住按键时，面板和按键在同一行（`KeyboardView.handedKeys`），整行都不画，侧边数字键盘跟着不见。
   */
  static sidePadShown(
    panel: OneHandedSidePanel,
    active: boolean,
    sidePadWidthVp: number,
    nineKeyFamilyLetters: boolean,
  ): boolean {
    return (
      panel === OneHandedSidePanel.NUMBER_PAD &&
      active &&
      sidePadWidthVp > 0 &&
      nineKeyFamilyLetters
    );
  }

  /** 侧边数字键盘的前三行，与电话拨号盘和九键数字层相同，1 2 3 在上；第四行是 `.`、`0` 和 ⌫。 */
  static sidePadDigitRows(): string[][] {
    return [
      ["1", "2", "3"],
      ["4", "5", "6"],
      ["7", "8", "9"],
    ];
  }

  /** 侧边数字键的读屏名称，与数字层的数字键一样读「数字 N」。 */
  static sidePadDigitLabel(digit: string): string {
    return `数字 ${digit}`;
  }

  /** 侧边数字键盘小数点的读屏名称。 */
  static sidePadDecimalLabel(): string {
    return "小数点";
  }

  /** 本版本认得的存储值；其他值一律返回 `fallback`。 */
  static parse(
    value: string | null | undefined,
    fallback: OneHandedMode = OneHandedPolicy.DEFAULT,
  ): OneHandedMode {
    for (const mode of MODES) {
      if (value === mode) {
        return mode;
      }
    }
    return fallback;
  }

  /** 磁贴的点按和侧边栏的退出按钮：关闭时切换为贴右侧开启，贴任一侧时关闭。 */
  static toggled(mode: OneHandedMode): OneHandedMode {
    return mode === OneHandedMode.OFF ? OneHandedMode.RIGHT : OneHandedMode.OFF;
  }

  /** 磁贴的长按和侧边栏的换边按钮：左变右，其他一律变左，所以关闭时长按磁贴会以贴左侧开启该模式，与 Android 相同。 */
  static swapped(mode: OneHandedMode): OneHandedMode {
    return mode === OneHandedMode.LEFT ? OneHandedMode.RIGHT : OneHandedMode.LEFT;
  }

  /** 按键是否在侧边栏旁缩窄绘制：模式已开启且键盘绘制触摸按键。 */
  static active(mode: OneHandedMode, desktop: boolean): boolean {
    return mode !== OneHandedMode.OFF && !desktop;
  }

  /** 按键贴右侧时侧边栏在左边，否则在右边。 */
  static railOnLeft(mode: OneHandedMode): boolean {
    return mode === OneHandedMode.RIGHT;
  }

  /** 换边按钮供屏幕阅读器朗读的名称。 */
  static swapLabel(): string {
    return "单手键盘换到另一侧";
  }

  /** 退出按钮供屏幕阅读器朗读的名称。 */
  static exitLabel(): string {
    return "退出单手模式";
  }
}
