import { FourteenKeyLetterWidthCell } from '../input/FourteenKeyLetterWidths';

/** 宽窄按键距分配份额，键帽两侧各扣半个键距；占位格不再额外消耗 Row 间距。 */
@Component
export struct LetterWidthRow {
  @Require @Prop cells: FourteenKeyLetterWidthCell[];
  @Prop varied: boolean = false;
  @Prop spacing: number = 0;
  @Prop rowHeight: number = 48;
  @Prop identity: string = '';
  @Require @BuilderParam content: (cell: FourteenKeyLetterWidthCell, index: number, share: number) => void;

  build() {
    Row({ space: this.varied ? 0 : this.spacing }) {
      ForEach(this.cells, (cell: FourteenKeyLetterWidthCell, index: number) => {
        if (this.varied) {
          Row() {
            this.content(cell, index, 1)
          }
          .layoutWeight(cell.weight)
          .height('100%')
          .padding({
            left: cell.kind === 'pad' ? 0 : this.spacing / 2,
            right: cell.kind === 'pad' ? 0 : this.spacing / 2
          })
        } else {
          this.content(cell, index, cell.weight)
        }
      }, (cell: FourteenKeyLetterWidthCell, index: number) =>
        `${index}:${cell.kind}:${cell.key?.letters ?? ''}:${cell.weight}:${this.identity}`)
    }
    // 多给一份键距再左右各移半份，让首末键帽与原字母区的两侧保持齐平。
    .width(this.varied ? `calc(100% + ${this.spacing}vp)` : '100%')
    .height(this.rowHeight)
    .offset({ x: this.varied ? -this.spacing / 2 : 0 })
  }
}
