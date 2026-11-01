import { KeyboardGeometry } from "./KeyboardGeometry";

/**
 * 「键盘调整」遮罩（功能面板的「键盘调整」磁贴）里四项的取值、生效条件、外框换算和各个柄的拖动约束。Android、iOS 按同一份契约实现，三端界面和行为一致，见 `.agents/notes/implemented/feature/2026-11-01-harmony-keyboard-adjust-overlay.md`。
 *
 * 四项：键盘高度（共享偏好 `touch_keyboard_height_adjustment`，-12..48vp）、底部抬高（0–160vp）、宽度（面板宽的 60–100%，另有 240vp 下限；停靠面板通常就是整屏宽）、水平位置（剩余空间里的千分比 0–1000，500 居中）。后三项只存本机的按键反馈文件（`KeyboardFeedback`），不进共享偏好、不同步。
 *
 * 所有长度都是 vp，拖动位移按屏幕坐标、向下和向右为正。这里只做换算，不依赖 ArkUI，供 node 下的逻辑测试直接调用。
 */

/** 四项的值。 */
export interface AdjustValues {
  /** 高度调整（vp），即 `touch_keyboard_height_adjustment`。 */
  readonly height: number;
  /** 底部抬高（vp）。 */
  readonly lift: number;
  /** 宽度占面板宽的百分比。 */
  readonly widthPercent: number;
  /** 水平位置：键盘两侧剩余空间里左侧所占的千分比。 */
  readonly positionX: number;
}

/** 此刻哪几项生效。不生效的项照样存着，只是不画对应的柄、不影响布局。 */
export interface AdjustAvailability {
  readonly lift: boolean;
  readonly width: boolean;
  readonly position: boolean;
}

/** 键盘外框在面板里的位置（vp）：左右各空出多少，底下抬起多少。空出和抬起的地方只画键盘底色。 */
export interface AdjustFrame {
  readonly left: number;
  readonly right: number;
  readonly lift: number;
}

/** 遮罩上画哪些柄和「移动」键。 */
export interface AdjustHandles {
  readonly top: boolean;
  readonly bottom: boolean;
  readonly left: boolean;
  readonly right: boolean;
  readonly move: boolean;
}

/** 拖左右沿或「移动」之后的宽度和位置。 */
export interface AdjustSpan {
  readonly widthPercent: number;
  readonly positionX: number;
}

/** 拖下沿或「移动」之后的高度和底部抬高。 */
export interface AdjustRise {
  readonly height: number;
  readonly lift: number;
}

export class KeyboardAdjustPolicy {
  static readonly MIN_LIFT_VP: number = 0;
  static readonly MAX_LIFT_VP: number = 160;
  static readonly DEFAULT_LIFT_VP: number = 0;
  static readonly MIN_WIDTH_PERCENT: number = 60;
  static readonly MAX_WIDTH_PERCENT: number = 100;
  static readonly DEFAULT_WIDTH_PERCENT: number = 100;
  /** 拖动收窄时键盘至少这么宽（vp），屏幕窄时它比 60% 更宽，取两者中更宽的。 */
  static readonly MIN_WIDTH_VP: number = 240;
  static readonly MAX_POSITION: number = 1000;
  static readonly DEFAULT_POSITION: number = 500;

  /** 「重置」回到的值：高度不调整、不抬高、铺满宽度、居中。 */
  static readonly DEFAULTS: AdjustValues = {
    height: KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP,
    lift: KeyboardAdjustPolicy.DEFAULT_LIFT_VP,
    widthPercent: KeyboardAdjustPolicy.DEFAULT_WIDTH_PERCENT,
    positionX: KeyboardAdjustPolicy.DEFAULT_POSITION,
  };

  /** 遮罩：约 45% 不透明的黑色，下面的键盘照常画作实时预览。 */
  static readonly MASK_COLOR: string = "#73000000";
  /** 柄、按钮图标和文字的颜色。 */
  static readonly INK_COLOR: string = "#FFFFFF";

