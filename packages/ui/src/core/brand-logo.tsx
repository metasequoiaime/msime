/** 水杉标志的框和笔触，位于应用图标图稿 116×132 的框内。 */
export const msimeFramePath = "M5.84314 5.8335H109.843V125.833H5.84314V5.8335Z";
export const msimeStrokePath =
  "M80.394 18.8335L34.3451 36.489L80.394 49.7306L34.3451 71.7999C77.8789 79.1564 118.8 85.1887 31.8431 113.088";

/**
 * 水杉标志：填充的框上叠白色笔触，按设计稿 `contain` 蒙版的方式适配进 `size`×`size` 的正方形。框默认为设计稿的 `logoBg`，即压暗到 82% 的强调色，所以随平台和季节的强调色变化。与设计稿的标志一样播报为水杉输入法；已有可见名称说明它时，通过外层包装传入 `aria-hidden`。
 */
export function MsimeMark({
  size,
  frameFill = "color-mix(in srgb, var(--accent-color) 82%, #000)",
  strokeColor = "#FFFFFF",
  strokeWidth = 9,
  className,
}: {
  size: number;
  frameFill?: string;
  strokeColor?: string;
  strokeWidth?: number;
  className?: string;
}) {
  return (
    <svg
      viewBox="0 0 116 132"
      width={size}
      height={size}
      role="img"
      aria-label="水杉输入法"
      className={className}
    >
      <path d={msimeFramePath} style={{ fill: frameFill }} />
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
