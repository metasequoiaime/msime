/**
 * 大屏的分离式键盘（#10224）：什么时候分、哪些键面分、每一行从哪里断开、中间空出多宽，以及它与单手模式的互斥。规则逐条移植自 Android 的 `SplitKeyboardPolicy`，取值范围和默认值也相同；纯计算，不依赖 ArkUI，在 node 下测试。
 *
 * 分离需要共享偏好 `touch_keyboard_split` 打开、屏幕最短边不小于启用宽度（`touch_keyboard_split_threshold`，480–1000vp，默认 600，与 `KeyboardFormFactorPolicy.largeScreen` 的界线相同），并且横屏或 `touch_keyboard_split_portrait` 打开；2in1 不画触摸按键，从不分离。正在画的键面也必须是 26 键一族（全拼、双拼、五笔、英文、日文罗马字、粤拼、越南语、藏文）或韩文，以及它们的 123 / #+= 层；九键、14/18 键、笔画、注音、手写和假名网格不分离，它们借来的数字九宫格也不分离。
 *
 * 每一行都是「不分离时的那一行，在中间某个键的边界上断开，中间插入一段不接触摸的空隙」。空隙至少占整行的 `gapPercent`（默认 25%），宽屏上两侧原始键区合计不超过 720vp，多出的宽度留在中间（`gapFraction`）。断点取使左段份额最接近整行一半的键边界，一样接近时多出来的键归左边（`cutIndex`）。字母层第二、三行在右半边内侧重复左半边最内侧的字母（asdfg | ghjkl、⇧zxcv | vbnm⌫，`duplicatesInnerKey`），重复的键从空隙里借一个键宽（`gapWeight`），所以三行键宽不变。底行的空格拆成两个，左边那个的份额让空隙落在整行中间（`leftSpaceWeight`）。
 */

/** 分离式键盘的四项共享偏好。 */
export interface SplitKeyboardSettings {
  /** `touch_keyboard_split`。 */
  readonly enabled: boolean;
  /** `touch_keyboard_split_portrait`。 */
  readonly portrait: boolean;
  /** `touch_keyboard_split_threshold`，vp。 */
  readonly thresholdVp: number;
  /** `touch_keyboard_split_gap`，百分比。 */
  readonly gapPercent: number;
}

/** 一行断开后的样子：左段有几个子视图、要不要在右段开头重复左段最内侧的键、空隙的份额。 */
export interface SplitRow {
  /** 留在左半边的子视图个数，空隙插在这个下标上。 */
  readonly cut: number;
  /** 右半边内侧是否重复左半边最内侧的键（字母层第二、三行的 G、V）。 */
  readonly duplicate: boolean;
  /** 空隙的 `layoutWeight`，已经扣掉重复键借走的份额。 */
  readonly gapShare: number;
}

/** 拆开后的底行空格：两半的份额和中间空隙的份额。 */
export interface SplitSpace {
  readonly left: number;
  readonly right: number;
  readonly gap: number;
}

const EPSILON: number = 1e-3;

function nonNegative(value: number): number {
  return Number.isFinite(value) && value > 0 ? value : 0;
}

function bounded(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, value));
}

export class SplitKeyboardPolicy {
  /** 中间空隙占整行宽度的最小比例。 */
  static readonly GAP_FRACTION: number = 0.25;
  static readonly MIN_THRESHOLD_VP: number = 480;
  static readonly MAX_THRESHOLD_VP: number = 1000;
  static readonly DEFAULT_THRESHOLD_VP: number = 600;
  static readonly THRESHOLD_STEP_VP: number = 20;
  static readonly MIN_GAP_PERCENT: number = 10;
  static readonly MAX_GAP_PERCENT: number = 40;
  static readonly DEFAULT_GAP_PERCENT: number = 25;
  static readonly GAP_PERCENT_STEP: number = 5;
  /** 两侧原始键区的合计上限，与 Android 的 `MAX_KEY_AREA_WIDTH_DP`（大屏键盘外框的 720dp）相同；重复的 G、V 仍从空隙借一个键宽。 */
  static readonly MAX_KEY_AREA_WIDTH_VP: number = 720;
  /** 拆开的空格每一半至少保留原宽度的这个比例，避免某一侧只剩一条窄缝。 */
  static readonly MIN_SPACE_SHARE: number = 0.15;

  static readonly DEFAULTS: SplitKeyboardSettings = {
    enabled: false,
    portrait: false,
    thresholdVp: SplitKeyboardPolicy.DEFAULT_THRESHOLD_VP,
    gapPercent: SplitKeyboardPolicy.DEFAULT_GAP_PERCENT,
  };