  /** 四个柄的读屏名称，三端相同。当前值放在说明里（`heightValue` 等）。 */
  static readonly TOP_LABEL: string = "键盘高度";
  static readonly BOTTOM_LABEL: string = "键盘底部高度";
  static readonly LEFT_LABEL: string = "键盘宽度（左边）";
  static readonly RIGHT_LABEL: string = "键盘宽度（右边）";
  /** 三个按钮的文字，也是它们的读屏名称，三端相同。 */
  static readonly RESET_LABEL: string = "重置";
  static readonly MOVE_LABEL: string = "移动";
  static readonly CONFIRM_LABEL: string = "确认";
  static readonly RESET_DESCRIPTION: string = "高度、底部抬高、宽度和位置恢复默认，确认后保存";
  static readonly MOVE_DESCRIPTION: string = "按住拖动，移动整块键盘";
  static readonly CONFIRM_DESCRIPTION: string = "保存键盘调整并退出";
  static readonly TOP_DESCRIPTION: string = "上下拖动上沿，调整键盘高度";
  static readonly BOTTOM_DESCRIPTION: string = "上下拖动下沿，调整底部抬高";
  static readonly SIDE_DESCRIPTION: string = "左右拖动，调整键盘宽度";

  /** 存着的底部抬高：取整钳到 0–160；不是有限数时取 `fallback`。 */
  static lift(
    value: number | null | undefined,
    fallback: number = KeyboardAdjustPolicy.DEFAULT_LIFT_VP,
  ): number {
    return KeyboardAdjustPolicy.integer(
      value,
      KeyboardAdjustPolicy.MIN_LIFT_VP,
      KeyboardAdjustPolicy.MAX_LIFT_VP,
      fallback,
    );
  }

  /** 存着的宽度：取整钳到 60–100；不是有限数时取 `fallback`。 */
  static widthPercent(
    value: number | null | undefined,
    fallback: number = KeyboardAdjustPolicy.DEFAULT_WIDTH_PERCENT,
  ): number {
    return KeyboardAdjustPolicy.integer(
      value,
      KeyboardAdjustPolicy.MIN_WIDTH_PERCENT,
      KeyboardAdjustPolicy.MAX_WIDTH_PERCENT,
      fallback,
    );
  }

  /** 存着的水平位置：取整钳到 0–1000；不是有限数时取 `fallback`。 */
  static position(
    value: number | null | undefined,
    fallback: number = KeyboardAdjustPolicy.DEFAULT_POSITION,
  ): number {
    return KeyboardAdjustPolicy.integer(value, 0, KeyboardAdjustPolicy.MAX_POSITION, fallback);
  }

  /**
   * 此刻哪几项生效。浮动键盘、手机横屏和 2in1 上底部抬高、宽度、位置都不生效，只剩上沿改高度；单手模式或分离式键盘画着时宽度和位置不生效，底部抬高照常。
   */
  static availability(
    floating: boolean,
    phoneLandscape: boolean,
    desktop: boolean,
    oneHanded: boolean,
    split: boolean,
  ): AdjustAvailability {
    const docked: boolean = !floating && !phoneLandscape && !desktop;
    const spans: boolean = docked && !oneHanded && !split;
    return { lift: docked, width: spans, position: spans };
  }

  /** 遮罩上画哪些柄：上沿一直有；下沿跟底部抬高；左右沿跟宽度；「移动」在抬高或位置生效时有（单手、分离时只改抬高）。 */
  static handles(availability: AdjustAvailability): AdjustHandles {
    return {
      top: true,
      bottom: availability.lift,
      left: availability.width,
      right: availability.width,
      move: availability.lift || availability.position,
    };
  }

  /**
   * 离开键盘调整时存下的四项：高度一直生效，取预览值；抬高、宽度、位置只有此刻生效、并且和打开时（`original`）不同才取预览值，否则保留已存的值（`stored`）。不生效的项看不到柄，「重置」把它们预览回默认也不该改掉它们；没动过的项不写，设置页中途存下的值不被打开时的旧值盖掉。与 Android `AndroidLocalSettings.keyboardAdjustEdits` 同一条规则。
   */
  static saved(
    stored: AdjustValues,
    original: AdjustValues,
    preview: AdjustValues,
    availability: AdjustAvailability,
  ): AdjustValues {
    return {
      height: preview.height,
      lift: availability.lift && preview.lift !== original.lift ? preview.lift : stored.lift,
      widthPercent:
        availability.width && preview.widthPercent !== original.widthPercent
          ? preview.widthPercent
          : stored.widthPercent,
      positionX:
        availability.position && preview.positionX !== original.positionX
          ? preview.positionX
          : stored.positionX,
    };
  }

