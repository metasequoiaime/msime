/** 设计稿 MiniKb 的虚拟画布；缩略图里的一切都按这个单位排版，再按缩略图的宽度缩放。 */
const MINI_WIDTH: number = 390;
const MINI_HEIGHT: number = 292;
/** 键行左右的内边距。设计稿是 3，但面板里的缩略图只有 80vp 左右宽，3 个单位不到 1vp，最外侧的键贴着描边、底排两角的键被圆角切掉，所以这里加宽到 10。 */
const MINI_PADDING: number = 10;
const TOOLBAR_HEIGHT: number = 50;
const TOOLBAR_GAP: number = 8;
const TOOLBAR_COLUMNS: number = 7;
const BRAND_SIDE: number = 26;
const ROW_HEIGHT: number = 43;
const ROW_GAP: number = 11;
const KEY_GAP: number = 6;
const KEY_RADIUS: number = 5;
/** 第二排字母键两侧各向内缩进一行宽度的 5%，和真实键盘一致。 */
const MIDDLE_ROW_INSET: number = 0.05;
const THUMB_RADIUS_VP: number = 9;
/** 选中环和缩略图之间的间隙，透出面板底色，对应设计稿 `box-shadow: 0 0 0 2px <页面底色>, 0 0 0 4px <强调色>` 里的第一圈。没有它，环直接贴着缩略图，看上去像压住了两侧的按键。 */
const RING_GAP_VP: number = 2;
/** 选中环本身的宽度。间隙和环在每张卡片上都占着位置（未选中时透明），选中时卡片不会变大，环也不会伸出格子被网格裁掉。 */
const RING_VP: number = 2;
const NAME_GAP_VP: number = 4;
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
 * 名称在选中环下方 4vp，字号 12fp。选中时缩略图外侧隔 2vp 间隙是 2vp 的 `accent` 环，名称用 `accent`、字重 600；否则缩略图外侧是 1vp 的 `hair` 细线，名称用 `nameColor`（面板的文字颜色，设计稿叫 `kb.fg`）、字重 400。间隙和环在格子四周各占 4vp，不管选没选中都占着，所以相邻格子之间的间距可以相应减少（见 `PagedTiles`）。按下时格子缩小到 .96，和设计稿一致。
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

  build() {
    Column({ space: NAME_GAP_VP }) {
      Column() {
        Column({ space: TOOLBAR_GAP * this.unit() }) {
          Row() {
            Stack() {
              // 品牌位画真正的水杉标志（app_icon 就是它：深色圆角方块、绿边、白折线），与工具栏的品牌键一致，不再是一块强调色方块。
              Image($r('app.media.app_icon'))
                .width(BRAND_SIDE * this.unit())
                .height(BRAND_SIDE * this.unit())
                .objectFit(ImageFit.Contain)
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
        // 未选中时缩略图外侧一条细线，与设计稿的 `box-shadow: 0 0 0 1px` 一致。
        .outline({ width: this.selected ? 0 : 1, color: this.hair, radius: THUMB_RADIUS_VP + 1 })
        .onAreaChange((_old: Area, now: Area): void => {
          this.thumbWidth = Number(now.width);
        })
      }
      .width('100%')
      .padding(RING_GAP_VP)
      .border({
        width: RING_VP,
        color: this.selected ? this.accent : Color.Transparent,
        radius: THUMB_RADIUS_VP + RING_GAP_VP + RING_VP
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

/**
 * 皮肤选择器最后一格「更多皮肤」：和皮肤缩略图同样大小的虚线框，中间一个「+」，下方是名称，点了打开设置应用的「皮肤」页。
 *
 * 键盘进程读不到设置应用下载的社区皮肤和保存的设计（两边没有共享沙箱，见 `HarmonySharedSandboxPolicy`），选择器里只有内置主题和自定义；这一格把用户带到能用全部皮肤的地方。尺寸、圆角和选中环的留位与 `MiniKeyboardPreview` 相同，放在同一行里对得齐。
 */
@Component
export struct MoreSkinsTile {
  @Require @Prop hair: string;
  @Require @Prop foreground: string;
  @Require @Prop nameColor: string;
  @Require @Prop name: string;
  onTap: () => void = () => {};

  build() {
    Column({ space: NAME_GAP_VP }) {
      Column() {
        Stack() {
          Text('+')
            .fontSize(26)
            .fontWeight(300)
            .fontColor(this.foreground)
        }
        .width('100%')
        .aspectRatio(MINI_WIDTH / MINI_HEIGHT)
        .borderRadius(THUMB_RADIUS_VP)
        .border({ width: 1, color: this.hair, style: BorderStyle.Dashed, radius: THUMB_RADIUS_VP })
      }
      .width('100%')
      .padding(RING_GAP_VP + RING_VP)

      Text(this.name)
        .fontSize(NAME_FONT_SIZE)
        .fontColor(this.nameColor)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)
        .width('100%')
    }
    .width('100%')
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
    .onClick(() => this.onTap())
  }
}
