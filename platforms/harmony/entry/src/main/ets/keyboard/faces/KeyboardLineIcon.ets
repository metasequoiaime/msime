/**
 * 由 SVG 路径字符串绘制的单个线条图标，画法与设计稿绘制每个键盘图标相同：在 24 单位的 viewBox 上使用 `fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"`。
 *
 * 路径字符串放在 kb-core 的 `KeyboardIconPaths.ts` 里，由外部传入，本组件不知道自己画的是哪个图标。`stroke` 以 viewBox 单位计，与 SVG 一致：24 viewBox 上 1.7 的描边，按 24vp 绘制时宽 1.7vp，按 12vp 绘制时宽 0.85vp。
 *
 * ArkUI 的 `Path` 以 px 解释命令，而其他长度默认都是 vp。因此 viewport 和描边宽度也用 px 给出，让三者描述同一个坐标空间，再由 `Shape` 的 viewport 变换像 SVG viewBox 那样把这个空间映射到 `iconSize` vp。有两点必须在真机上确认，因为除了实际运行的 HAP，没有哪道门禁能覆盖它们：viewport 变换是否随几何图形一起缩放描边（设计稿和本组件都按 SVG 的行为假定会缩放），以及路径解析器是否接受 kb-core 提供的每一种线段（正因如此，它提供的都是不含弧线的字符串）。
 */
@Component
export struct KeyboardLineIcon {
  /** SVG 路径数据，以 viewBox 单位计。 */
  @Require @Prop d: string;
  /** 绘制边长，单位 vp。非正方形的 viewBox 按长边等于该值来适配。 */
  @Require @Prop iconSize: number;
  /** 描边宽度，以 viewBox 单位计。 */
  @Require @Prop stroke: number;
  @Require @Prop color: string;
  @Prop viewWidth: number = 24;
  @Prop viewHeight: number = 24;

  private drawnWidth(): number {
    return this.iconSize * this.viewWidth / Math.max(this.viewWidth, this.viewHeight);
  }

  private drawnHeight(): number {
    return this.iconSize * this.viewHeight / Math.max(this.viewWidth, this.viewHeight);
  }

  build() {
    Shape() {
      Path()
        .width(`${this.viewWidth}px`)
        .height(`${this.viewHeight}px`)
        .commands(this.d)
        .fill(Color.Transparent)
        .stroke(this.color)
        .strokeWidth(`${this.stroke}px`)
        .strokeLineCap(LineCapStyle.Round)
        .strokeLineJoin(LineJoinStyle.Round)
    }
    .viewPort({ x: 0, y: 0, width: `${this.viewWidth}px`, height: `${this.viewHeight}px` })
    .width(this.drawnWidth())
    .height(this.drawnHeight())
  }
}
