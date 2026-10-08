/**
 * 单手模式：按键贴向哪一侧，功能面板磁贴和侧边栏的两个按钮把它切换成什么，以及开启时按键区域如何划分。
 *
 * 移植自本键盘所遵循的先例 Android：存储值及其默认值是 `AndroidLocalSettings` 中的 `platform.android.one_handed`（`off`、`left` 或 `right`，默认 `off`），磁贴的点按和长按是 `MSIMEInputService.toggleOneHanded`，侧边栏是 `ImeFrame` 加 `OneHandGutterView`。侧边栏位于按键让开的那一侧，占宽度的 15%；工具栏和候选条保持全宽。侧边栏的尺寸取自设计稿（全平台 UI.dc.html 的单手按键区）：40vp 按钮间隔 14vp，20vp 图标描边 1.9，侧边栏与按键间距 6vp，按下缩放 .9。
 *
 * 只有按键会变窄。替换按键的面板（功能面板、表情、剪贴板等）全宽绘制，就像 Android 的侧边栏在没有按键行显示时隐藏一样。2in1 在条带上不绘制自己的触摸按键，所以该模式在那里从不生效。
 */
export enum OneHandedMode {
  OFF = "off",
  LEFT = "left",
  RIGHT = "right",
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
