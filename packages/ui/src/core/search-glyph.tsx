export interface SearchGlyphProps {
  /** 固定尺寸；不传时由父级 CSS 控制 SVG 尺寸。 */
  size?: number;
  /** 搜索线宽，默认保持设置搜索框的线宽。 */
  strokeWidth?: number;
  className?: string;
}

/** 装饰性的搜索图标；尺寸和线宽可按使用场景覆盖。 */
export function SearchGlyph({ size, strokeWidth = 1.8, className }: SearchGlyphProps) {
  return (
    <svg
      {...(size === undefined ? {} : { width: size, height: size })}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      aria-hidden="true"
      focusable="false"
      className={className}
    >
      <circle cx="7" cy="7" r="5" />
      <path d="m11 11 3.5 3.5" />
    </svg>
  );
}
