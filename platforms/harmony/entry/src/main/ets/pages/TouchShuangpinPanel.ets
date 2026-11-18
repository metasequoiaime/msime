import { AppEdition } from '../keyboard/AppEdition';
import { ShuangpinOption, ShuangpinSettingsBridge, TouchShuangpinSettings } from '../keyboard/settings/TouchShuangpinSettings';

@Component
export struct TouchShuangpinPanel {
  bridge: ShuangpinSettingsBridge = {
    loadPreferences: (): string => '',
    savePreferences: (_revision: number, _document: string): string => '',
    plan: (_request: string): string => ''
  };
  back: () => void = (): void => {};
  @Prop @Watch('refresh') foregroundRevision: number = 0;
  @State private options: ShuangpinOption[] = [];
  @State private profile: string = '';
  @State private message: string = '';
  @State private ready: boolean = false;
  private model: TouchShuangpinSettings | undefined = undefined;

  aboutToAppear(): void {
    this.model = new TouchShuangpinSettings(this.bridge, AppEdition.current().id);
    this.refresh();
  }

  private update(): void {
    if (this.model === undefined) return;
    this.options = this.model.options.slice();
    this.profile = this.model.profile;
    this.message = this.model.message;
    this.ready = this.model.ready;
  }

  private refresh(): void {
    this.model?.load();
    this.update();
  }

  private select(profile: string): void {
    this.model?.select(profile);
    this.update();
  }

  private help(): void {
    this.getUIContext().showAlertDialog({ title: '双拼方案',
      message: '选择后切换为该双拼，并添加到已启用输入方式；其他输入方式保留。',
      confirm: { value: '知道了', action: (): void => {} } });
  }

  build() {
    Column({ space: 16 }) {
      Row({ space: 8 }) {
        Button('返回').onClick((): void => this.back())
        Text('双拼方案').fontSize(22).fontWeight(FontWeight.Bold).layoutWeight(1).textAlign(TextAlign.Center)
        Button('?').accessibilityText('双拼方案的说明：选择后切换为该双拼，并添加到已启用输入方式；其他输入方式保留。')
          .onClick((): void => this.help())
      }.width('100%')
      Scroll() {
        Column({ space: 8 }) {
          ForEach(this.options, (option: ShuangpinOption) => {
            Button() {
              Row() {
                Text(option.title).fontSize(17).fontColor($r('sys.color.font_primary')).layoutWeight(1)
                Radio({ value: option.profile, group: 'touch-shuangpin' }).checked(this.profile === option.profile)
                  .enabled(this.ready).hitTestBehavior(HitTestMode.None)
              }.width('100%').padding(16)
            }.width('100%').type(ButtonType.Normal).borderRadius(12)
              .backgroundColor($r('sys.color.comp_background_primary'))
              .enabled(this.ready).accessibilityText(`${option.title}${this.profile === option.profile ? '，已选' : ''}`)
              .onClick((): void => this.select(option.profile))
          }, (option: ShuangpinOption): string => option.profile)
          if (this.profile === 'custom') {
            Text('当前方案：自定义').fontSize(14).width('100%')
          }
          if (this.message.length > 0) {
            Text(this.message).fontSize(14).width('100%')
          }
          Button('刷新').onClick((): void => this.refresh())
        }.width('100%')
      }.layoutWeight(1)
    }.padding(16).width('100%').height('100%').backgroundColor($r('sys.color.background_primary'))
  }
}