  /** 生效时的底部抬高（vp），不生效时为 0。面板高度（`KeyboardMetrics.totalHeightVp`）和根节点底部内边距都加上它。 */
  static liftVp(storedLift: number, availability: AdjustAvailability): number {
    return availability.lift ? KeyboardAdjustPolicy.lift(storedLift) : 0;
  }

  /** 在 `availableVp` 宽的面板里宽度最小可以到多少：60% 与 240vp 中更宽的那个，屏幕不到 240vp 时为 100%。宽度未知时为 100%。 */
  static minWidthPercent(availableVp: number): number {
    if (!Number.isFinite(availableVp) || availableVp <= 0) {
      return KeyboardAdjustPolicy.MAX_WIDTH_PERCENT;
    }
    const floor: number = Math.ceil((KeyboardAdjustPolicy.MIN_WIDTH_VP * 100) / availableVp - 1e-9);
    return Math.min(
      KeyboardAdjustPolicy.MAX_WIDTH_PERCENT,
      Math.max(KeyboardAdjustPolicy.MIN_WIDTH_PERCENT, floor),
    );
  }

  /** 画的时候用的宽度：存着的值不低于这块屏幕上的下限（转到更窄的屏幕时抬到下限，存着的值不变）。 */
  static drawnWidthPercent(storedPercent: number, availableVp: number): number {
    return Math.max(
      KeyboardAdjustPolicy.widthPercent(storedPercent),
      KeyboardAdjustPolicy.minWidthPercent(availableVp),
    );
  }

  /** 键盘外框在 `availableVp` 宽的面板里的位置。宽度不生效时铺满；位置不生效时居中。 */
  static frame(
    values: AdjustValues,
    availability: AdjustAvailability,
    availableVp: number,
  ): AdjustFrame {
    const lift: number = KeyboardAdjustPolicy.liftVp(values.lift, availability);
    if (!availability.width || !Number.isFinite(availableVp) || availableVp <= 0) {
      return { left: 0, right: 0, lift: lift };
    }
    const width: number =
      (availableVp * KeyboardAdjustPolicy.drawnWidthPercent(values.widthPercent, availableVp)) /
      100;
    const free: number = Math.max(0, availableVp - width);
    const position: number = availability.position
      ? KeyboardAdjustPolicy.position(values.positionX)
      : KeyboardAdjustPolicy.DEFAULT_POSITION;
    const left: number = (free * position) / KeyboardAdjustPolicy.MAX_POSITION;
    return { left: left, right: free - left, lift: lift };
  }

  /** 拖上沿：往上拖（`dy` 为负）变高，1vp 对 1vp，钳在共享偏好的 -12..48 里。下沿不动。 */
  static dragTop(baseHeight: number, dy: number): number {
    const offset: number = Number.isFinite(dy) ? dy : 0;
    return KeyboardGeometry.heightAdjustment(
      Math.round(KeyboardGeometry.heightAdjustment(baseHeight) - offset),
    );
  }

