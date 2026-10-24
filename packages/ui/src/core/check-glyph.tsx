export interface CheckGlyphProps {
  /** 固定尺寸；不传时由父级 CSS 控制 SVG 尺寸。 */
  size?: number;
  /** 勾选线宽，默认保持皮肤编辑器的线宽。 */
  strokeWidth?: number;
  className?: string;
}

/** 装饰性的勾选图标；尺寸和线宽可按使用场景覆盖。 */
export function CheckGlyph({ size, strokeWidth = 3, className }: CheckGlyphProps) {
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
      <path d="m5 12.5 4.5 4.5L19 7.5" />
    </svg>
  );
}
