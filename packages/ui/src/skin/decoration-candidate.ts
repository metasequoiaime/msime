/** 装饰词的共享清单声明，原始颜色用于无损导出，规范化颜色供预览绘制。 */
export type DecorationCandidate = {
  index: number;
  fontSizeDip: number;
  minFontSizeDip: number;
  maxWidthDip: number;
  color: string;
  resolvedColor?: string;
  alignH: "left" | "center" | "right";
  alignV: "top" | "center" | "bottom";
  offsetXDip: number;
  offsetYDip: number;
  stripBadge: boolean;
};

/** 编辑器恢复旧草稿时可能没有共享层的规范化色值，按同一规则补齐。 */
function resolvedColor(value: string): string {
  const input = value.trim();
  if (input === "transparent") return "#00000000";
  let hex = input.replace(/^#/, "");
  if (/^(?:[a-f0-9]{3,4}|[a-f0-9]{6}|[a-f0-9]{8})$/i.test(hex)) {
    if (hex.length <= 4) hex = [...hex].map((digit) => digit + digit).join("");
    return `#${hex.toUpperCase()}${hex.length === 6 ? "FF" : ""}`;
  }
  if ((input.startsWith("rgb(") || input.startsWith("rgba(")) && input.includes(")")) {
    let remaining = input
      .slice(input.indexOf("(") + 1, input.lastIndexOf(")"))
      .replaceAll(",", " ");
    const take = () => {
      const match = /^\s*(-?(?:\d+\.?\d*|\.\d+)(?:[eE]-?\d+)?)/.exec(remaining);
      if (!match || !Number.isFinite(Number(match[1]))) return null;
      remaining = remaining.slice(match[0].length);
      return Number(match[1]);
    };
    const channels = [take(), take(), take()];
    if (channels.every((channel) => channel !== null)) {
      const alpha = take() ?? 1;
      const byte = (value: number, maximum: number) =>
        Math.round((Math.min(maximum, Math.max(0, value)) * 255) / maximum)
          .toString(16)
          .padStart(2, "0")
          .toUpperCase();
      return `#${channels.map((channel) => byte(channel!, 255)).join("")}${byte(alpha, 1)}`;
    }
  }
  return "#FFFFFFFF";
}

export function decorationCandidate(value: unknown): DecorationCandidate | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const raw = value as Partial<DecorationCandidate>;
  const result = {
    index: raw.index ?? 1,
    fontSizeDip: raw.fontSizeDip ?? 16,
    minFontSizeDip: raw.minFontSizeDip ?? 8,
    maxWidthDip: raw.maxWidthDip ?? 0,
    color: raw.color ?? "",
    alignH: raw.alignH ?? "center",
    alignV: raw.alignV ?? "center",
    offsetXDip: raw.offsetXDip ?? 0,
    offsetYDip: raw.offsetYDip ?? 0,
    stripBadge: raw.stripBadge ?? true,
  };
  const number = (v: number, minimum: number, maximum: number) =>
    typeof v === "number" && Number.isFinite(v) && v >= minimum && v <= maximum;
  if (
    !Number.isInteger(result.index) ||
    !number(result.index, 0, 100) ||
    !number(result.fontSizeDip, 0, 200) ||
    result.fontSizeDip === 0 ||
    !number(result.minFontSizeDip, 0, 200) ||
    result.minFontSizeDip === 0 ||
    !number(result.maxWidthDip, 0, 2000) ||
    !number(result.offsetXDip, -2000, 2000) ||
    !number(result.offsetYDip, -2000, 2000) ||
    !["left", "center", "right"].includes(result.alignH) ||
    !["top", "center", "bottom"].includes(result.alignV) ||
    typeof result.stripBadge !== "boolean" ||
    typeof result.color !== "string" ||
    result.color.length > 80 ||
    /[^A-Za-z0-9#(),.% /-]/.test(result.color)
  )
    return null;
  const resolved =
    typeof raw.resolvedColor === "string" && /^#[0-9a-f]{8}$/i.test(raw.resolvedColor)
      ? raw.resolvedColor.toUpperCase()
      : resolvedColor(result.color);
  return { ...result, resolvedColor: resolved };
}
