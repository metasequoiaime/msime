/**
 * 按键弹出预览气泡的位置，移植自 platforms/android/java/app/msime/android/keyboard/KeyboardKeyPreview.java 的静态几何：宽度为按键宽度的 138%（权重超过 1.2 的按键为 150%），高 54 vp，底边在按键顶边下方 6 vp，以按键为中心并限制在键盘内。所有值的单位都是 vp，相对于键盘左上角。
 */
export interface KeyPreviewFrame {
  label: string;
  left: number;
  top: number;
  width: number;
}

export class KeyPreviewLayout {
  static readonly HEIGHT_VP: number = 54;
  static readonly OVERLAP_VP: number = 6;
  static readonly TEXT_FP: number = 30;
  static readonly TOP_RADIUS_VP: number = 12;
  static readonly BOTTOM_RADIUS_VP: number = 8;
  /** 设计稿的 `0 6px 18px rgba(0,0,0,.22)` 阴影及其 `.5px rgba(0,0,0,.08)` 描边。 */
  static readonly SHADOW_RADIUS_VP: number = 18;
  static readonly SHADOW_OFFSET_VP: number = 6;
  static readonly SHADOW_COLOR: string = "#38000000";
  static readonly RING_WIDTH_VP: number = 0.5;
  static readonly RING_COLOR: string = "#14000000";

  /** 按键宽度的 138%，权重超过 1.2 的按键为 150%。 */
  static bubbleWidth(keyWidth: number, weight: number): number {
    return keyWidth * (weight > 1.2 ? 1.5 : 1.38);
  }

  /** 以按键为中心，再限制在 [0, parentWidth - bubbleWidth] 内。 */
  static bubbleLeft(
    keyLeft: number,
    keyWidth: number,
    bubbleWidth: number,
    parentWidth: number,
  ): number {
    const left: number = keyLeft + keyWidth / 2 - bubbleWidth / 2;
    const max: number = Math.max(0, parentWidth - bubbleWidth);
    return Math.min(Math.max(left, 0), max);
  }

  /** 气泡底边落在按键顶边下方 OVERLAP_VP 处。 */
  static bubbleTop(keyTop: number): number {
    return keyTop + KeyPreviewLayout.OVERLAP_VP - KeyPreviewLayout.HEIGHT_VP;
  }

  /** 位于 (keyLeft, keyTop)、宽 keyWidth 的按键在宽 parentWidth 的键盘中的完整框。 */
  static frame(
    label: string,
    keyLeft: number,
    keyTop: number,
    keyWidth: number,
    weight: number,
    parentWidth: number,
  ): KeyPreviewFrame {
    const width: number = KeyPreviewLayout.bubbleWidth(keyWidth, weight);
    return {
      label: label,
      left: KeyPreviewLayout.bubbleLeft(keyLeft, keyWidth, width, parentWidth),
      top: KeyPreviewLayout.bubbleTop(keyTop),
      width: width,
    };
  }
}
