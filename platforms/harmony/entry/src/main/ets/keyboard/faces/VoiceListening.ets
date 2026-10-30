import { FrameCallback } from '@kit.ArkUI';
import { KeyboardLineIcon } from './KeyboardLineIcon';
import {
  VOICE_GAP_INDICATOR_TITLE_VP,
  VOICE_GAP_TITLE_HINT_VP,
  VOICE_INNER_RING_OPACITY,
  VOICE_OUTER_RING_OPACITY,
  VOICE_PAD_TOP_VP,
  VoiceListeningPlan,
  VoiceListeningPolicy
} from './VoiceListeningPolicy';

/** 麦克风图标边长占圆盘直径的比例，沿用设计稿 72vp 圆盘配 30vp 图标。 */
const MIC_RATIO: number = 30 / 72;
const MIC_STROKE: number = 1.8;
const TITLE_FONT_SIZE: number = 16;
const HINT_FONT_SIZE: number = 13;
const ACTION_FONT_SIZE: number = 15;
const TEXT_SIDE_PADDING_VP: number = 16;
const ACTIONS_SIDE_INSET_VP: number = 16;
const ACTIONS_BOTTOM_MARGIN_VP: number = 10;
const ACTIONS_GAP_VP: number = 12;
const ACTION_MIN_HEIGHT_VP: number = 40;
/** 实测单行高度用的样字，含中文和带下伸部的拉丁字母。 */
const LINE_SAMPLE: string = '聆听Ag';
/** 帧回调第一次触发时还没有上一帧的时间，按一帧 16ms 推进。 */
const FIRST_FRAME_MS: number = 16;

/** 把逐帧回调转给面板；只在显示的音量还没追上目标时才挂上，追上就不再挂。 */
class LevelFrame extends FrameCallback {
  private readonly tick: (frameTimeInNano: number) => void;

  constructor(tick: (frameTimeInNano: number) => void) {
    super();
    this.tick = tick;
  }

  onFrame(frameTimeInNano: number): void {
    this.tick(frameTimeInNano);
  }
}

/**
 * 手机语音面板展示录音、转写和上屏状态。主按钮结束采集或重试，取消按钮返回键盘；整句上传与流式识别共用同一界面，不把结果放进可编辑的输入框。两个按钮的文字都为空时（按住说话的会话在收尾识别）不画按钮行，上面那块区域占满整个面板。
 *
 * 排布按 `VoiceListeningPolicy.plan`：按钮行上面那块区域（没有按钮行时是整个面板）的实测高度决定圆盘多大、标题位留几行、画不画提示行，光圈的最大外扩算在方框里，满音量时也不出面板。光圈随麦克风电平（`level`）缩放，只在显示值还没追上电平时逐帧推进，追上就停。当前 API 21 编译 SDK 不提供系统「减弱动画」查询接口。
 */
@Component
export struct VoiceListening {
  @Require @Prop accent: string;
  /** 画在强调色上的文字和图标的颜色，即皮肤的 `onAccent`。 */
  @Require @Prop onAccent: string;
  @Require @Prop foreground: string;
  @Require @Prop secondary: string;
  @Require @Prop micPath: string;
  /** 圆盘下方的标题位：状态文字，或 `titleTail` 时目前听到的文字。 */
  @Prop @Watch('refit') title: string = '正在聆听…';
  /** 标题是转写的中间结果：放不下预留的行数时从开头省略，留住最新说的那段。 */
  @Prop @Watch('refit') titleTail: boolean = false;
  /** 再下面较小的一行：如何结束、目前听到的文字，或已上屏的内容。 */
  @Prop @Watch('refit') hint: string = '说完后自动上屏';
  /** 提示是转写文字：放不下一行时从开头省略，`hintPrefix` 始终留在最前面。 */
  @Prop @Watch('refit') hintTail: boolean = false;
  @Prop @Watch('refit') hintPrefix: string = '';
  /** 麦克风电平，0–1；不在录音时为 0。 */
  @Prop @Watch('levelChanged') level: number = 0;
  @Prop primaryLabel: string = '';
  @Prop primaryEnabled: boolean = true;
  /** 为空时不画取消按钮。 */
  @Prop cancelLabel: string = '取消';
  onPrimary: () => void = (): void => {};
  @Require onCancel: () => void;
  @State private plan: VoiceListeningPlan = VoiceListeningPolicy.plan(0, 0, 0);
  @State private shownTitle: string = '';
  @State private shownHint: string = '';
  /** 光圈当前画到的音量，逐帧追向 `level`。 */
  @State private shown: number = 0;
  private regionWidth: number = 0;
  private regionHeight: number = 0;
  private titleLine: number = 0;
  private hintLine: number = 0;
  private visible: boolean = false;
  private framePending: boolean = false;
  private lastFrameNs: number = -1;
  private frame: LevelFrame | undefined = undefined;

