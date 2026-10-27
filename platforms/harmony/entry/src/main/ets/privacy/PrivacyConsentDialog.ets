import { ConsentDocument, PrivacyConsentPolicy } from './PrivacyConsentPolicy';

/**
 * 首次启动的隐私政策弹窗：几句话说明处理什么、为什么，可以点开的《隐私政策》，以及「不同意」「同意」两个同样醒目的按钮。文案和协议列表在 `PrivacyConsentPolicy`，由逻辑测试核对。
 *
 * 弹窗只报告用户选了什么，写记录、通知键盘和页面都由设置页面（`Settings.ets`）处理。
 */
@CustomDialog
export struct PrivacyConsentDialog {
  controller?: CustomDialogController;
  /** 当前版本是否已经同意；从「隐私」页重新打开时为真，多显示一句撤回的说明。 */
  @Link granted: boolean;
  onDecide: (granted: boolean) => void = (granted: boolean): void => {};
  onOpenDocument: (url: string) => void = (url: string): void => {};

  private decide(granted: boolean): void {
    this.controller?.close();
    this.onDecide(granted);
  }

  build() {
    Column({ space: 16 }) {
      Text(PrivacyConsentPolicy.COPY.title)
        .fontSize(20)
        .fontWeight(FontWeight.Medium)
        .fontColor($r('sys.color.ohos_id_color_text_primary'))
        .width('100%')
        .textAlign(TextAlign.Center)
      Scroll() {
        Column({ space: 10 }) {
          Text(PrivacyConsentPolicy.COPY.intro)
            .fontSize(15)
            .fontColor($r('sys.color.ohos_id_color_text_primary'))
            .width('100%')
          ForEach(PrivacyConsentPolicy.COPY.points, (point: string) => {
            Row({ space: 6 }) {
              Text('·')
                .fontSize(15)
                .fontColor($r('sys.color.ohos_id_color_text_secondary'))
              Text(point)
                .fontSize(14)
                .fontColor($r('sys.color.ohos_id_color_text_secondary'))
                .layoutWeight(1)
            }
            .width('100%')
            .alignItems(VerticalAlign.Top)
          }, (point: string): string => point)
          Flex({ wrap: FlexWrap.Wrap, alignItems: ItemAlign.Center }) {
            Text(PrivacyConsentPolicy.COPY.linkPrefix)
              .fontSize(14)
              .fontColor($r('sys.color.ohos_id_color_text_secondary'))
            ForEach(PrivacyConsentPolicy.DOCUMENTS, (document: ConsentDocument, index: number) => {
              if (index > 0) {
                Text('和')
                  .fontSize(14)
                  .fontColor($r('sys.color.ohos_id_color_text_secondary'))
              }
              Text(document.title)
                .fontSize(14)
                .fontColor($r('sys.color.ohos_id_color_emphasize'))
                .accessibilityText(`查看${document.title}`)
                .onClick((): void => this.onOpenDocument(document.url))
            }, (document: ConsentDocument): string => document.url)
          }
          .width('100%')
          Text(this.granted ? PrivacyConsentPolicy.COPY.grantedNote : PrivacyConsentPolicy.COPY.refusalNote)
            .fontSize(13)
            .fontColor($r('sys.color.ohos_id_color_text_tertiary'))
            .width('100%')
        }
        .width('100%')
      }
      .constraintSize({ maxHeight: 360 })
      .scrollBar(BarState.Auto)
      Row({ space: 12 }) {
        Button(PrivacyConsentPolicy.COPY.refuse, { type: ButtonType.Capsule, buttonStyle: ButtonStyleMode.NORMAL })
          .layoutWeight(1)
          .height(40)
          .onClick((): void => this.decide(false))
        Button(PrivacyConsentPolicy.COPY.agree, { type: ButtonType.Capsule, buttonStyle: ButtonStyleMode.EMPHASIZED })
          .layoutWeight(1)
          .height(40)
          .onClick((): void => this.decide(true))
      }
      .width('100%')
    }
    .padding({ left: 24, right: 24, top: 24, bottom: 20 })
  }
}