  /**
   * 拖下沿，从按下时的高度和抬高出发，手指累计往上移 u：
   *
   * - 往上（u ≥ 0）：抬高跟着手指走，到 160 停；高度反向让出同样多但不低于下限 −12。所以先拿高度换抬高、上沿不动；高度到下限后整块键盘继续往上移，直到抬高到顶。鸿蒙的高度下限只有 −12，没有这一段的话下沿柄最多只能把键盘抬起 12 + 当前高度那么多。
   * - 往下（u < 0）：抬高减、高度加同样多，上沿不动；抬高到 0 或高度到上限 48 时两项一起停。
   */
  static dragBottom(baseHeight: number, baseLift: number, dy: number): AdjustRise {
    const height: number = KeyboardGeometry.heightAdjustment(baseHeight);
    const lift: number = KeyboardAdjustPolicy.lift(baseLift);
    const up: number = Number.isFinite(dy) ? Math.round(-dy) : 0;
    if (up >= 0) {
      const raised: number = Math.min(lift + up, KeyboardAdjustPolicy.MAX_LIFT_VP);
      const yielded: number = Math.min(
        raised - lift,
        height - KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP,
      );
      return { height: height - yielded, lift: raised };
    }
    const lowest: number = Math.max(
      KeyboardAdjustPolicy.MIN_LIFT_VP - lift,
      height - KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP,
    );
    const rise: number = Math.max(lowest, up);
    return { height: height - rise, lift: lift + rise };
  }

  /** 拖左沿：往左拖（`dx` 为负）变宽，右沿在屏幕上不动。宽度按整数百分比走，左沿到屏幕边或宽度到下限就停。 */
  static dragLeft(
    availableVp: number,
    baseWidthPercent: number,
    basePosition: number,
    dx: number,
  ): AdjustSpan {
    if (!Number.isFinite(availableVp) || availableVp <= 0) {
      return KeyboardAdjustPolicy.span(baseWidthPercent, basePosition);
    }
    const base: AdjustFrame = KeyboardAdjustPolicy.spanFrame(
      availableVp,
      baseWidthPercent,
      basePosition,
    );
    const right: number = availableVp - base.right;
    const startWidth: number = right - base.left;
    const wanted: number = startWidth - (Number.isFinite(dx) ? dx : 0);
    const percent: number = KeyboardAdjustPolicy.boundedPercent(wanted, availableVp, right);
    const width: number = (availableVp * percent) / 100;
    return {
      widthPercent: percent,
      positionX: KeyboardAdjustPolicy.positionOf(right - width, width, availableVp, basePosition),
    };
  }

  /** 拖右沿：往右拖变宽，左沿在屏幕上不动。 */
  static dragRight(
    availableVp: number,
    baseWidthPercent: number,
    basePosition: number,
    dx: number,
  ): AdjustSpan {
    if (!Number.isFinite(availableVp) || availableVp <= 0) {
      return KeyboardAdjustPolicy.span(baseWidthPercent, basePosition);
    }
    const base: AdjustFrame = KeyboardAdjustPolicy.spanFrame(
      availableVp,
      baseWidthPercent,
      basePosition,
    );
    const startWidth: number = availableVp - base.left - base.right;
    const wanted: number = startWidth + (Number.isFinite(dx) ? dx : 0);
    const percent: number = KeyboardAdjustPolicy.boundedPercent(
      wanted,
      availableVp,
      availableVp - base.left,
    );
    const width: number = (availableVp * percent) / 100;
    return {
      widthPercent: percent,
      positionX: KeyboardAdjustPolicy.positionOf(base.left, width, availableVp, basePosition),
    };
  }

  /** 「移动」拖抬高的一半：往上拖抬高，钳在 0–160，大小不变；抬高不生效时不变。 */
  static movedLift(baseLift: number, dy: number, availability: AdjustAvailability): number {
    const lift: number = KeyboardAdjustPolicy.lift(baseLift);
    if (!availability.lift || !Number.isFinite(dy)) {
      return lift;
    }
    return KeyboardAdjustPolicy.lift(Math.round(lift - dy));
  }

  /** 「移动」拖位置的一半：左沿跟着手指走，钳在屏幕里，宽度不变；位置不生效或键盘铺满时不变。 */
  static movedPosition(
    availableVp: number,
    widthPercent: number,
    basePosition: number,
    dx: number,
    availability: AdjustAvailability,
  ): number {
    const position: number = KeyboardAdjustPolicy.position(basePosition);
    if (
      !availability.position ||
      !Number.isFinite(availableVp) ||
      availableVp <= 0 ||
      !Number.isFinite(dx)
    ) {
      return position;
    }
    const base: AdjustFrame = KeyboardAdjustPolicy.spanFrame(availableVp, widthPercent, position);
    const free: number = base.left + base.right;
    if (free <= 0) {
      return position;
    }
    const left: number = Math.max(0, Math.min(free, base.left + dx));
    return KeyboardAdjustPolicy.position((left * KeyboardAdjustPolicy.MAX_POSITION) / free);
  }

