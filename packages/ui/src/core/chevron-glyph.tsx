export interface ChevronGlyphProps {
  /** 固定尺寸；调用方应按所在控件传入原有尺寸。 */
  size?: number;
  /** 线宽，默认使用设置控件的线宽。 */
  strokeWidth?: number;
  className?: string;
}

/** 装饰性的右向 chevron 图标。 */
export function ChevronGlyph({ size, strokeWidth = 2.4, className }: ChevronGlyphProps) {
  return (
    <svg
      {...(size === undefined ? {} : { width: size, height: size })}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      className={className}
    >
      <path d="m9 6 6 6-6 6" />
    </svg>
  );
}
