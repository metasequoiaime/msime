@Entry
@Component
struct CanvasProbeEditor {
  @State private text: string = '';
  private controller: TextAreaController = new TextAreaController();

  build() {
    Column({ space: 16 }) {
      Text('候选皮肤设备验收').fontSize(24)
      Text('只输入合成文字：nihao → 你好').fontSize(16)
      TextArea({ text: this.text, placeholder: '点此唤起水杉键盘', controller: this.controller })
        .id('canvasProbeEditor').height(200).fontSize(24)
        .onChange((text: string): void => { this.text = text; })
      Text(this.text).id('canvasProbeCommit').fontSize(24)
      Button('重新聚焦').onClick((): void => { focusControl.requestFocus('canvasProbeEditor'); })
    }.width('100%').padding(24)
  }
}
