import common from '@ohos.app.ability.common';
import { CustomShuangpinSettings, CustomShuangpinBridge, ShuangpinField, ShuangpinKey }
  from '../keyboard/settings/CustomShuangpinSettings';
import { ShuangpinFiles } from '../keyboard/settings/ShuangpinFiles';
import { AppEdition } from '../keyboard/AppEdition';

@Component
export struct CustomShuangpinPanel {
  bridge: CustomShuangpinBridge = {
    load: (): string => '', save: (_revision: number, _document: string): string => '',
    plan: (_request: string): string => ''
  };
  back: () => void = (): void => {};
  @Prop @Watch('reload') refresh: number = 0;
  @Prop @Watch('askBack') backRequest: number = 0;
  private model: CustomShuangpinSettings | undefined = undefined;
  @State private fields: ShuangpinField[] = [];
  @State private keys: ShuangpinKey[] = [];
  @State private ready: boolean = false;
  @State private busy: boolean = false;
  @State private saved: boolean = false;
  @State private dirty: boolean = false;
  @State private active: boolean = false;
  @State private hints: boolean = false;
  @State private problem: string = '';
  @State private message: string = '';
  @State private templateIndex: number = 0;
  private profiles: string[] = ['xiaohe', 'ziranma', 'microsoft', 'shoudao', 'sogou', 'jiajia', 'ziguang', 'smart_abc', 'guobiao'];

  aboutToAppear(): void {
    this.model = new CustomShuangpinSettings(this.bridge, AppEdition.current().id);
    this.reload();
  }

  private update(): void {
    if (this.model === undefined) return;
    this.fields = this.model.fields('initials').concat(this.model.fields('finals'), this.model.fields('zero_initials'));
    this.keys = this.model.keys;
    this.ready = this.model.ready;
    this.saved = this.model.saved;
    this.dirty = this.model.dirty;
    this.active = this.model.active;
    this.hints = this.model.hints;
    this.problem = this.model.problem;
    this.message = this.model.message;
  }

  private reload(): void {
    if (this.busy || this.model === undefined) return;
    this.model.load(); this.update();
  }

  private askBack(): void {
    if (this.busy) return;
    if (this.model === undefined || !this.model.dirty) { this.back(); return; }
    this.getUIContext().showAlertDialog({ title: '放弃未保存的修改？',
      primaryButton: { value: '继续编辑', action: (): void => {} },
      secondaryButton: { value: '放弃', action: (): void => this.back() }
    });
  }

  private chooseTemplate(index: number): void {
    const apply = (): void => {
      this.templateIndex = index;
      this.model?.template(this.profiles[index]); this.update();
    };
    if (this.model?.dirty !== true) { apply(); return; }
    this.getUIContext().showAlertDialog({ title: '用模板替换当前草稿？',
      primaryButton: { value: '取消', action: (): void => {} },
      secondaryButton: { value: '替换', action: apply }
    });
  }

  private requestImport(): void {
    if (this.busy) return;
    if (this.model?.dirty !== true) { this.importFile(); return; }
    this.getUIContext().showAlertDialog({ title: '导入并替换当前草稿？',
      primaryButton: { value: '取消', action: (): void => {} },
      secondaryButton: { value: '继续', action: (): void => { this.importFile(); } }
    });
  }

  private async importFile(): Promise<void> {
    if (this.busy || this.model === undefined) return;
    this.busy = true;
    try {
      const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      const text: string | undefined = await ShuangpinFiles.read(context);
      if (text !== undefined) this.model.importText(text);
      this.update();
    } catch (error) { this.message = (error as Error).message || '读取文件失败，请重新选择'; }
    finally { this.busy = false; }
  }

  private async exportFile(): Promise<void> {
    if (this.busy || this.model === undefined) return;
    const text: string | undefined = this.model.exportText();
    this.update();
    if (text === undefined) return;
    this.busy = true;
    try {
      const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      if (await ShuangpinFiles.write(context, text)) this.message = '已导出';
    } catch (error) { this.message = (error as Error).message || '写入文件失败，请重试'; }
    finally { this.busy = false; }
  }

  private remove(): void {
    this.getUIContext().showAlertDialog({ title: '删除自定义双拼方案？',
      message: '删除已保存的自定义表和当前草稿；正在使用时回退小鹤。内置方案、九键双拼表和个人词库会保留。',
      primaryButton: { value: '取消', action: (): void => {} },
      secondaryButton: { value: '删除', action: (): void => { this.model?.remove(); this.update(); } }
    });
  }

