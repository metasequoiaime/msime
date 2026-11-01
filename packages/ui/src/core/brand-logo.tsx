/** 水杉标志的框和笔触。笔触是设计稿 116×132 画布里那一笔，以外框中心为原点缩到 80%、线宽不变，四周留出边距；框是围着它居中的 120×120 圆角正方形（圆角半径为边长的 22%，与 assets/msime.svg 相同），所以画布向左右各扩 8，成为 `msimeViewBox` 这个正方形。 */
export const msimeViewBox = "-8 0 132 132";
export const msimeFramePath =
  "M24.2431 5.8335L91.4431 5.8335C106.023 5.8335 117.843 17.6532 117.843 32.2335L117.843 99.4335C117.843 114.014 106.023 125.834 91.4431 125.834L24.2431 125.834C9.6628 125.834 -2.1569 114.014 -2.1569 99.4335L-2.1569 32.2335C-2.1569 17.6532 9.6628 5.8335 24.2431 5.8335Z";
export const msimeStrokePath =
  "M75.8838 28.2335L39.0447 42.3579L75.8838 52.9512L39.0447 70.6066C73.8717 76.4918 106.609 81.3177 37.0431 103.637";
/** 标志的固定配色，与 assets/msime.svg 和各平台图标相同，不随主题或皮肤变化：深色底、绿边（宽度以 `msimeViewBox` 单位计，压在框的路径上居中）。 */
export const msimeFieldColor = "#252525";
export const msimeEdgeColor = "#A8DF8E";
export const msimeEdgeWidth = 6;

/**
 * 水杉标志：深色圆角正方形带绿边，上面叠白色笔触，适配进 `size`×`size` 的正方形；配色固定，与各平台的图标一致。与设计稿的标志一样播报为水杉输入法；已有可见名称说明它时，通过外层包装传入 `aria-hidden`。
 */
export function MsimeMark({
  size,
  strokeColor = "#FFFFFF",
  strokeWidth = 9,
  className,
}: {
  size: number;
  strokeColor?: string;
  strokeWidth?: number;
  className?: string;
}) {
  return (
    <svg
      viewBox={msimeViewBox}
      width={size}
      height={size}
      role="img"
      aria-label="水杉输入法"
      className={className}
    >
      <path
        d={msimeFramePath}
        fill={msimeFieldColor}
        stroke={msimeEdgeColor}
        strokeWidth={msimeEdgeWidth}
      />
      <path
        d={msimeStrokePath}
        fill="none"
        stroke={strokeColor}
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