  aboutToAppear(): void {
    this.visible = true;
    this.frame = new LevelFrame((frameTimeInNano: number): void => this.advance(frameTimeInNano));
    this.refit();
  }

  aboutToDisappear(): void {
    this.visible = false;
    this.framePending = false;
  }

  private levelChanged(): void {
    this.schedule();
  }

  private schedule(): void {
    if (this.framePending || !this.visible || this.frame === undefined
      || VoiceListeningPolicy.settled(this.shown, this.level)) {
      return;
    }
    this.framePending = true;
    this.getUIContext().postFrameCallback(this.frame);
  }

  private advance(frameTimeInNano: number): void {
    if (!this.framePending || !this.visible) return;
    this.framePending = false;
    const elapsed: number = this.lastFrameNs < 0 ? FIRST_FRAME_MS : (frameTimeInNano - this.lastFrameNs) / 1e6;
    this.lastFrameNs = frameTimeInNano;
    this.shown = VoiceListeningPolicy.step(this.shown, this.level, elapsed);
    if (VoiceListeningPolicy.settled(this.shown, this.level)) {
      this.lastFrameNs = -1;
      return;
    }
    this.schedule();
  }

  private regionChanged(area: Area): void {
    const width: number = Number(area.width);
    const height: number = Number(area.height);
    if (width === this.regionWidth && height === this.regionHeight) return;
    this.regionWidth = width;
    this.regionHeight = height;
    this.relayout();
  }

  /** 按区域高度和实测行高重排；字号跟随系统字体大小，所以行高每次都重新量。 */
  private relayout(): void {
    this.titleLine = this.lineHeight(TITLE_FONT_SIZE);
    this.hintLine = this.lineHeight(HINT_FONT_SIZE);
    const next: VoiceListeningPlan = VoiceListeningPolicy.plan(this.regionHeight, this.titleLine, this.hintLine);
    if (next.orb !== this.plan.orb || next.halo !== this.plan.halo || next.titleLines !== this.plan.titleLines
      || next.showsHint !== this.plan.showsHint || next.top !== this.plan.top) {
      this.plan = next;
    }
    this.refit();
  }

  private lineHeight(fontSize: number): number {
    const size: SizeOptions = this.getUIContext().getMeasureUtils().measureTextSize({
      textContent: LINE_SAMPLE, fontSize: `${fontSize}fp`, maxLines: 1
    });
    return this.getUIContext().px2vp(Number(size.height));
  }

  /** 这段文字按给定字号、在文字区宽度内排开后不超过 `lines` 行。多行的总高不一定正好是单行的整数倍，所以按「不到 `lines + 0.5` 行高」判断，不会因取整差一点就当成多出一行。 */
  private fits(text: string, fontSize: number, lines: number, lineHeight: number): boolean {
    const size: SizeOptions = this.getUIContext().getMeasureUtils().measureTextSize({
      textContent: text, fontSize: `${fontSize}fp`, constraintWidth: `${this.textWidth()}vp`
    });
    return this.getUIContext().px2vp(Number(size.height)) < (lines + 0.5) * lineHeight;
  }

  private textWidth(): number {
    return Math.max(0, this.regionWidth - 2 * TEXT_SIDE_PADDING_VP);
  }

