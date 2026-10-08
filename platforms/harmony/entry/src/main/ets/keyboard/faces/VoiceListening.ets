import { KeyboardLineIcon } from './KeyboardLineIcon';

const ORB_VP: number = 72;
const MIC_VP: number = 30;
const MIC_STROKE: number = 1.8;
const MIC_COLOR: string = '#FFFFFF';
const COLUMN_GAP_VP: number = 14;
const TITLE_FONT_SIZE: number = 15;
const HINT_FONT_SIZE: number = 12.5;
const HINT_SIDE_PADDING_VP: number = 16;
const PULSE_MS: number = 1200;
const PULSE_START_OPACITY: number = 0.35;
/** 光环在圆球四周各向外扩出 18vp，即设计稿的 `box-shadow: 0 0 0 18px`：(72 + 36) / 72。 */
const PULSE_END_SCALE: number = (ORB_VP + 36) / ORB_VP;

/**
 * 语音面板的聆听界面，移植自 Android 的 VoiceListeningView：一个 72vp 的 `accent` 圆球，中间是白色 30vp 麦克风，每 1.2s 从中脉冲出一圈光环，下面是标题（正在聆听…）和提示（点任意处取消），宿主会像 Android 视图那样在识别到文字时和结果上屏后改写它们。
 *
 * 整个界面是一个点击目标：点任意处调用 `onCancel`。设计稿把脉冲画成固定的绿色（rgba(46,125,80,.35)），在秋季和冬季强调色下显得突兀，所以光环用 `accent` 以同样的 .35 绘制。`micPath` 是 kb-core 中 24 单位 viewBox 下的 MIC 路径。
 *
 * 脉冲在光环出现时开始，重复直到界面被移除。它只对光环自身的缩放和不透明度状态做动画，所以随光环节点一同结束，没有计时器活得比组件更久。
 */
@Component
export struct VoiceListening {
  @Require @Prop accent: string;
  @Require @Prop foreground: string;
  @Require @Prop secondary: string;
  @Require @Prop micPath: string;
  /** 圆球下方的那行文字；结果上屏后宿主显示识别完成。 */
  @Prop title: string = '正在聆听…';
  /** 再下面较小的一行：如何取消、目前听到的文字，或带上已上屏内容的已识别：…。 */
  @Prop hint: string = '点任意处取消';
  @Require onCancel: () => void;
  @State private pulseScale: number = 1;
  @State private pulseOpacity: number = PULSE_START_OPACITY;

  private startPulse(): void {
    this.getUIContext().animateTo({
      duration: PULSE_MS,
      curve: Curve.EaseInOut,
      iterations: -1,
      playMode: PlayMode.Normal
    }, (): void => {
      this.pulseScale = PULSE_END_SCALE;
      this.pulseOpacity = 0;
    });
  }

  build() {
    Column({ space: COLUMN_GAP_VP }) {
      Stack() {
        Circle()
          .width(ORB_VP)
          .height(ORB_VP)
          .fill(this.accent)
          .scale({ x: this.pulseScale, y: this.pulseScale })
          .opacity(this.pulseOpacity)
          .onAppear(() => this.startPulse())
        Circle()
          .width(ORB_VP)
          .height(ORB_VP)
          .fill(this.accent)
        KeyboardLineIcon({ d: this.micPath, iconSize: MIC_VP, stroke: MIC_STROKE, color: MIC_COLOR })
      }
      .width(ORB_VP)
      .height(ORB_VP)

      Text(this.title)
        .fontSize(TITLE_FONT_SIZE)
        .fontColor(this.foreground)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      Text(this.hint)
        .fontSize(HINT_FONT_SIZE)
        .fontColor(this.secondary)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .padding({ left: HINT_SIDE_PADDING_VP, right: HINT_SIDE_PADDING_VP })
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .accessibilityGroup(true)
    .accessibilityText(`${this.title}，${this.hint}`)
    .onClick(() => this.onCancel())
  }
}
