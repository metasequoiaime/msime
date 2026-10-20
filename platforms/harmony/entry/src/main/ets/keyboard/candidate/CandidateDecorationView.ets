import { DecorationCandidatePolicy, DecorationCandidateStyle, DecorationWordCandidate,
  DecorationWordFrame, DecorationWordSize } from './DecorationCandidatePolicy';
import { CandidateDecorationRect } from './CandidateDecorationLayout';

/** 图片与文字共用节点生命周期；父视图按图片身份重建，旧加载回调不会更新新皮肤。 */
@Component
export struct CandidateDecorationView {
  @Prop image: string = '';
  @Prop rect: CandidateDecorationRect = { x: 0, y: 0, width: 0, height: 0 };
  @Prop @Watch('refreshWord') boxWidth: number = 0;
  @Prop @Watch('refreshWord') boxHeight: number = 0;
  @Prop imageAlign: string = 'right';
  @Prop @Watch('refreshWord') fontFamily: string = '';
  @Prop @Watch('refreshWord') style: DecorationCandidateStyle | null = null;
  @Prop @Watch('refreshWord') candidates: DecorationWordCandidate[] = [];
  @State private imageReady: boolean = false;
  @State private wordFrame: DecorationWordFrame | null = null;

  /** 候选、高亮、字体或声明变化时重测，让绘制节点直接订阅新的文字与尺寸。 */
  private refreshWord(): void {
    this.wordFrame = this.imageReady ? this.frame() : null;
  }

  private frame(): DecorationWordFrame | null {
    return DecorationCandidatePolicy.frame(this.style, this.candidates, this.boxWidth, this.boxHeight,
      (text: string, fontSize: number): DecorationWordSize => {
        const size: SizeOptions = this.getUIContext().getMeasureUtils().measureTextSize({
          textContent: text, fontSize: `${fontSize}vp`, fontFamily: this.fontFamily, maxLines: 1
        });
        return { width: this.getUIContext().px2vp(Number(size.width)),
          height: this.getUIContext().px2vp(Number(size.height)) };
      });
  }

  build() {
    Stack({ alignContent: Alignment.TopStart }) {
      Image(this.image)
        .width(this.rect.width)
        .height(this.rect.height)
        .objectFit(ImageFit.Contain)
        .syncLoad(false)
        .hitTestBehavior(HitTestMode.None)
        .accessibilityLevel('no')
        .onComplete((): void => {
          this.imageReady = true;
          this.refreshWord();
        })
        .onError((): void => {
          this.imageReady = false;
          this.refreshWord();
        })
      if (this.wordFrame !== null) {
        Text(this.wordFrame!.text)
          .fontSize(`${this.wordFrame!.fontSize}vp`)
          .fontFamily(this.fontFamily)
          .fontWeight(FontWeight.Normal)
          .fontColor(this.wordFrame!.color)
          .maxLines(1)
          .width(Math.ceil(this.wordFrame!.width))
          .height(Math.ceil(this.wordFrame!.height))
          .position({
            x: (this.rect.width - this.boxWidth) *
              (this.imageAlign === 'left' ? 0 : this.imageAlign === 'center' ? 0.5 : 1) + this.wordFrame!.x,
            y: this.rect.height - this.boxHeight + this.wordFrame!.y
          })
          .clip(false)
          .hitTestBehavior(HitTestMode.None)
          .accessibilityLevel('no')
          .id('candidate-decoration-word')
      }
    }
    .position({ x: this.rect.x, y: this.rect.y })
    .width(this.rect.width)
    .height(this.rect.height)
    .clip(false)
    .hitTestBehavior(HitTestMode.None)
    .accessibilityLevel('no')
  }
}