  /** 从共享偏好读四项；类型不对、超出范围或不在步进上的值按默认值，与 client-core 的校验和 Android 本机设置的读法相同。 */
  static settings(
    enabled: unknown,
    portrait: unknown,
    thresholdVp: unknown,
    gapPercent: unknown,
  ): SplitKeyboardSettings {
    return {
      enabled: enabled === true,
      portrait: portrait === true,
      thresholdVp: SplitKeyboardPolicy.stepped(
        thresholdVp,
        SplitKeyboardPolicy.MIN_THRESHOLD_VP,
        SplitKeyboardPolicy.MAX_THRESHOLD_VP,
        SplitKeyboardPolicy.THRESHOLD_STEP_VP,
        SplitKeyboardPolicy.DEFAULT_THRESHOLD_VP,
      ),
      gapPercent: SplitKeyboardPolicy.stepped(
        gapPercent,
        SplitKeyboardPolicy.MIN_GAP_PERCENT,
        SplitKeyboardPolicy.MAX_GAP_PERCENT,
        SplitKeyboardPolicy.GAP_PERCENT_STEP,
        SplitKeyboardPolicy.DEFAULT_GAP_PERCENT,
      ),
    };
  }

  private static stepped(
    value: unknown,
    min: number,
    max: number,
    step: number,
    fallback: number,
  ): number {
    if (
      typeof value !== "number" ||
      !Number.isInteger(value) ||
      value < min ||
      value > max ||
      (value - min) % step !== 0
    ) {
      return fallback;
    }
    return value;
  }

  /**
   * 分离式键盘的前提是否满足（与当前键面无关）：开关打开、不是 2in1、最短边不小于启用宽度，并且横屏或竖屏选项打开。
   *
   * @param widthVp 屏幕此刻的宽（转屏后与高互换）
   * @param heightVp 屏幕此刻的高
   */
  static active(
    settings: SplitKeyboardSettings,
    widthVp: number,
    heightVp: number,
    desktop: boolean,
  ): boolean {
    if (!settings.enabled || desktop || !(widthVp > 0) || !(heightVp > 0)) {
      return false;
    }
    const threshold: number = bounded(
      settings.thresholdVp,
      SplitKeyboardPolicy.MIN_THRESHOLD_VP,
      SplitKeyboardPolicy.MAX_THRESHOLD_VP,
    );
    const landscape: boolean = widthVp > heightVp;
    return (landscape || settings.portrait) && Math.min(widthVp, heightVp) >= threshold;
  }

  /** 当前是否真的画成分离式：前提满足且正在画的键面可以分离。单手模式和每一行的拆分都只看这一个结果。 */
  static drawn(
    settings: SplitKeyboardSettings,
    widthVp: number,
    heightVp: number,
    desktop: boolean,
    splitsFace: boolean,
  ): boolean {
    return splitsFace && SplitKeyboardPolicy.active(settings, widthVp, heightVp, desktop);
  }

  /** 当前这行宽度下空隙占整行的比例：用户选的比例，且保证两侧原始键区合计不超过 720vp。宽度未知时只用用户选的比例。 */
  static gapFraction(availableWidthVp: number, gapPercent: number): number {
    const configured: number =
      bounded(
        gapPercent,
        SplitKeyboardPolicy.MIN_GAP_PERCENT,
        SplitKeyboardPolicy.MAX_GAP_PERCENT,
      ) / 100;
    if (!(availableWidthVp > 0)) {
      return configured;
    }
    return Math.max(configured, 1 - SplitKeyboardPolicy.MAX_KEY_AREA_WIDTH_VP / availableWidthVp);
  }

  /** 份额合计为 `rowWeight` 的一行需要多大份额的空隙，才能让空隙占插入后整行的 `gapFraction`；空隙里还放着一个份额为 `duplicateWeight` 的重复键时从中扣掉它，整行总份额不变，键宽也就和不重复的行一样。 */
  static gapWeight(
    rowWeight: number,
    duplicateWeight: number,
    availableWidthVp: number,
    gapPercent: number,
  ): number {
    const gap: number = SplitKeyboardPolicy.gapFraction(availableWidthVp, gapPercent);
    return nonNegative((nonNegative(rowWeight) * gap) / (1 - gap) - nonNegative(duplicateWeight));
  }

