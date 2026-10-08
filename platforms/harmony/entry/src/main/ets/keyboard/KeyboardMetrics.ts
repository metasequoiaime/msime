import { KeyboardGeometry } from "./KeyboardGeometry";

/**
 * 键盘的尺寸。触屏框架遵循设计稿的鸿蒙键盘（`全平台 UI.dc.html`，`kbPad: '8px 4px 6px'`、`kbGap: '10px'`、50px 的工具栏或候选栏、44px 的按键间隔 6px）；按键尺寸最初取自 iOS 扩展 `platforms/ios/KeyboardExtension/Sources/KeyboardViewController.swift`，让两个键盘的布局一致。
 *
 * 它们放在这里而不是视图里，因为扩展 ability 要在视图存在之前确定面板尺寸，而与内容不一致的面板要么裁掉底行，要么在其下留出一条空白。
 */
export class KeyboardMetrics {
  /** 2in1（桌面）框架。触屏键盘使用下面的 `TOUCH_*` 内边距。 */
  static readonly ROOT_HORIZONTAL_PADDING_VP: number = 5;
  static readonly ROOT_VERTICAL_PADDING_VP: number = 7;
  /** 触屏键盘根节点的内边距：条带上方 8，两侧 4，底行下方 6。 */
  static readonly TOUCH_ROOT_TOP_PADDING_VP: number = 8;
  static readonly TOUCH_ROOT_HORIZONTAL_PADDING_VP: number = 4;
  static readonly TOUCH_ROOT_BOTTOM_PADDING_VP: number = 6;
  /** 触屏时按键上方的条带：空闲时是工具栏，输入中是候选栏，高度固定相同，切换时不会挪动任何按键。 */
  static readonly STRIP_HEIGHT_VP: number = 50;
  /** 触屏时条带与第一行按键之间的固定间距。 */
  static readonly STRIP_GAP_VP: number = 10;
  /** 按键行之间的默认间距，可由 `touch_row_spacing_tenths` 偏好覆盖。设计稿画的是 10；取 7 是为了与其他平台的默认值一致。 */
  static readonly ROW_SPACING_VP: number = 7;
  static readonly KEY_SPACING_VP: number = 6;
  static readonly KEY_CORNER_VP: number = 8;
  static readonly KEY_FONT_SIZE: number = 20;
  static readonly SMALL_KEY_FONT_SIZE: number = 15;
  static readonly LANGUAGE_FONT_SIZE: number = 13;
  static readonly MODIFIER_WIDTH_VP: number = 44;
  static readonly RETURN_WIDTH_VP: number = 59;
  static readonly LANGUAGE_WIDTH_VP: number = 34;
  static readonly LAYOUT_TOGGLE_WIDTH_VP: number = 48;
  static readonly ROW_HEIGHT_VP: number = 44;
  static readonly KEY_ROWS: number = 4;

  static readonly COMPOSITION_ROW_HEIGHT_VP: number = 28;
  static readonly CANDIDATE_ROW_HEIGHT_VP: number = 40;
  /** One caption-sized line reserved before an asynchronous candidate gloss arrives. */
  static readonly CANDIDATE_GLOSS_LINE_HEIGHT_VP: number = 14;
  static readonly CANDIDATE_FONT_SIZE: number = 20;
  static readonly CANDIDATE_PREEDIT_FONT_SIZE: number = 15;
  static readonly CANDIDATE_PADDING_VP: number = 12;
  static readonly STRIP_CORNER_VP: number = 12;
  /** The 2in1 candidate card and each candidate's fill inside it, the HarmonyOS PC window's 16 and 10. */
  static readonly CANDIDATE_CARD_CORNER_VP: number = 16;
  static readonly CANDIDATE_ITEM_CORNER_VP: number = 10;
  /** 2in1 候选卡片各边的内边距，即设计稿 hm2 的 `padding: 8`。 */
  static readonly CANDIDATE_CARD_PADDING_VP: number = 8;

  /**
   * Everything the view stacks vertically, including the gaps between the pieces.
   *
   * 行间距和高度调整来自用户设置，由 `KeyboardGeometry` 限定范围。它们作为参数传入而不是在这里读取，让这里保持为纯计算，ability 和视图都能算并得出一致结果。
   *
   * 触屏（`compact` 为 false）时框架固定：顶部内边距、50vp 条带、10vp 间距、按键行及其间的行间距、底部内边距。条带在自身高度内容纳候选及其释义，所以字号和释义行不会改变面板。`compact` 是 2in1 键盘和表情界面，它们保留一直以来的叠放组合行、候选行和释义预留。
   */
  static totalHeightVp(
    rowSpacingTenths: number = KeyboardMetrics.ROW_SPACING_VP * 10,
    heightAdjustmentVp: number = 0,
    glossRows: number = 0,
    candidateFontSize: number = KeyboardMetrics.CANDIDATE_FONT_SIZE,
    preeditFontSize: number = KeyboardMetrics.CANDIDATE_PREEDIT_FONT_SIZE,
    compact: boolean = false,
  ): number {
    const keys: number =
      KeyboardMetrics.ROW_HEIGHT_VP * KeyboardMetrics.KEY_ROWS + heightAdjustmentVp;
    if (!compact) {
      return (
        KeyboardMetrics.TOUCH_ROOT_TOP_PADDING_VP +
        KeyboardMetrics.STRIP_HEIGHT_VP +
        KeyboardMetrics.STRIP_GAP_VP +
        keys +
        KeyboardMetrics.touchRowGapsVp(rowSpacingTenths) +
        KeyboardMetrics.TOUCH_ROOT_BOTTOM_PADDING_VP
      );
    }
    // 条带就是 2in1 候选卡片，卡片在各边内侧留出 `CANDIDATE_CARD_PADDING_VP`，与 `candidateHeightVp` 的计算一致。
    const strip: number =
      KeyboardMetrics.compositionRowHeightVp(preeditFontSize) +
      KeyboardMetrics.candidateRowHeightVp(candidateFontSize, compact) +
      KeyboardMetrics.glossHeightVp(glossRows, candidateFontSize) +
      KeyboardMetrics.CANDIDATE_CARD_PADDING_VP * 2;
    // One gap between the strip and the first key row, and one between each pair of key rows.
    const gaps: number = (rowSpacingTenths / 10) * KeyboardMetrics.KEY_ROWS;
    return strip + keys + gaps + KeyboardMetrics.ROOT_VERTICAL_PADDING_VP * 2;
  }

