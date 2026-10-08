const BAR_HEIGHT_VP: number = 50;
const BAR_GAP_VP: number = 2;
const BAR_PADDING_VP: number = 2;
const TEXT_BUTTON_HEIGHT_VP: number = 36;
const TEXT_BUTTON_FONT_SIZE: number = 15;
const CANCEL_PADDING_VP: number = 12;
const RESET_PADDING_VP: number = 10;
const HANDLE_AREA_HEIGHT_VP: number = 44;
const HANDLE_GAP_VP: number = 5;
const HANDLE_WIDTH_VP: number = 44;
const HANDLE_HEIGHT_VP: number = 5;
const HANDLE_RADIUS_VP: number = 3;
const HANDLE_OPACITY: number = 0.35;
const CAPTION_FONT_SIZE: number = 12;
const DONE_HEIGHT_VP: number = 32;
const DONE_PADDING_VP: number = 14;
const DONE_FONT_SIZE: number = 14;
const DONE_FONT_WEIGHT: number = 600;

/**
 * 编辑键盘高度期间取代 50vp 工具栏的高度条，移植自 Android 的 `InlineHeightBar`：取消 | 标注「上下拖动调整 · N%」的拖动手柄 | 重置 | 完成。
 *
 * 它自身不保存高度。调整由调用方负责，并通过 `percent` 传入要显示的百分比；手柄只报告手指移动距离。`onDrag(deltaVp)` 收到自上次调用以来的纵向移动距离，单位 vp，向上为负，所以向上拖（负的 delta）意味着让键盘变高；手指抬起或手势取消后接着调用 `onRelease()`。取消、重置和完成分别通过 `onCancel`、`onReset` 和 `onDone` 报告，自身不改变任何东西。
 *
 * 当前 SDK 的 HarmonyOS 没有可调节的无障碍操作（`AccessibilityAction` 只提供 click），因此手柄朗读当前百分比和调整方法，取消、重置和完成都是普通按钮。
 */
@Component
export struct HeightBar {
  @Require @Prop percent: number;
  @Require @Prop foreground: string;
  @Require @Prop secondary: string;
  @Require @Prop accent: string;
  @Require @Prop onAccent: string;
  @Require onDrag: (deltaVp: number) => void;
  @Require onRelease: () => void;
  @Require onCancel: () => void;
  @Require onReset: () => void;
  @Require onDone: () => void;
  /** 手势在上一次更新时的累计偏移，让每次报告的都是此后的移动距离。 */
  private dragOffsetVp: number = 0;

  private caption(): string {
    return `上下拖动调整 · ${Math.round(this.percent)}%`;
  }

  private endDrag(): void {
    this.dragOffsetVp = 0;
    this.onRelease();
  }

  @Builder
  textButton(label: string, padding: number, action: () => void) {
    Text(label)
      .height(TEXT_BUTTON_HEIGHT_VP)
      .padding({ left: padding, right: padding })
      .fontSize(TEXT_BUTTON_FONT_SIZE)
      .fontColor(this.secondary)
      .maxLines(1)
      .flexShrink(0)
      .stateStyles({
        normal: {
          .opacity(1)
        },
        pressed: {
          .opacity(0.55)
        }
      })
      .onClick(() => action())
  }

  build() {
    Row({ space: BAR_GAP_VP }) {
      this.textButton('取消', CANCEL_PADDING_VP, (): void => this.onCancel())

      Column({ space: HANDLE_GAP_VP }) {
        Column()
          .width(HANDLE_WIDTH_VP)
          .height(HANDLE_HEIGHT_VP)
          .borderRadius(HANDLE_RADIUS_VP)
          .backgroundColor(this.foreground)
          .opacity(HANDLE_OPACITY)
        Text(this.caption())
          .fontSize(CAPTION_FONT_SIZE)
          .fontColor(this.secondary)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .height(HANDLE_AREA_HEIGHT_VP)
      .justifyContent(FlexAlign.Center)
      .alignItems(HorizontalAlign.Center)
      .accessibilityGroup(true)
      .accessibilityText(`键盘高度 ${Math.round(this.percent)}%`)
      .accessibilityDescription('上下拖动调整')
      .gesture(
        PanGesture({ direction: PanDirection.Vertical, distance: 1 })
          .onActionStart(() => {
            this.dragOffsetVp = 0;
          })
          .onActionUpdate((event: GestureEvent) => {
            const delta: number = event.offsetY - this.dragOffsetVp;
            this.dragOffsetVp = event.offsetY;
            if (delta !== 0) {
              this.onDrag(delta);
            }
          })
          .onActionEnd(() => this.endDrag())
          .onActionCancel(() => this.endDrag())
      )

      this.textButton('重置', RESET_PADDING_VP, (): void => this.onReset())

      Text('完成')
        .height(DONE_HEIGHT_VP)
        .padding({ left: DONE_PADDING_VP, right: DONE_PADDING_VP })
        .borderRadius(DONE_HEIGHT_VP / 2)
        .backgroundColor(this.accent)
        .fontSize(DONE_FONT_SIZE)
        .fontWeight(DONE_FONT_WEIGHT)
        .fontColor(this.onAccent)
        .maxLines(1)
        .flexShrink(0)
        .stateStyles({
          normal: {
            .opacity(1)
          },
          pressed: {
            .opacity(0.7)
          }
        })
        .onClick(() => this.onDone())
    }
    .width('100%')
    .height(BAR_HEIGHT_VP)
    .padding({ left: BAR_PADDING_VP, right: BAR_PADDING_VP })
    .alignItems(VerticalAlign.Center)
  }
}
