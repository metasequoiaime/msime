import { KeyboardIconPaths } from '../KeyboardIconPaths';
import { KeyboardLineIcon } from './KeyboardLineIcon';

const ICON_AREA_VP: number = 30;
const ICON_VP: number = 24;
const GLYPH_BOX_VP: number = 26;
const GLYPH_BOX_RADIUS_VP: number = 5;
const GLYPH_BOX_BORDER_VP: number = 1.8;
const GLYPH_FONT_SIZE: number = 14;
/** 两个字符（方案的「双拼」「五笔」之类）用 14 号字放不进框。 */
const GLYPH_PAIR_FONT_SIZE: number = 10.5;
const GLYPH_FONT_WEIGHT: number = 700;
const LABEL_GAP_VP: number = 7;
const LABEL_FONT_SIZE: number = 12.5;
/** 设计稿中 12.5px 文字的常规行高，正是它让 30 + 7 + 15 恰好填满 52vp 的行。 */
const LABEL_LINE_HEIGHT: number = 15;
const BADGE_VP: number = 13;
const BADGE_RING_VP: number = 2;
const BADGE_CHECK_VP: number = 9;
/** 徽标相对图标区右下角的偏移：设计稿的 right -5 和 bottom -4。 */
const BADGE_OUTSET_RIGHT_VP: number = 5;
const BADGE_OUTSET_BOTTOM_VP: number = 4;
const SUBSCRIPT_FONT_SIZE: number = 8.5;
const SUBSCRIPT_FONT_WEIGHT: number = 800;
/** 角标相对图标区右下角的偏移：设计稿的 right -3 和 bottom -1。 */
const SUBSCRIPT_OUTSET_RIGHT_VP: number = 3;
const SUBSCRIPT_OUTSET_BOTTOM_VP: number = 1;
/** 带圆环的徽标在图标区中的位置：圆环使它每边各宽出 2vp，所以起点要再往外移这么多。 */
const BADGE_POSITION: Edges = {
  right: -(BADGE_OUTSET_RIGHT_VP + BADGE_RING_VP),
  bottom: -(BADGE_OUTSET_BOTTOM_VP + BADGE_RING_VP)
};
const SUBSCRIPT_POSITION: Edges = { right: -SUBSCRIPT_OUTSET_RIGHT_VP, bottom: -SUBSCRIPT_OUTSET_BOTTOM_VP };

/**
 * 分页键盘面板（功能面板和方案选择器）中的一项，移植自 Android 的 `FunctionPanelView.Tile`。
 *
 * 磁贴没有背景。30x30 的图标区里要么是字形框（`glyph` 非空：26x26 的框，1.8vp 边框、圆角 5，框内字形 14fp/700，两个字符时 10.5fp），要么是用 `iconPath` 绘制、描边 1.7 的 24vp 线性图标。标签在其下方 7vp，12.5fp 单行，放不下时以省略号结尾。
 *
 * 关闭时图标和标签用 `foreground`、字重 400。开启时它们变为 `accent`，标签字重变为 600，图标区右下角外侧放一个 13vp 的勾选徽标：带 2vp `ringColor` 圆环的强调色圆盘（`ringColor` 即键盘背景色，让徽标看起来像是从背后挖出来的），以及用 `ringColor` 描边 4 的 `checkPath`。`subscript` 非空时，是方案字形框带的小号 9 或 26，放在同一角上的 `ringColor` 底衬上。
 *
 * 按下时磁贴变暗到 .55。长按时若磁贴有 `onHold` 就执行它（「单手模式」长按时左右互换，与 Android 的磁贴相同），否则当作点按。禁用的磁贴以 .4 绘制，不响应点按，并向无障碍报告为禁用。`spokenLabel` 是朗读名称（为空时用标签），`accessibilityState` 在其后朗读，例如「已开启」「已关闭」或「不可用」。
 */
