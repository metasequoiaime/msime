/** 水杉标志的框和笔触，都是 assets/msime.svg 以 1.2 倍放进 `msimeViewBox` 里的 120×120 外框：框是圆角半径为边长 22% 的正方形，笔触是 msime.svg 那一笔，线宽 8 放大成 `msimeStrokeWidth`。外框左上角落在 (-2.1569, 5.8335)，所以 msime.svg 的 (x, y) 对应这里的 (-2.1569 + 1.2(x − 5), 5.8335 + 1.2(y − 5))。 */
export const msimeViewBox = "-8 0 132 132";
export const msimeFramePath =
  "M24.2431 5.8335L91.4431 5.8335C106.023 5.8335 117.843 17.6532 117.843 32.2335L117.843 99.4335C117.843 114.014 106.023 125.834 91.4431 125.834L24.2431 125.834C9.6628 125.834 -2.1569 114.014 -2.1569 99.4335L-2.1569 32.2335C-2.1569 17.6532 9.6628 5.8335 24.2431 5.8335Z";
export const msimeStrokePath =
  "M76.7775 26.4735L38.7872 41.0393L76.7775 51.9637L38.7872 70.1708C74.7026 76.2399 108.462 81.2165 36.7231 104.2335";
/** 标志的固定配色，与 assets/msime.svg 和各平台图标相同，不随主题或皮肤变化：深色底、绿边（宽度以 `msimeViewBox` 单位计，压在框的路径上居中）。 */
export const msimeFieldColor = "#252525";
export const msimeEdgeColor = "#A8DF8E";
export const msimeEdgeWidth = 6;
/** 笔触宽度（`msimeViewBox` 单位），即 msime.svg 的 8 放大 1.2 倍。笔触只用圆头、不设连接方式：与 msime.svg 一样用 SVG 默认的斜接（miter，斜接上限 4），三个折角都画成尖角。 */
export const msimeStrokeWidth = 9.6;

/**
 * 水杉标志：深色圆角正方形带绿边，上面叠白色笔触，适配进 `size`×`size` 的正方形；配色固定，与各平台的图标一致。与设计稿的标志一样播报为水杉输入法；已有可见名称说明它时，通过外层包装传入 `aria-hidden`。
 */
export function MsimeMark({
  size,
  strokeColor = "#FFFFFF",
  strokeWidth = msimeStrokeWidth,
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
      />
    </svg>
  );
}
