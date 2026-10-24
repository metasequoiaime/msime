export interface CloseGlyphProps {
  /** 固定尺寸；不传时由父级 CSS 控制 SVG 尺寸。 */
  size?: number;
  /** 关闭线宽，默认保持两个设置按钮的线宽。 */
  strokeWidth?: number;
  className?: string;
}

/** 装饰性的关闭图标；尺寸和线宽可按使用场景覆盖。 */
export function CloseGlyph({ size, strokeWidth = 2.6, className }: CloseGlyphProps) {
  return (
    <svg
      {...(size === undefined ? {} : { width: size, height: size })}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      aria-hidden="true"
      focusable="false"
      className={className}
    >
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  );
}