@Component
export struct PanelTile {
  @Require @Prop label: string;
  /** 画在框里的字形；为空时改画 `iconPath`。 */
  @Prop glyph: string = '';
  /** 没有字形时绘制的 24 单位线性图标。 */
  @Prop iconPath: string = '';
  @Prop on: boolean = false;
  @Prop disabled: boolean = false;
  @Require @Prop foreground: string;
  @Require @Prop accent: string;
  @Require @Prop ringColor: string;
  @Require @Prop checkPath: string;
  @Prop subscript: string = '';
  @Prop spokenLabel: string = '';
  @Prop accessibilityState: string = '';
  @Require onTap: () => void;
  /** 长按执行的操作；长按等同点按时为 null。 */
  onHold: (() => void) | null = null;

  private color(): string {
    return this.on ? this.accent : this.foreground;
  }

  private glyphFontSize(): number {
    return this.glyph.length >= 2 ? GLYPH_PAIR_FONT_SIZE : GLYPH_FONT_SIZE;
  }

  private spokenText(): string {
    return this.spokenLabel.length > 0 ? this.spokenLabel : this.label;
  }

  build() {
    Column({ space: LABEL_GAP_VP }) {
      Stack() {
        if (this.glyph.length > 0) {
          Text(this.glyph)
            .width(GLYPH_BOX_VP)
            .height(GLYPH_BOX_VP)
            .fontSize(this.glyphFontSize())
            .fontWeight(GLYPH_FONT_WEIGHT)
            .fontColor(this.color())
            .textAlign(TextAlign.Center)
            .maxLines(1)
            .border({ width: GLYPH_BOX_BORDER_VP, color: this.color(), radius: GLYPH_BOX_RADIUS_VP })
          if (this.subscript.length > 0) {
            Text(this.subscript)
              .fontSize(SUBSCRIPT_FONT_SIZE)
              .fontWeight(SUBSCRIPT_FONT_WEIGHT)
              .fontColor(this.color())
              .lineHeight(SUBSCRIPT_FONT_SIZE)
              .maxLines(1)
              .padding({ left: 1, right: 1 })
              .backgroundColor(this.ringColor)
              .position(SUBSCRIPT_POSITION)
          }
        } else if (this.iconPath.length > 0) {
          KeyboardLineIcon({ d: this.iconPath, iconSize: ICON_VP, stroke: KeyboardIconPaths.PANEL_STROKE, color: this.color() })
        }
        if (this.on) {
          // 圆环是徽标背后一个大 2vp 的圆盘，对应设计稿的 `box-shadow: 0 0 0 2px`。
          Stack() {
            Stack() {
              KeyboardLineIcon({
                d: this.checkPath,
                iconSize: BADGE_CHECK_VP,
                stroke: KeyboardIconPaths.CHECK_STROKE,
                color: this.ringColor
              })
            }
            .width(BADGE_VP)
            .height(BADGE_VP)
            .borderRadius(BADGE_VP / 2)
            .backgroundColor(this.accent)
          }
          .width(BADGE_VP + BADGE_RING_VP * 2)
          .height(BADGE_VP + BADGE_RING_VP * 2)
          .borderRadius(BADGE_VP / 2 + BADGE_RING_VP)
          .backgroundColor(this.ringColor)
          .position(BADGE_POSITION)
        }
      }
      .width(ICON_AREA_VP)
      .height(ICON_AREA_VP)

      Text(this.label)
        .fontSize(LABEL_FONT_SIZE)
        .lineHeight(LABEL_LINE_HEIGHT)
        .fontWeight(this.on ? 600 : 400)
        .fontColor(this.color())
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .enabled(!this.disabled)
    .stateStyles({
      normal: {
        .opacity(1)
      },
      pressed: {
        .opacity(0.55)
      },
      disabled: {
        .opacity(0.4)
      }
    })
    .accessibilityGroup(true)
    .accessibilityText(this.spokenText())
    .accessibilityDescription(this.accessibilityState)
    .onClick(() => this.onTap())
    .gesture(LongPressGesture().onAction(() => {
      const hold: (() => void) | null = this.onHold;
      if (hold !== null) {
        hold();
      } else {
        this.onTap();
      }
    }))
  }
}
