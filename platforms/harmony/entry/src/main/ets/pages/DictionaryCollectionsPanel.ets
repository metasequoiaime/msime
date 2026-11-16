import common from '@ohos.app.ability.common';
import picker from '@ohos.file.picker';
import fs from '@ohos.file.fs';
import util from '@ohos.util';

interface CollectionSource { type: string; format?: string; }
interface CollectionItem {
  id: string;
  name: string;
  kind: string;
  source: CollectionSource;
  enabled: boolean;
  entry_count: number;
  pending: number;
}
interface ImportReport { imported: number; duplicates: number; failed: number; truncated: boolean; }
interface CollectionsDocument { collections: CollectionItem[]; outbox_pending: number; import?: ImportReport; }
interface CollectionsReply { ok: boolean; value?: CollectionsDocument; error?: string; }
interface CollectionAction {
  operation: string;
  id?: string;
  ids?: string[];
  name?: string;
  kind?: string;
  enabled?: boolean;
  format?: string;
  text?: string;
  bytes_base64?: string;
}

/** 原生分组管理只适配共享集合协议，沿用设置应用的队列与键盘词条交换。 */
@Component
export struct DictionaryCollectionsPanel {
  request: (action: string) => Promise<string> = async (_action: string): Promise<string> => '';
  changed: () => void = (): void => {};
  back: () => void = (): void => {};
  @State private items: CollectionItem[] = [];
  @State private selected: string[] = [];
  @State private name: string = '';
  @State private kindIndex: number = 0;
  @State private formatIndex: number = 0;
  @State private targetIndex: number = 0;
  @State private busy: boolean = false;
  @State private message: string = '';
  private kinds: string[] = ['pinyin', 'wubi', 'wubi98', 'wubixsj', 'english', 'quickPhrase', 'cantonese'];
  private formats: string[] = ['standard', 'windows', 'rime', 'hans', 'scel'];

  aboutToAppear(): void { this.perform({ operation: 'load' }); }

