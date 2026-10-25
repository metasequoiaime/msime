export type ChevronGlyphVariant = "profile" | "sheet-back" | "subpage-back";

export interface ChevronGlyphProps {
  /** 固定尺寸；调用方应按所在控件传入原有尺寸。 */
  size?: number;
  /** 线宽，默认使用设置控件的线宽。 */
  strokeWidth?: number;
  /** `profile`、`sheet-back` 和 `subpage-back` 保留各调用点的原有几何。 */
  variant?: ChevronGlyphVariant;
  className?: string;
}

const chevronGlyphGeometry = {
  default: {
    viewBox: "0 0 24 24",
    path: "m9 6 6 6-6 6",
    width: undefined,
    height: undefined,
    strokeWidth: 2.4,
  },
  profile: {
    viewBox: "0 0 8 14",
    path: "m1 1 6 6-6 6",
    width: 7,
    height: 12,
    strokeWidth: 2,
  },
  "sheet-back": {
    viewBox: "0 0 24 24",
    path: "m15 6-6 6 6 6",
    width: 18,
    height: 18,
    strokeWidth: 2.6,
  },
  "subpage-back": {
    viewBox: "0 0 12 21",
    path: "M10 2 2 10.5 10 19",
    width: 12,
    height: 21,
    strokeWidth: 2.4,
  },
} as const;

/** 装饰性的 chevron 图标；变体保留各调用点的方向、尺寸和几何。 */
export function ChevronGlyph({ size, strokeWidth, variant, className }: ChevronGlyphProps) {
  const geometry = chevronGlyphGeometry[variant ?? "default"];
  const width = size ?? geometry.width;
  const height = size ?? geometry.height;
  return (
    <svg
      {...(width === undefined ? {} : { width, height })}
      viewBox={geometry.viewBox}
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth ?? geometry.strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      className={className}
    >
      <path d={geometry.path} />
    </svg>
  );
}
