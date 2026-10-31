import { KeyboardIconPaths } from '../KeyboardIconPaths';

const TILE_SIDE_VP: number = 40;
const TILE_RADIUS_VP: number = 12;
/** 圆底比工具图标大一圈；标志扣除 viewBox 内留白后，圆盘与标志之间仍留出清楚的边距。 */
const DISC_VP: number = KeyboardIconPaths.TOOLBAR_SIZE_VP + 7;
const MARK_HEIGHT_VP: number = KeyboardIconPaths.TOOLBAR_SIZE_VP - 2;

/**
 * 工具栏的「功能」键：季节色圆盘上的官方标志，移植自 Android 的 `KeyboardBrandButton`。
 *
 * 40x40、圆角 12 的图块，平时透明，`active` 时（功能面板、选择器或其他工具界面打开）用 `toggleBackground` 填充。其中是一个 `logoCirc` 色的 30vp 圆盘，承载 21vp 的标志：用 `logoBg` 填充的 `msime_frame` 矩形，加一道宽 9、圆头的白色描边。两条路径字符串都来自 kb-core 的 `KeyboardIconPaths`（`BRAND_FRAME`、`BRAND_STROKE`），viewBox 为 116x132。
 *
 * ArkUI 的 `Path` 以 px 解释命令，所以 viewport 和描边宽度也用 px，由 `Shape` 的 viewport 变换把整幅图缩放到标志尺寸。哪些行为需要在真机上确认，见 `KeyboardLineIcon`。
 */
@Component
export struct BrandTile {
  @Require @Prop active: boolean;
  @Require @Prop toggleBackground: string;
  @Require @Prop logoCirc: string;
  @Require @Prop logoBg: string;
  @Require @Prop framePath: string;
  @Require @Prop strokePath: string;
  /** 跟着工具条缩小的倍数（`KeyboardMetrics.toolbarScale`）：「候选栏大小」调小、工具条矮于 50vp 时图块、圆盘和标志等比缩小，不伸出这一行。 */
  @Prop sizeScale: number = 1;
  @Require @Prop spokenLabel: string;
  @Require onTap: () => void;
  /** 长按：工具栏用它打开设置里的工具栏开关。长按被识别后不再触发 `onTap`。 */
  @Require onHold: () => void;

  build() {
    Stack() {
      Stack() {
        Shape() {
          Path()
            .width(`${KeyboardIconPaths.BRAND_VIEW_WIDTH}px`)
            .height(`${KeyboardIconPaths.BRAND_VIEW_HEIGHT}px`)
            .commands(this.framePath)
            .fill(this.logoBg)
            .stroke(Color.Transparent)
            .strokeWidth(0)
          Path()
            .width(`${KeyboardIconPaths.BRAND_VIEW_WIDTH}px`)
            .height(`${KeyboardIconPaths.BRAND_VIEW_HEIGHT}px`)
            .commands(this.strokePath)
            .fill(Color.Transparent)
            .stroke(KeyboardIconPaths.BRAND_STROKE_COLOR)
            .strokeWidth(`${KeyboardIconPaths.BRAND_STROKE_WIDTH}px`)
            .strokeLineCap(LineCapStyle.Round)
            .strokeLineJoin(LineJoinStyle.Round)
        }
        .viewPort({ x: 0, y: 0, width: `${KeyboardIconPaths.BRAND_VIEW_WIDTH}px`, height: `${KeyboardIconPaths.BRAND_VIEW_HEIGHT}px` })
        // 用 viewBox 自身的宽高比，使标志按工具图标的尺寸等比居中，也不依赖 viewport 变换如何居中另一种比例的框。
        .width(MARK_HEIGHT_VP * this.sizeScale * KeyboardIconPaths.BRAND_VIEW_WIDTH / KeyboardIconPaths.BRAND_VIEW_HEIGHT)
        .height(MARK_HEIGHT_VP * this.sizeScale)
      }
      .width(DISC_VP * this.sizeScale)
      .height(DISC_VP * this.sizeScale)
      .borderRadius(DISC_VP * this.sizeScale / 2)
      .backgroundColor(this.logoCirc)
    }
    .width(TILE_SIDE_VP * this.sizeScale)
    .height(TILE_SIDE_VP * this.sizeScale)
    .borderRadius(TILE_RADIUS_VP * this.sizeScale)
    .backgroundColor(this.active ? this.toggleBackground : Color.Transparent)
    .stateStyles({
      normal: {
        .opacity(1)
      },
      pressed: {
        .opacity(0.55)
      }
    })
    .accessibilityGroup(true)
    .accessibilityText(this.spokenLabel)
    .accessibilitySelected(this.active)
    .onClick(() => this.onTap())
    .gesture(LongPressGesture().onAction(() => this.onHold()))
  }
}