  private async perform(action: CollectionAction): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.message = '正在处理…';
    try {
      const reply: CollectionsReply = JSON.parse(await this.request(JSON.stringify(action))) as CollectionsReply;
      if (reply.ok !== true || reply.value === undefined) {
        this.message = reply.error === 'collections_source_conflict' ? '此分组不能接收该来源，请新建分组导入'
          : reply.error === 'collections_name_invalid' ? '分组名称须为 1–32 个字'
          : reply.error === 'collections_corrupt' ? '分组数据损坏，原文件未改动' : '词库操作失败，请重试';
        return;
      }
      this.items = reply.value.collections;
      this.selected = this.selected.filter((id: string): boolean => this.items.some((item: CollectionItem): boolean => item.id === id));
      const report: ImportReport | undefined = reply.value.import;
      this.message = report === undefined ? '' : `导入 ${report.imported} 条，跳过重复 ${report.duplicates} 条，失败 ${report.failed} 条${report.truncated ? '；分组已满' : ''}`;
      if (action.operation !== 'load') this.changed();
    } catch {
      this.message = '词库操作失败，请重试';
    } finally { this.busy = false; }
  }

  private choose(id: string, checked: boolean): void {
    this.selected = checked ? this.selected.concat([id]) : this.selected.filter((value: string): boolean => value !== id);
  }
  private targets(): CollectionItem[] {
    return this.items.filter((item: CollectionItem): boolean => item.kind === this.kinds[this.kindIndex] && item.source.type !== 'community');
  }
  private targetOptions(): SelectOption[] {
    const values: SelectOption[] = [{ value: '新建分组' }];
    this.targets().forEach((item: CollectionItem): void => { values.push({ value: item.name }); });
    return values;
  }
  private async importFile(): Promise<void> {
    if (this.busy) return;
    try {
      const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      const options = new picker.DocumentSelectOptions(); options.maxSelectNumber = 1;
      const chosen: string[] = await new picker.DocumentViewPicker(context).select(options);
      if (chosen.length === 0) return;
      const format: string = this.formats[this.formatIndex];
      const maximum: number = format === 'scel' ? 8 * 1024 * 1024 : 16 * 1024 * 1024;
      const file: fs.File = fs.openSync(chosen[0], fs.OpenMode.READ_ONLY);
      let buffer: ArrayBuffer;
      try {
        const size: number = fs.statSync(file.fd).size;
        if (size <= 0 || size > maximum) { this.message = '文件为空或超出容量限制'; return; }
        buffer = new ArrayBuffer(size + 1);
        let count: number = 0;
        while (count < buffer.byteLength) {
          const chunk = new ArrayBuffer(buffer.byteLength - count);
          const read: number = fs.readSync(file.fd, chunk);
          if (read <= 0) break;
          new Uint8Array(buffer).set(new Uint8Array(chunk, 0, read), count); count += read;
        }
        if (count > size) { this.message = '文件在读取时发生变化，请重试'; return; }
        buffer = buffer.slice(0, count);
      } finally { fs.closeSync(file); }
      const fileName: string = decodeURIComponent(chosen[0].substring(chosen[0].lastIndexOf('/') + 1));
      const dot: number = fileName.lastIndexOf('.');
      const action: CollectionAction = { operation: 'import', kind: this.kinds[this.kindIndex], format: format,
        name: this.name.trim().length > 0 ? this.name.trim() : dot > 0 ? fileName.substring(0, dot) : fileName };
      const targets: CollectionItem[] = this.targets();
      if (this.targetIndex > 0 && this.targetIndex <= targets.length) action.id = targets[this.targetIndex - 1].id;
      if (format === 'scel') action.bytes_base64 = new util.Base64Helper().encodeToStringSync(new Uint8Array(buffer));
      else action.text = util.TextDecoder.create('utf-8', { fatal: true }).decodeWithStream(new Uint8Array(buffer));
      await this.perform(action);
    } catch { this.message = '读取文件失败，请重新选择'; }
  }
  private batch(enabled: boolean): void {
    if (this.selected.length === 0) { this.message = '请先选择词库'; return; }
    this.perform({ operation: 'set_enabled_many', ids: this.selected.slice(), enabled: enabled });
  }
  private deleteSelected(): void {
    if (this.selected.length === 0) { this.message = '请先选择词库'; return; }
    this.getUIContext().showAlertDialog({ title: '删除所选词库？',
      message: '删除分组及其词条；其他已启用分组和原有个人词条会保留。',
      primaryButton: { value: '取消', action: (): void => {} },
      secondaryButton: { value: '删除', action: (): void => { this.perform({ operation: 'delete_many', ids: this.selected.slice() }); } }
    });
  }

  @Builder
  sourceGroup(source: string, title: string) {
    if (this.items.some((item: CollectionItem): boolean => item.source.type === source)) {
      Column({ space: 8 }) {
        Text(title).fontSize(14).fontColor($r('sys.color.font_secondary')).width('100%')
        ForEach(this.items.filter((item: CollectionItem): boolean => item.source.type === source), (item: CollectionItem) => {
          Row({ space: 12 }) {
            Checkbox().select(this.selected.includes(item.id)).enabled(!this.busy)
              .onChange((checked: boolean): void => this.choose(item.id, checked))
              .accessibilityText(`选择「${item.name}」`)
            Column({ space: 4 }) {
              Text(item.name).fontSize(17).width('100%')
              Text(`${item.entry_count} 条 · ${item.enabled ? '已启用' : '已停用'}${item.pending > 0 ? ` · ${item.pending} 条待写入` : ''}`)
                .fontSize(13).fontColor($r('sys.color.font_secondary')).width('100%')
            }.layoutWeight(1)
          }.width('100%').padding(12).backgroundColor($r('sys.color.comp_background_primary')).borderRadius(12)
        }, (item: CollectionItem): string => item.id)
      }.width('100%').alignItems(HorizontalAlign.Start)
    }
  }

  build() {
    Column({ space: 12 }) {
      Row() {
        Button('返回').onClick((): void => this.back())
        Text('词库分组').fontSize(22).fontWeight(FontWeight.Bold).layoutWeight(1).textAlign(TextAlign.Center)
        Button('刷新').enabled(!this.busy).onClick((): void => { this.perform({ operation: 'load' }); })
      }.width('100%')
      Scroll() {
        Column({ space: 18 }) {
          this.sourceGroup('user', '自建词库')
          this.sourceGroup('import', '文件导入')
          this.sourceGroup('community', '社区词库')
          if (this.items.length === 0 && !this.busy) { Text('还没有分组').fontColor($r('sys.color.font_secondary')) }
          TextInput({ text: this.name, placeholder: '分组名称（导入时默认文件名）' }).enabled(!this.busy)
            .onChange((value: string): void => { this.name = value; })
          Select([{ value: '拼音' }, { value: '86 五笔' }, { value: '98 五笔' }, { value: '新世纪五笔' },
            { value: '英文' }, { value: '快捷短语' }, { value: '粤语' }]).selected(this.kindIndex).enabled(!this.busy)
            .onSelect((index: number): void => { this.kindIndex = index; this.targetIndex = 0; })
          Select([{ value: '词在前（TSV）' }, { value: '编码在前（TSV）' }, { value: 'Rime' },
            { value: '汉字词表' }, { value: '搜狗细胞词库' }]).selected(this.formatIndex).enabled(!this.busy)
            .onSelect((index: number): void => { this.formatIndex = index; })
          Select(this.targetOptions()).selected(this.targetIndex).enabled(!this.busy)
            .onSelect((index: number): void => { this.targetIndex = index; })
          Row({ space: 8 }) {
            Button('新建').onClick((): void => { this.perform({ operation: 'create', name: this.name, kind: this.kinds[this.kindIndex] }); })
            Button('改名').onClick((): void => {
              if (this.selected.length === 1) this.perform({ operation: 'rename', id: this.selected[0], name: this.name });
              else this.message = '改名时请选择一个词库';
            })
            Button('导入…').onClick((): void => { this.importFile(); })
          }.enabled(!this.busy)
          Row({ space: 8 }) {
            Button('启用所选').onClick((): void => this.batch(true))
            Button('停用所选').onClick((): void => this.batch(false))
            Button('删除所选').onClick((): void => this.deleteSelected())
          }.enabled(!this.busy)
          if (this.message.length > 0) { Text(this.message).fontSize(14).width('100%') }
        }.width('100%')
      }.layoutWeight(1)
    }.padding(16).width('100%').height('100%').backgroundColor($r('sys.color.background_primary'))
  }
}
