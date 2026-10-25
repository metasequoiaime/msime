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
 * 手机语音面板展示录音、转写和上屏状态。主按钮结束采集或重试，取消按钮返回键盘；整句上传与流式识别共用同一界面，不把结果放进可编辑的输入框。
 * 光环沿用当前皮肤强调色，只对自身缩放和透明度做动画，随组件移除而结束。
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
  @Prop hint: string = '说完后自动上屏';
  @Prop primaryLabel: string = '';
  @Prop primaryEnabled: boolean = true;
  @Prop cancelLabel: string = '取消';
  onPrimary: () => void = (): void => {};
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
        .maxLines(2)
        .textAlign(TextAlign.Center)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .padding({ left: HINT_SIDE_PADDING_VP, right: HINT_SIDE_PADDING_VP })
      Row({ space: COLUMN_GAP_VP }) {
        if (this.primaryLabel.length > 0) {
          Button(this.primaryLabel)
            .fontSize(TITLE_FONT_SIZE)
            .fontColor(MIC_COLOR)
            .backgroundColor(this.accent)
            .enabled(this.primaryEnabled)
            .onClick(() => this.onPrimary())
        }
        Button(this.cancelLabel)
          .fontSize(TITLE_FONT_SIZE)
          .fontColor(this.foreground)
          .backgroundColor(Color.Transparent)
          .onClick(() => this.onCancel())
      }
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }
}