  @Builder
  help(title: string, message: string) {
    Button('?').type(ButtonType.Circle).width(28).height(28).accessibilityText(`${title}的说明`)
      .onClick((): void => {
        this.getUIContext().showAlertDialog({ title: title, message: message,
          confirm: { value: '好', action: (): void => {} } });
      })
  }

  @Builder
  part(part: string, title: string) {
    Column({ space: 8 }) {
      Text(title).fontSize(18).fontWeight(FontWeight.Medium).width('100%')
      ForEach(this.fields.filter((field: ShuangpinField): boolean => field.part === part), (field: ShuangpinField) => {
        Row({ space: 12 }) {
          Text(field.unit).fontSize(16).width(60)
          TextInput({ text: field.key }).maxLength(8).enabled(this.ready && !this.busy)
            .accessibilityText(`${title} ${field.unit} 的按键`)
            .onChange((value: string): void => { this.model?.edit(field, value); this.update(); })
        }.width('100%')
      }, (field: ShuangpinField): string => `${field.part}/${field.unit}`)
    }.width('100%')
  }

  build() {
    Column({ space: 16 }) {
      Row({ space: 12 }) {
        Button('返回').enabled(!this.busy).onClick((): void => this.askBack())
        Text('自定义双拼').fontSize(22).fontWeight(FontWeight.Bold).layoutWeight(1)
        this.help('自定义双拼', '导入不超过 16 KB 的 JSON 码表，或选择内置模板后修改。保存并使用会切换到普通双拼入口；未保存的草稿不影响键盘。')
      }.width('100%')
      Scroll() {
        Column({ space: 20 }) {
          if (this.dirty) { Text('尚未保存').fontColor($r('sys.color.font_secondary')).width('100%') }
          else if (this.active) { Text('正在使用自定义方案').fontColor($r('sys.color.font_secondary')).width('100%') }
          Row({ space: 8 }) {
            Text('键位提示').fontSize(16).layoutWeight(1)
            this.help('键位提示', '在 26 键双拼字母键底部显示对应的声母或韵母。')
            Toggle({ type: ToggleType.Switch, isOn: this.hints }).enabled(this.ready && !this.busy)
              .onChange((value: boolean): void => { this.model?.setHints(value); this.update(); })
          }.width('100%')
          Select([{ value: '小鹤模板' }, { value: '自然码模板' }, { value: '微软模板' }, { value: '首道模板' },
            { value: '搜狗模板' }, { value: '加加模板' }, { value: '紫光模板' }, { value: '智能 ABC 模板' }, { value: '国标模板' }])
            .selected(this.templateIndex).enabled(this.ready && !this.busy)
            .onSelect((index: number): void => this.chooseTemplate(index))
          Row({ space: 12 }) {
            Button('导入…').onClick((): void => this.requestImport())
            Button('导出…').enabled(this.problem.length === 0).onClick((): void => { this.exportFile(); })
            Button('删除').enabled(this.saved).onClick((): void => this.remove())
          }.enabled(this.ready && !this.busy).width('100%')
          if (this.problem.length > 0) { Text(this.problem).fontColor($r('sys.color.font_secondary')).width('100%') }
          if (this.message.length > 0) { Text(this.message).fontColor($r('sys.color.font_secondary')).width('100%') }
          if (!this.ready) { Button('重试').onClick((): void => this.reload()) }
          this.part('initials', '声母')
          this.part('finals', '韵母')
          this.part('zero_initials', '零声母音节')
          if (this.keys.length > 0) {
            Text('按键提示').fontSize(18).fontWeight(FontWeight.Medium).width('100%')
            ForEach(this.keys.filter((key: ShuangpinKey): boolean => key.hint.length > 0), (key: ShuangpinKey) => {
              Text(`${key.key.toUpperCase()}  ${key.hint}`).fontSize(14).width('100%')
            }, (key: ShuangpinKey): string => key.key)
          }
        }.width('100%')
      }.layoutWeight(1)
      Button('保存并使用').enabled(this.ready && !this.busy && this.problem.length === 0)
        .onClick((): void => { this.model?.use(); this.update(); })
    }.padding(20).width('100%').height('100%').backgroundColor($r('app.color.start_window_background'))
  }
}