  /** 标题和提示的文字随内容或排布变化重算；还没量到区域时原样显示。 */
  private refit(): void {
    const measured: boolean = this.regionWidth > 0 && this.titleLine > 0;
    const lines: number = this.plan.titleLines;
    this.shownTitle = this.titleTail && measured
      ? VoiceListeningPolicy.fitTail(this.title,
        (candidate: string): boolean => this.fits(candidate, TITLE_FONT_SIZE, lines, this.titleLine))
      : this.title;
    const prefix: string = this.hintPrefix;
    this.shownHint = this.hintTail && measured
      ? prefix + VoiceListeningPolicy.fitTail(this.hint,
        (candidate: string): boolean => this.fits(prefix + candidate, HINT_FONT_SIZE, 1, this.hintLine))
      : prefix + this.hint;
  }

  private box(): number {
    return this.plan.orb + 2 * this.plan.halo;
  }

  private ringScale(share: number): number {
    return VoiceListeningPolicy.ringScale(this.plan.orb, this.plan.halo, this.shown, share);
  }

  build() {
    Column() {
      Column() {
        Stack() {
          Circle()
            .width(this.box())
            .height(this.box())
            .fill(this.accent)
            .opacity(VOICE_OUTER_RING_OPACITY)
            .scale({ x: this.ringScale(1), y: this.ringScale(1) })
          Circle()
            .width(this.box())
            .height(this.box())
            .fill(this.accent)
            .opacity(VOICE_INNER_RING_OPACITY)
            .scale({ x: this.ringScale(0.5), y: this.ringScale(0.5) })
          Circle()
            .width(this.plan.orb)
            .height(this.plan.orb)
            .fill(this.accent)
          KeyboardLineIcon({
            d: this.micPath,
            iconSize: MIC_RATIO * this.plan.orb,
            stroke: MIC_STROKE,
            color: this.onAccent
          })
        }
        .width(this.box())
        .height(this.box())
        .margin({ top: this.plan.top + VOICE_PAD_TOP_VP })

        Text(this.shownTitle)
          .fontSize(TITLE_FONT_SIZE)
          .fontColor(this.foreground)
          .maxLines(this.plan.titleLines)
          .textAlign(TextAlign.Center)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .padding({ left: TEXT_SIDE_PADDING_VP, right: TEXT_SIDE_PADDING_VP })
          .margin({ top: VOICE_GAP_INDICATOR_TITLE_VP })
        if (this.plan.showsHint) {
          Text(this.shownHint)
            .fontSize(HINT_FONT_SIZE)
            .fontColor(this.secondary)
            .maxLines(1)
            .textAlign(TextAlign.Center)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .padding({ left: TEXT_SIDE_PADDING_VP, right: TEXT_SIDE_PADDING_VP })
            .margin({ top: VOICE_GAP_TITLE_HINT_VP })
        }
      }
      .width('100%')
      .layoutWeight(1)
      .justifyContent(FlexAlign.Start)
      .alignItems(HorizontalAlign.Center)
      .clip(true)
      .onAreaChange((_old: Area, area: Area): void => this.regionChanged(area))

      if (this.primaryLabel.length > 0 || this.cancelLabel.length > 0) {
        Row({ space: ACTIONS_GAP_VP }) {
          if (this.primaryLabel.length > 0) {
            Button(this.primaryLabel)
              .fontSize(ACTION_FONT_SIZE)
              .fontColor(this.onAccent)
              .backgroundColor(this.accent)
              .constraintSize({ minHeight: ACTION_MIN_HEIGHT_VP })
              .enabled(this.primaryEnabled)
              .onClick(() => this.onPrimary())
          }
          if (this.cancelLabel.length > 0) {
            Button(this.cancelLabel)
              .fontSize(ACTION_FONT_SIZE)
              .fontColor(this.foreground)
              .backgroundColor(Color.Transparent)
              .constraintSize({ minHeight: ACTION_MIN_HEIGHT_VP })
              .onClick(() => this.onCancel())
          }
        }
        .width('100%')
        .justifyContent(FlexAlign.Center)
        .padding({ left: ACTIONS_SIDE_INSET_VP, right: ACTIONS_SIDE_INSET_VP })
        .margin({ bottom: ACTIONS_BOTTOM_MARGIN_VP })
      }
    }
    .width('100%')
    .height('100%')
  }
}
