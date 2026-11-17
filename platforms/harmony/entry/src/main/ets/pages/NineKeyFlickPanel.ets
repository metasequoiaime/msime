import { FlickThresholdSettings, FlickThresholdSettingsState } from '../keyboard/settings/FlickThresholdSettings';
import { NineKeyFlickPolicy } from '../keyboard/input/NineKeyFlickPolicy';

const FLICK_HELP: string = '滑动超过此距离后输入字母：上滑第一个、左滑第二个、右滑第三个。距离越小越容易触发；数字用长按输入。';

@Component
export struct NineKeyFlickPanel {
  loadPreferences: () => string = (): string => '';
  savePreferences: (revision: number, document: string) => string = (_revision: number, _document: string): string => '';
  back: () => void = (): void => {};
  @Prop @Watch('reload') refresh: number = 0;
  @State private threshold: number = NineKeyFlickPolicy.DEFAULT_THRESHOLD_VP;
  @State private ready: boolean = false;
  @State private error: string = '';

  aboutToAppear(): void { this.reload(); }

  private update(state: FlickThresholdSettingsState): void {
    this.threshold = state.threshold;
    this.ready = state.ready;
    this.error = state.error;
  }

  private reload(): void {
    this.update(new FlickThresholdSettings(this.loadPreferences, this.savePreferences).read());
  }

  private change(value: number, mode: SliderChangeMode): void {
    if (!this.ready) return;
    this.threshold = value;
    if (mode === SliderChangeMode.End || mode === SliderChangeMode.Click) {
      this.update(new FlickThresholdSettings(this.loadPreferences, this.savePreferences).write(value));
    }
  }

  build() {
    Column({ space: 24 }) {
      Row({ space: 16 }) {
        Button('返回').onClick((): void => this.back())
        Text('九键四向滑动').fontSize(22).fontWeight(FontWeight.Bold).layoutWeight(1)
      }.width('100%')
      Column({ space: 12 }) {
        Row({ space: 8 }) {
          Text('滑动距离').fontSize(16)
          Button('?').type(ButtonType.Circle).width(28).height(28)
            .accessibilityText(`滑动距离的说明：${FLICK_HELP}`)
            .onClick((): void => {
              this.getUIContext().showAlertDialog({ title: '滑动距离', message: FLICK_HELP,
                confirm: { value: '好', action: (): void => {} } });
            })
          Blank()
          Text(`${this.threshold}`).fontSize(16)
        }.width('100%')
        Slider({ value: this.threshold, min: NineKeyFlickPolicy.MIN_THRESHOLD_VP,
          max: NineKeyFlickPolicy.MAX_THRESHOLD_VP, step: NineKeyFlickPolicy.THRESHOLD_STEP_VP })
          .enabled(this.ready)
          .accessibilityText('九键滑动距离')
          .onChange((value: number, mode: SliderChangeMode): void => this.change(value, mode))
      }.width('100%')
      if (this.error.length > 0) {
        Row({ space: 12 }) {
          Text(this.error).fontColor($r('sys.color.font_secondary')).layoutWeight(1)
          Button('重试').onClick((): void => this.reload())
        }.width('100%')
      }
      Blank()
    }.padding(20).width('100%').height('100%')
      .backgroundColor($r('app.color.start_window_background'))
  }
}
