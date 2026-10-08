import { PagerDots } from './PagerDots';

const COLUMNS: number = 4;
const SIDE_PADDING_VP: number = 4;
const BOTTOM_PADDING_VP: number = 4;
const DOTS_GAP_VP: number = 8;

/**
 * 横向分页的键盘功能块网格，下方带页码圆点，移植自 Android 的 `PagedTileGrid` 和 `FunctionPanelView` 的面板外壳。
 *
 * 功能块每行四个、每页 `pageSize` 个，按顺序排列：第 `index` 个功能块位于第 `floor(index / pageSize)` 页。翻页会吸附、不循环，边缘没有回弹。面板在网格上方留 `topPadding`（未设置时为 10vp），在圆点下方留 4vp；圆点位于网格下方 8vp，全部内容一页能放下时不显示。
 *
 * 功能块由调用方负责：`tile` 是绘制第 `index` 个功能块的 builder。请以箭头函数传入，使其中的 `this` 仍指向调用方，功能块才能继续响应调用方的状态，例如 `tile: (index: number): void => { this.functionTile(index) }`。改变 `resetToken` 会回到第一页，常驻挂载的面板就是靠它在每次重新打开时回到第一页。
 *
 * 三个键盘面板这样使用它：
 * - 功能面板（功能）：`rowHeight` 52，使用默认间距（行间 16、列间 4），每格一个 `PanelTile`。
 * - 方案选择（输入方式）：与功能面板相同，每格是带 9/26 下标的 `PanelTile` 字形框。
 * - 皮肤选择（皮肤）：每格一个 `MiniKeyboardPreview`，`rowHeight` 约 92，`topPadding` 6，`rowsGap` 4，`columnsGap` 6。设计稿的间距是行间 6、缩略图之间 10，但每个预览在缩略图上方和两侧各留了 2vp 给选中环，所以网格的行间距小 2（只有下方的缩略图有这段边距），列间距小 4。
 */
@Component
export struct PagedTiles {
  @Require @Prop @Watch('clampPage') count: number;
  @Prop @Watch('clampPage') pageSize: number = 8;
  @Prop rowHeight: number = 52;
  @Prop rowsGap: number = 16;
  @Prop columnsGap: number = 4;
  @Prop topPadding: number = 10;
  @Prop @Watch('resetPage') resetToken: number = 0;
  @Require @Prop accent: string;
  @Require @Prop hair: string;
  @Require @BuilderParam tile: (index: number) => void;
  /** 翻页或重置后停稳时，以停下的页码调用。 */
  onPageChange: (page: number) => void = (_page: number): void => {};
  @State private page: number = 0;

  private perPage(): number {
    return Math.max(1, Math.floor(this.pageSize));
  }

  private rows(): number {
    return Math.ceil(this.perPage() / COLUMNS);
  }

  private pageCount(): number {
    return this.count <= 0 ? 1 : Math.ceil(this.count / this.perPage());
  }

  private pages(): number[] {
    const pages: number[] = [];
    for (let page: number = 0; page < this.pageCount(); page++) {
      pages.push(page);
    }
    return pages;
  }

  private tilesOn(page: number): number[] {
    const tiles: number[] = [];
    const end: number = Math.min(this.count, (page + 1) * this.perPage());
    for (let index: number = page * this.perPage(); index < end; index++) {
      tiles.push(index);
    }
    return tiles;
  }

  private rowsTemplate(): string {
    const rows: string[] = [];
    for (let row: number = 0; row < this.rows(); row++) {
      rows.push('1fr');
    }
    return rows.join(' ');
  }

  private gridHeight(): number {
    return this.rows() * this.rowHeight + (this.rows() - 1) * this.rowsGap;
  }

  private showPage(page: number): void {
    if (page !== this.page) {
      this.page = page;
      this.onPageChange(page);
    }
  }

  private resetPage(): void {
    this.showPage(0);
  }

  private clampPage(): void {
    this.showPage(Math.min(this.page, this.pageCount() - 1));
  }

  build() {
    Column() {
      Swiper() {
        ForEach(this.pages(), (page: number) => {
          Grid() {
            ForEach(this.tilesOn(page), (index: number) => {
              GridItem() {
                this.tile(index)
              }
            }, (index: number): string => index.toString())
          }
          .columnsTemplate('1fr 1fr 1fr 1fr')
          .rowsTemplate(this.rowsTemplate())
          .rowsGap(this.rowsGap)
          .columnsGap(this.columnsGap)
          .padding({ left: SIDE_PADDING_VP, right: SIDE_PADDING_VP })
          .width('100%')
          .height(this.gridHeight())
        }, (page: number): string => page.toString())
      }
      .index(this.page)
      .loop(false)
      .indicator(false)
      .effectMode(EdgeEffect.None)
      .disableSwipe(this.pageCount() <= 1)
      .width('100%')
      .height(this.gridHeight())
      .onChange((page: number): void => this.showPage(page))

      if (this.pageCount() > 1) {
        PagerDots({ count: this.pageCount(), page: this.page, accent: this.accent, hair: this.hair })
          .margin({ top: DOTS_GAP_VP })
      }
    }
    .width('100%')
    .padding({ top: this.topPadding, bottom: BOTTOM_PADDING_VP })
  }
}