  /** 一行在哪里断开：返回留在左半边的子视图个数。取使左段份额最接近整行一半的键边界，一样接近时取左段更多的那个。 */
  static cutIndex(weights: readonly number[]): number {
    let total: number = 0;
    for (const weight of weights) {
      total += nonNegative(weight);
    }
    const half: number = total / 2;
    let best: number = 0;
    let bestDistance: number = Number.MAX_VALUE;
    let left: number = 0;
    for (let index: number = 0; index <= weights.length; index++) {
      if (index > 0) {
        left += nonNegative(weights[index - 1]);
      }
      const distance: number = Math.abs(left - half);
      if (
        distance < bestDistance - EPSILON ||
        (Math.abs(distance - bestDistance) <= EPSILON && left > half)
      ) {
        best = index;
        bestDistance = distance;
      }
    }
    return best;
  }

  /** 字母层的第 `rowIndex` 行（0 起，共 `rowCount` 行字母）要不要在右半边内侧重复左半边最内侧的键：只有三行字母的第二、三行，并且重复之后两段份额正好相等。 */
  static duplicatesInnerKey(
    rowIndex: number,
    rowCount: number,
    weights: readonly number[],
  ): boolean {
    if (rowCount !== 3 || (rowIndex !== 1 && rowIndex !== 2)) {
      return false;
    }
    const cut: number = SplitKeyboardPolicy.cutIndex(weights);
    if (cut <= 0) {
      return false;
    }
    let left: number = 0;
    let right: number = 0;
    for (let index: number = 0; index < weights.length; index++) {
      if (index < cut) {
        left += nonNegative(weights[index]);
      } else {
        right += nonNegative(weights[index]);
      }
    }
    const inner: number = nonNegative(weights[cut - 1]);
    return inner > 0 && Math.abs(left - (right + inner)) <= EPSILON;
  }

  /**
   * 一行怎么断开。`letterRow` 是字母层这一行的下标（0 起），数字行、符号层各行传 -1：它们只从中间断开，不重复键。
   *
   * @param weights 这一行各子视图（含缩进占位）按最终可见状态计的份额
   */
  static row(
    weights: readonly number[],
    letterRow: number,
    letterRowCount: number,
    availableWidthVp: number,
    gapPercent: number,
  ): SplitRow {
    const cut: number = SplitKeyboardPolicy.cutIndex(weights);
    const duplicate: boolean =
      letterRow >= 0 && SplitKeyboardPolicy.duplicatesInnerKey(letterRow, letterRowCount, weights);
    let total: number = 0;
    for (const weight of weights) {
      total += nonNegative(weight);
    }
    const inner: number = duplicate ? nonNegative(weights[cut - 1]) : 0;
    return {
      cut: cut,
      duplicate: duplicate,
      gapShare: SplitKeyboardPolicy.gapWeight(total, inner, availableWidthVp, gapPercent),
    };
  }

  /** 底行的空格拆成两半时左边那一半的份额：让空隙正好落在整行中间，两半各自至少保留 `MIN_SPACE_SHARE` 的原宽度。 */
  static leftSpaceWeight(weights: readonly number[], spaceIndex: number): number {
    if (spaceIndex < 0 || spaceIndex >= weights.length) {
      return 0;
    }
    let total: number = 0;
    let before: number = 0;
    for (let index: number = 0; index < weights.length; index++) {
      const weight: number = nonNegative(weights[index]);
      total += weight;
      if (index < spaceIndex) {
        before += weight;
      }
    }
    const space: number = nonNegative(weights[spaceIndex]);
    return bounded(
      total / 2 - before,
      space * SplitKeyboardPolicy.MIN_SPACE_SHARE,
      space * (1 - SplitKeyboardPolicy.MIN_SPACE_SHARE),
    );
  }

  /** 底行拆开后的两个空格和空隙的份额。`weights` 是底行各键的份额，空格按拆开前的整份计。 */
  static space(
    weights: readonly number[],
    spaceIndex: number,
    availableWidthVp: number,
    gapPercent: number,
  ): SplitSpace {
    const left: number = SplitKeyboardPolicy.leftSpaceWeight(weights, spaceIndex);
    const space: number =
      spaceIndex >= 0 && spaceIndex < weights.length ? nonNegative(weights[spaceIndex]) : 0;
    let total: number = 0;
    for (const weight of weights) {
      total += nonNegative(weight);
    }
    return {
      left: left,
      right: nonNegative(space - left),
      gap: SplitKeyboardPolicy.gapWeight(total, 0, availableWidthVp, gapPercent),
    };
  }
}
