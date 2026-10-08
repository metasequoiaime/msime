/** 常用语还在读取；此时什么都不画，读得快时就不会闪出提示。 */
export const PHRASE_LIST_LOADING: string = 'loading';
/** 常用语已读完；列表为空时会如实说明。 */
export const PHRASE_LIST_READY: string = 'ready';
/** 读取常用语失败。 */
export const PHRASE_LIST_ERROR: string = 'error';

const ROW_FONT_SIZE: number = 15;
const ROW_PADDING_VERTICAL_VP: number = 12;
const ROW_PADDING_HORIZONTAL_VP: number = 8;
const ROW_DIVIDER_VP: number = 1;
const MESSAGE_FONT_SIZE: number = 14;

/**
 * 顶替按键的「常用语」面板：用户的常用语，一行一条，点按即插入。
 *
 * 按设计的常用语面板排布：列表上方 6vp，两侧和底部 2vp；行的上下内边距 12vp、左右 8vp，15fp、`foreground` 色、单行省略，每行底边一条 1vp 的 `hair` 线。设计用内嵌阴影（inset box-shadow）画这条线，所以它横贯整行而不缩进。
 *
 * `state` 取 `PHRASE_LIST_LOADING`、`PHRASE_LIST_READY`、`PHRASE_LIST_ERROR` 之一。就绪但没有常用语时显示「还没有常用语」，出错时显示「常用语读取失败」，都是 14fp、`secondary` 色。点按一行会以它的文字调用 `onPick`；插入文字和关闭面板由调用方负责。
 */
@Component
export struct PhraseList {
  @Require @Prop phrases: string[];
  @Require @Prop state: string;
  @Require @Prop foreground: string;
  @Require @Prop secondary: string;
  @Require @Prop hair: string;
  @Require onPick: (text: string) => void;

  @Builder
  message(text: string) {
    Column() {
      Text(text)
        .fontSize(MESSAGE_FONT_SIZE)
        .fontColor(this.secondary)
        .textAlign(TextAlign.Center)
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

  build() {
    Column() {
      if (this.state === PHRASE_LIST_ERROR) {
        this.message('常用语读取失败')
      } else if (this.state === PHRASE_LIST_READY && this.phrases.length === 0) {
        this.message('还没有常用语')
      } else if (this.state === PHRASE_LIST_READY) {
        List() {
          ForEach(this.phrases, (phrase: string) => {
            ListItem() {
              Text(phrase)
                .width('100%')
                .padding({
                  top: ROW_PADDING_VERTICAL_VP,
                  bottom: ROW_PADDING_VERTICAL_VP,
                  left: ROW_PADDING_HORIZONTAL_VP,
                  right: ROW_PADDING_HORIZONTAL_VP
                })
                .fontSize(ROW_FONT_SIZE)
                .fontColor(this.foreground)
                .maxLines(1)
                .textOverflow({ overflow: TextOverflow.Ellipsis })
                .borderWidth({ bottom: ROW_DIVIDER_VP })
                .borderColor(this.hair)
                .stateStyles({
                  normal: {
                    .opacity(1)
                  },
                  pressed: {
                    .opacity(0.55)
                  }
                })
                .onClick(() => this.onPick(phrase))
            }
          }, (phrase: string, index: number): string => `${index}:${phrase}`)
        }
        .width('100%')
        .height('100%')
        .scrollBar(BarState.Off)
        .edgeEffect(EdgeEffect.None)
      }
    }
    .width('100%')
    .height('100%')
    .padding({ top: 6, left: 2, right: 2, bottom: 2 })
  }
}
