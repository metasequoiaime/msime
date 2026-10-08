const DOT_VP: number = 6;
const ACTIVE_DOT_VP: number = 16;
const DOT_GAP_VP: number = 6;
const TRANSITION_MS: number = 200;

/**
 * 分页键盘面板下方的页码点，移植自 Android 的 `KeyboardPagerDots`：高 6vp、间隔 6vp，当前页为 16vp 的 `accent` 胶囊，其他页为 `hair` 色的 6vp 圆点，换页时宽度和颜色以 200ms 过渡。
 *
 * 只有一页时不绘制。圆点不可点按，并对无障碍隐藏：它们描述的 `Swiper` 自己已播报位置并提供滚动操作。
 */
@Component
export struct PagerDots {
  @Require @Prop count: number;
  @Require @Prop page: number;
  @Require @Prop accent: string;
  @Require @Prop hair: string;

  private pages(): number[] {
    const pages: number[] = [];
    for (let index: number = 0; index < this.count; index++) {
      pages.push(index);
    }
    return pages;
  }

  build() {
    if (this.count > 1) {
      Row({ space: DOT_GAP_VP }) {
        // 按位置作键，让每个圆点在换页时保留自己的节点，宽度和颜色才能产生动画而不是被替换。
        ForEach(this.pages(), (index: number) => {
          Column()
            .width(index === this.page ? ACTIVE_DOT_VP : DOT_VP)
            .height(DOT_VP)
            .borderRadius(DOT_VP / 2)
            .backgroundColor(index === this.page ? this.accent : this.hair)
            .animation({ duration: TRANSITION_MS })
        }, (index: number): string => index.toString())
      }
      .justifyContent(FlexAlign.Center)
      .accessibilityLevel('no-hide-descendants')
    }
  }
}
