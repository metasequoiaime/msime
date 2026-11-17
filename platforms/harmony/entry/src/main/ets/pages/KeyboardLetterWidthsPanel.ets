import { LetterWidthSettings, LetterWidthSettingsState } from '../keyboard/settings/LetterWidthSettings';
import { FourteenKeyLetterWidthMode } from '../keyboard/input/FourteenKeyLetterWidths';

@Component
export struct KeyboardLetterWidthsPanel {
  loadPreferences: () => string = (): string => '';
  savePreferences: (revision: number, document: string) => string = (_revision: number, _document: string): string => '';
  back: () => void = (): void => {};
  @Prop @Watch('reload') refresh: number = 0;
  @State private mode: FourteenKeyLetterWidthMode = 'equal';
  @State private ready: boolean = false;
  @State private error: string = '';

  aboutToAppear(): void { this.reload(); }

  private update(state: LetterWidthSettingsState): void {
    this.mode = state.mode;
    this.ready = state.ready;
    this.error = state.error;
  }

  private reload(): void {
    this.update(new LetterWidthSettings(this.loadPreferences, this.savePreferences).read());
  }

  private select(index: number): void {
    if (!this.ready || (index !== 0 && index !== 1)) return;
    this.update(new LetterWidthSettings(this.loadPreferences, this.savePreferences)
      .write(index === 1 ? 'varied' : 'equal'));
  }

  build() {
    Column({ space: 24 }) {
      Row({ space: 16 }) {
        Button('返回').onClick((): void => this.back())
        Text('键盘布局').fontSize(22).fontWeight(FontWeight.Bold).layoutWeight(1)
      }.width('100%')
      Row({ space: 8 }) {
        Text('14/18 键字母宽度').fontSize(16)
        Button('?').type(ButtonType.Circle).width(28).height(28)
          .accessibilityText('字母宽度说明')
          .onClick((): void => {
            this.getUIContext().showAlertDialog({ title: '字母宽度',
              message: '宽窄布局中，双字母键比单字母键更宽。适用于全拼和双拼的 14 键、18 键。',
              confirm: { value: '好', action: (): void => {} } });
          })
        Blank()
        Select([{ value: '等宽' }, { value: '宽窄' }])
          .selected(this.mode === 'varied' ? 1 : 0)
          .value(this.mode === 'varied' ? '宽窄' : '等宽')
          .enabled(this.ready)
          .accessibilityText('14/18 键字母宽度')
          .onSelect((index: number): void => this.select(index))
      }.width('100%')
      if (this.error.length > 0) {
        Row({ space: 12 }) {
          Text(this.error).fontColor($r('sys.color.ohos_id_color_text_secondary')).layoutWeight(1)
          Button('重试').onClick((): void => this.reload())
        }.width('100%')
      }
      Blank()
    }.padding(20).width('100%').height('100%')
      .backgroundColor($r('app.color.start_window_background'))
  }
}
