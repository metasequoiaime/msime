import { KeyboardGeometry } from "./KeyboardGeometry";

/**
 * 浮动键盘（#10225）的几何：键盘缩窄成一块可拖动的面板悬在应用上面，不占满屏幕宽度。移植自 Android 的 `FloatingKeyboardPolicy`（#5621），常数相同，dp 换成 vp。
 *
 * 面板是系统的 `FLG_FLOATING` 软键盘面板，扩展能力用 `resize` 定大小、`moveTo` 定位置（`@ohos.inputMethodEngine`，两者 API 10 起；`moveTo` 只对 `FLG_FIXED` 不可用）。宽度是屏幕宽度的 80%，夹在 240–480vp 且不超过屏幕；高度是停靠时的键盘高度加上顶部 22vp 的拖动条，不另外缩放。
 *
 * 位置按「可移动范围里的千分比」存（`MAX_FRACTION` 表示最右 / 最下），旋转或键盘变高后换算成像素时自动落回屏幕里，不会跑到屏幕外。这里只做换算，不依赖 ArkUI，供 node 下的逻辑测试直接调用。
 */
export class FloatingKeyboardPolicy {
  /** 浮动键盘占屏幕宽度的百分比。 */
  static readonly WIDTH_PERCENT: number = 80;
  /** 最大宽度（vp）：平板上不必铺到 80%，与手机上的手感接近即可。 */
  static readonly MAX_WIDTH_VP: number = 480;
  /** 最小宽度（vp）：再窄 26 键就点不准了；屏幕本身更窄时取屏幕宽度。 */
  static readonly MIN_WIDTH_VP: number = 240;
  /** 面板顶部拖动条的高度（vp）。 */
  static readonly BAR_HEIGHT_VP: number = 22;
  /** 拖动条中间横条的大小（vp）。 */
  static readonly GRIP_WIDTH_VP: number = 36;
  static readonly GRIP_HEIGHT_VP: number = 4;
  /** 拖动条右端「停靠」键的宽度和图标大小（vp）。 */
  static readonly DOCK_WIDTH_VP: number = 40;
  static readonly DOCK_ICON_VP: number = 18;
  /** 浮动时面板的圆角（vp）。 */
  static readonly CORNER_RADIUS_VP: number = 12;
  static readonly MAX_FRACTION: number = 1000;
  /** 第一次浮动时的位置：水平居中、贴着屏幕底部，和原来停靠的键盘在同一处，只是变窄。 */
  static readonly DEFAULT_X_FRACTION: number = 500;
  static readonly DEFAULT_Y_FRACTION: number = FloatingKeyboardPolicy.MAX_FRACTION;
  /** 拖动条的读屏名称，与 Android `FloatingKeyboardBar.DESCRIPTION` 相同。 */
  static readonly DESCRIPTION: string = "浮动键盘：按住拖动移动位置";
  /** 停靠键的读屏名称，与 Android `FloatingKeyboardBar.DOCK_DESCRIPTION` 相同。 */
  static readonly DOCK_DESCRIPTION: string = "停靠键盘";

  /** 浮动键盘此刻是否生效：开关打开，且键盘画触屏按键。2in1 的面板是跟随光标的候选窗（`FLAG_CANDIDATE`），不浮动。 */
  static active(enabled: boolean, desktop: boolean): boolean {
    return enabled && !desktop;
  }

  /** 屏幕宽度（vp）下浮动键盘的宽度（vp）：屏幕的 80%，夹在 240–480 之间，且不超过屏幕本身；宽度未知时取 `MIN_WIDTH_VP`。 */
  static widthVp(screenWidthVp: number): number {
    if (!Number.isFinite(screenWidthVp) || screenWidthVp <= 0) {
      return FloatingKeyboardPolicy.MIN_WIDTH_VP;
    }
    const preferred: number = Math.floor(
      (screenWidthVp * FloatingKeyboardPolicy.WIDTH_PERCENT) / 100,
    );
    const bounded: number = KeyboardGeometry.bounded(
      preferred,
      FloatingKeyboardPolicy.MIN_WIDTH_VP,
      FloatingKeyboardPolicy.MAX_WIDTH_VP,
    );
    return Math.min(bounded, screenWidthVp);
  }

  /** 浮动面板的高度（vp）：停靠时的键盘高度加上拖动条。 */
  static panelHeightVp(keyboardHeightVp: number): number {
    return keyboardHeightVp + FloatingKeyboardPolicy.BAR_HEIGHT_VP;
  }

  /** 千分比钳到 0–1000；不是有限数时取 `fallback`（存的文件被改坏时用默认位置）。 */
  static fraction(
    value: number,
    fallback: number = FloatingKeyboardPolicy.DEFAULT_X_FRACTION,
  ): number {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return fallback;
    }
    return Math.round(KeyboardGeometry.bounded(value, 0, FloatingKeyboardPolicy.MAX_FRACTION));
  }

  /** 千分比对应的偏移（px）：`free` 是可移动的像素范围（屏幕减去面板大小），不是正数时面板放不下，贴在起点（偏移 0）。 */
  static offset(fraction: number, free: number): number {
    if (!Number.isFinite(free) || free <= 0) {
      return 0;
    }
    return Math.round(
      (FloatingKeyboardPolicy.fraction(fraction) * free) / FloatingKeyboardPolicy.MAX_FRACTION,
    );
  }

  /** 偏移对应的千分比，`offset` 的反函数；范围外先钳回来，`free` 不是正数时为居中的 500。 */
  static fractionOf(offset: number, free: number): number {
    if (!Number.isFinite(free) || free <= 0 || !Number.isFinite(offset)) {
      return FloatingKeyboardPolicy.MAX_FRACTION / 2;
    }
    return FloatingKeyboardPolicy.fraction((offset * FloatingKeyboardPolicy.MAX_FRACTION) / free);
  }

  /** 拖动时的新偏移（px）：起点加位移后钳进 0…`free`；位移不是有限数时停在起点。 */
  static dragged(start: number, delta: number, free: number): number {
    const limit: number = Number.isFinite(free) ? Math.max(0, free) : 0;
    const next: number = Number.isFinite(delta) ? start + delta : start;
    return Math.max(0, Math.min(limit, next));
  }

  /**
   * 存着的千分比在这块屏幕上对应的面板左上角（px）。屏幕和面板大小都按此刻的值，所以转屏、键盘变高之后同一个千分比仍落在屏幕里。
   */
  static position(
    xFraction: number,
    yFraction: number,
    screenWidth: number,
    screenHeight: number,
    panelWidth: number,
    panelHeight: number,
  ): [number, number] {
    return [
      FloatingKeyboardPolicy.offset(xFraction, screenWidth - panelWidth),
      FloatingKeyboardPolicy.offset(yFraction, screenHeight - panelHeight),
    ];
  }
}