  /** 触屏键盘按键行之间的间隙：比行数少一，因为条带有自己的固定间距。 */
  static touchRowGapsVp(rowSpacingTenths: number): number {
    return (rowSpacingTenths / 10) * (KeyboardMetrics.KEY_ROWS - 1);
  }

  /**
   * How wide a candidate window is, where the panel is not the width of the screen.
   *
   * 第一个 Engine 视图到达前的后备宽度。候选可用后，宿主会用有界的 `CandidateWidthPolicy` 估算值替换它。
   */
  static readonly CANDIDATE_WINDOW_WIDTH_VP: number = 420;
  /** Clear of the caret's own line, so the window sits under the text rather than on it. */
  static readonly CANDIDATE_WINDOW_GAP_VP: number = 4;

  /**
   * The strip on its own, which is the whole panel where the machine has its own keys.
   *
   * 没有按键行，因此也没有行间距：剩下的是组合行、候选行、卡片自身的内边距和框住卡片的内边距。
   */
  static candidateHeightVp(
    layout: string = "horizontal",
    candidateCount: number = 0,
    showPreedit: boolean = true,
    decorationTopVp: number = 0,
    glossRows: number = 0,
    candidateFontSize: number = KeyboardMetrics.CANDIDATE_FONT_SIZE,
    preeditFontSize: number = KeyboardMetrics.CANDIDATE_PREEDIT_FONT_SIZE,
  ): number {
    const rows: number =
      layout === "vertical" ? KeyboardMetrics.visibleCandidateRows(candidateCount) : 1;
    const decoration: number =
      Number.isFinite(decorationTopVp) && decorationTopVp > 0 ? Math.min(512, decorationTopVp) : 0;
    return (
      decoration +
      (showPreedit ? KeyboardMetrics.compositionRowHeightVp(preeditFontSize) : 0) +
      KeyboardMetrics.candidateRowHeightVp(candidateFontSize, true) * rows +
      (layout === "vertical" ? 0 : KeyboardMetrics.glossHeightVp(glossRows, candidateFontSize)) +
      KeyboardMetrics.CANDIDATE_CARD_PADDING_VP * 2 +
      KeyboardMetrics.ROOT_VERTICAL_PADDING_VP * 2
    );
  }

  static visibleCandidateRows(candidateCount: number): number {
    return KeyboardGeometry.bounded(candidateCount, 1, 9);
  }

  /**
   * One candidate row at the configured font size. The Windows candidate window sizes its rows from the font (`itemHeight = fontSize * 1.35 + 2` plus a 2 DIP gap, `candidate_presenter.cpp`), so a small font gives a compact window and a large one never clips. A compact row is that and nothing more, which is what a candidate window on a machine with its own keys wants; a touch strip keeps CANDIDATE_ROW_HEIGHT_VP as a floor, because it is also a row of buttons for a finger.
   */
  static candidateRowHeightVp(fontSize: number, compact: boolean): number {
    const font: number = Number.isFinite(fontSize)
      ? Math.max(1, fontSize)
      : KeyboardMetrics.CANDIDATE_FONT_SIZE;
    const fitted: number = Math.ceil(font * 1.35 + 2) + 2;
    return compact ? fitted : Math.max(KeyboardMetrics.CANDIDATE_ROW_HEIGHT_VP, fitted);
  }

  /** The composition line, tall enough for its own font: the source measures the preedit at `candidate_preedit_font_size`, and a fixed line let a large preedit spill over the candidates. */
  static compositionRowHeightVp(preeditFontSize: number): number {
    const font: number = Number.isFinite(preeditFontSize)
      ? Math.max(1, preeditFontSize)
      : KeyboardMetrics.CANDIDATE_PREEDIT_FONT_SIZE;
    return Math.max(KeyboardMetrics.COMPOSITION_ROW_HEIGHT_VP, Math.ceil(font * 1.35));
  }

  static glossHeightVp(
    rows: number,
    candidateFontSize: number = KeyboardMetrics.CANDIDATE_FONT_SIZE,
  ): number {
    if (!Number.isFinite(rows)) return 0;
    const font: number = Number.isFinite(candidateFontSize) ? Math.max(1, candidateFontSize) : 1;
    const line: number = Math.max(
      KeyboardMetrics.CANDIDATE_GLOSS_LINE_HEIGHT_VP,
      Math.ceil(font * 0.78),
    );
    return Math.max(0, Math.floor(rows)) * line;
  }
}
