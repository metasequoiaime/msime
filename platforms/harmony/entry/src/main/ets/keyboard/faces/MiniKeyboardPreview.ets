/** 设计稿 MiniKb 的虚拟画布；缩略图里的一切都按这个单位排版，再按缩略图的宽度缩放。 */
const MINI_WIDTH: number = 390;
const MINI_HEIGHT: number = 292;
const MINI_PADDING: number = 3;
const TOOLBAR_HEIGHT: number = 50;
const TOOLBAR_GAP: number = 8;
const TOOLBAR_COLUMNS: number = 7;
const BRAND_SIDE: number = 26;
const BRAND_RADIUS: number = 8;
const ROW_HEIGHT: number = 43;
const ROW_GAP: number = 11;
const KEY_GAP: number = 6;
const KEY_RADIUS: number = 5;
/** 第二排字母键两侧各向内缩进一行宽度的 5%，和真实键盘一致。 */
const MIDDLE_ROW_INSET: number = 0.05;
const THUMB_RADIUS_VP: number = 9;
/** 在缩略图上方和两侧给选中环留出的空间，让环永远碰不到格子边缘，否则网格或 swiper 会把它裁掉。缩略图下方由到名称的间距容纳它。 */
const RING_ROOM_VP: number = 2;
const NAME_GAP_VP: number = 6;
const NAME_FONT_SIZE: number = 12;

/** 迷你键的填充方式。 */
const FILL_KEY: number = 0;
const FILL_FUNCTION: number = 1;
const FILL_ACCENT: number = 2;

interface MiniKey {
  weight: number;
  fill: number;
}

/**
 * 设计稿 MiniKb 以及 Android `KeyboardSkinPreview.MINI_ROWS` / `MINI_WEIGHTS` 的四排键：十个字母；缩进的九个字母；⇧、七个字母、⌫；然后是 123、中、，、空格键、。和回车键，回车键就是强调色的那一格。
 */
const MINI_ROWS: MiniKey[][] = [
  [
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }
  ],
  [
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }
  ],
  [
    { weight: 1.4, fill: FILL_FUNCTION }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY },
    { weight: 1, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1.4, fill: FILL_FUNCTION }
  ],
  [
    { weight: 1.25, fill: FILL_FUNCTION }, { weight: 1.05, fill: FILL_FUNCTION }, { weight: 1, fill: FILL_KEY },
    { weight: 4, fill: FILL_KEY }, { weight: 1, fill: FILL_KEY }, { weight: 1.9, fill: FILL_ACCENT }
  ]
];

/**
 * 键盘内皮肤选择器的一个格子：皮肤名称上方是用皮肤颜色画的键盘缩略图，移植自 Android 的 `KeyboardSkinCard` 和 `KeyboardSkinPreview.drawTile`。
 *
 * 缩略图保持设计稿 MiniKb 的 390:292 比例、圆角 9，在那块 390 单位宽的画布上排版并按自身宽度缩放：工具栏的品牌块用 `accent`，然后是 `keyboardBackground` 上四排 `keyFill` 圆角格子，其中 ⇧、⌫、123 和 中 用 `functionKey`，回车键用 `accent`。键面文字、工具栏图标和 home indicator 都不画：它们需要皮肤的文字颜色，而选择器格子拿不到；在这个尺寸下，区分皮肤靠的正是色块。
 *
 * 名称在缩略图下方 6vp，字号 12fp。选中时缩略图外围是 2vp 的 `accent` 环，名称用 `accent`、字重 600；否则环是 1vp 的 `hair`，名称用 `nameColor`（面板的文字颜色，设计稿叫 `kb.fg`）、字重 400。环画在缩略图外侧，落在格子在上方和两侧保留的 2vp 内边距里，所以相邻格子之间的间距可以相应减少（见 `PagedTiles`）。按下时格子缩小到 .96，和设计稿一致。
 */