  /**
   * 遮罩中间一排每个按钮的宽（vp）：`frameWidthVp` 宽的外框里，左右各让出一个 `handleVp` 宽的柄（不画柄时也占着同样宽的位置），按钮之间隔 `gapVp`，剩下的平分给 `buttons` 个按钮，最宽 `widestVp`。键盘收窄到 240vp 时三个 72vp 的按钮加间距要 248vp，柄之间只剩 176vp，不缩就压到左右柄上。
   */
  static buttonWidthVp(
    frameWidthVp: number,
    buttons: number,
    handleVp: number,
    gapVp: number,
    widestVp: number,
  ): number {
    if (!Number.isFinite(frameWidthVp) || frameWidthVp <= 0 || buttons <= 0) {
      return widestVp;
    }
    const room: number = frameWidthVp - 2 * handleVp - (buttons - 1) * gapVp;
    return Math.max(0, Math.min(widestVp, room / buttons));
  }

  /** 上沿柄读屏说明里的当前值：键盘高度的百分比（`KeyboardGeometry.heightPercent`）。 */
  static heightValue(height: number): string {
    return `${KeyboardGeometry.heightPercent(KeyboardGeometry.heightAdjustment(height))}%`;
  }

  /** 下沿柄读屏说明里的当前值：底部抬高。 */
  static liftValue(lift: number): string {
    return `${KeyboardAdjustPolicy.lift(lift)}vp`;
  }

  /** 左右沿柄读屏说明里的当前值：宽度百分比。 */
  static widthValue(widthPercent: number): string {
    return `${KeyboardAdjustPolicy.widthPercent(widthPercent)}%`;
  }

  /** 宽度和位置都生效时的外框，抬高不参与。 */
  private static spanFrame(
    availableVp: number,
    widthPercent: number,
    position: number,
  ): AdjustFrame {
    return KeyboardAdjustPolicy.frame(
      { height: 0, lift: 0, widthPercent: widthPercent, positionX: position },
      { lift: false, width: true, position: true },
      availableVp,
    );
  }

  /** 想要的宽度（vp）换成整数百分比，钳在下限和 `limitVp`（对边到屏幕边的距离）之间。 */
  private static boundedPercent(wantedVp: number, availableVp: number, limitVp: number): number {
    const lowest: number = KeyboardAdjustPolicy.minWidthPercent(availableVp);
    const highest: number = Math.max(
      lowest,
      Math.min(
        KeyboardAdjustPolicy.MAX_WIDTH_PERCENT,
        Math.floor((limitVp * 100) / availableVp + 1e-9),
      ),
    );
    return Math.max(lowest, Math.min(highest, Math.round((wantedVp * 100) / availableVp)));
  }

  /** 左沿在 `leftVp`、宽 `widthVp` 时的千分比；键盘铺满、没有剩余空间时保留 `fallback`。 */
  private static positionOf(
    leftVp: number,
    widthVp: number,
    availableVp: number,
    fallback: number,
  ): number {
    const free: number = availableVp - widthVp;
    if (free <= 1e-9) {
      return KeyboardAdjustPolicy.position(fallback);
    }
    return KeyboardAdjustPolicy.position((leftVp * KeyboardAdjustPolicy.MAX_POSITION) / free);
  }

  private static span(widthPercent: number, position: number): AdjustSpan {
    return {
      widthPercent: KeyboardAdjustPolicy.widthPercent(widthPercent),
      positionX: KeyboardAdjustPolicy.position(position),
    };
  }

  private static integer(
    value: number | null | undefined,
    minimum: number,
    maximum: number,
    fallback: number,
  ): number {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return fallback;
    }
    return KeyboardGeometry.bounded(Math.round(value), minimum, maximum);
  }
}
