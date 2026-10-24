export type PlusGlyphVariant = "compact" | "row";

export interface PlusGlyphProps {
  /** 保留调用点原有的图标尺寸。 */
  size?: number;
  /** 加号线宽，默认使用词库行的线宽。 */
  strokeWidth?: number;
  /** `compact` 用于 16×16 词库行，`row` 用于 24×24 语言行。 */
  variant?: PlusGlyphVariant;
  className?: string;
}

const plusGlyphGeometry = {
  compact: { viewBox: "0 0 16 16", path: "M8 2.5v11M2.5 8h11" },
  row: { viewBox: "0 0 24 24", path: "M12 5v14M5 12h14" },
} as const;

/** 装饰性的加号图标；两个变体保留词库行和设置行的既有几何。 */
export function PlusGlyph({
  size = 16,
  strokeWidth = 1.6,
  variant = "compact",
  className,
}: PlusGlyphProps) {
  const geometry = plusGlyphGeometry[variant];
  return (
    <svg
      width={size}
      height={size}
      viewBox={geometry.viewBox}
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      aria-hidden="true"
      focusable="false"
      className={className}
    >
      <path d={geometry.path} />
    </svg>
  );
}