@Component
export struct MiniKeyboardPreview {
  @Require @Prop keyboardBackground: string;
  @Require @Prop keyFill: string;
  @Require @Prop functionKey: string;
  @Require @Prop accent: string;
  @Require @Prop hair: string;
  /** 未选中时名称的颜色，即面板的文字颜色。 */
  @Require @Prop nameColor: string;
  @Require @Prop name: string;
  @Require @Prop selected: boolean;
  @Require onTap: () => void;
  /** 缩略图的宽度（vp），来自布局；知道宽度后才绘制缩略图。 */
  @State private thumbWidth: number = 0;

  /** 一个 MiniKb 单位对应的 vp。 */
  private unit(): number {
    return this.thumbWidth / MINI_WIDTH;
  }

  private fillColor(fill: number): string {
    if (fill === FILL_ACCENT) {
      return this.accent;
    }
    return fill === FILL_FUNCTION ? this.functionKey : this.keyFill;
  }

  private rowInset(row: number): number {
    return row === 1 ? (MINI_WIDTH - MINI_PADDING * 2) * MIDDLE_ROW_INSET * this.unit() : 0;
  }

  private ringWidth(): number {
    return this.selected ? 2 : 1;
  }

  build() {
    // 选中环落在名称上方 6vp 的间距里，和设计稿的 box-shadow 环一样。
    Column({ space: NAME_GAP_VP }) {
      Column() {
        Column({ space: TOOLBAR_GAP * this.unit() }) {
          Row() {
            Stack() {
              Column()
                .width(BRAND_SIDE * this.unit())
                .height(BRAND_SIDE * this.unit())
                .borderRadius(BRAND_RADIUS * this.unit())
                .backgroundColor(this.accent)
            }
            .layoutWeight(1)
            .height('100%')
            Blank()
              .layoutWeight(TOOLBAR_COLUMNS - 1)
          }
          .width('100%')
          .height(TOOLBAR_HEIGHT * this.unit())

          Column({ space: ROW_GAP * this.unit() }) {
            ForEach(MINI_ROWS, (row: MiniKey[], rowIndex: number) => {
              Row({ space: KEY_GAP * this.unit() }) {
                ForEach(row, (key: MiniKey) => {
                  Column()
                    .layoutWeight(key.weight)
                    .height('100%')
                    .borderRadius(KEY_RADIUS * this.unit())
                    .backgroundColor(this.fillColor(key.fill))
                }, (_key: MiniKey, index: number): string => index.toString())
              }
              .width('100%')
              .height(ROW_HEIGHT * this.unit())
              .padding({ left: this.rowInset(rowIndex), right: this.rowInset(rowIndex) })
            }, (_row: MiniKey[], rowIndex: number): string => rowIndex.toString())
          }
          .width('100%')
        }
        .width('100%')
        .aspectRatio(MINI_WIDTH / MINI_HEIGHT)
        .padding({ left: MINI_PADDING * this.unit(), right: MINI_PADDING * this.unit() })
        .borderRadius(THUMB_RADIUS_VP)
        .backgroundColor(this.keyboardBackground)
        .clip(true)
        .onAreaChange((_old: Area, now: Area): void => {
          this.thumbWidth = Number(now.width);
        })
      }
      .width('100%')
      // 画在缩略图外侧，对应设计稿的 `box-shadow: 0 0 0 2px`，这样环永远不会盖住缩略图。
      .outline({
        width: this.ringWidth(),
        color: this.selected ? this.accent : this.hair,
        radius: THUMB_RADIUS_VP + this.ringWidth()
      })

      Text(this.name)
        .fontSize(NAME_FONT_SIZE)
        .fontColor(this.selected ? this.accent : this.nameColor)
        .fontWeight(this.selected ? 600 : 400)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .width('100%')
    .padding({ left: RING_ROOM_VP, right: RING_ROOM_VP, top: RING_ROOM_VP })
    .stateStyles({
      normal: {
        .scale({ x: 1, y: 1 })
      },
      pressed: {
        .scale({ x: 0.96, y: 0.96 })
      }
    })
    .animation({ duration: 100 })
    .accessibilityGroup(true)
    .accessibilityText(this.name)
    .accessibilitySelected(this.selected)
    .onClick(() => this.onTap())
  }
}
