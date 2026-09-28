export function keyboardKeyPath(
  x: number,
  y: number,
  width: number,
  height: number,
  shape: string,
  radius: number,
): string {
  if (shape === "pebble")
    return `M${x + width * 0.35} ${y}C${x + width * 0.83} ${y} ${x + width} ${y + height * 0.04} ${x + width} ${y + height * 0.3}C${x + width} ${y + height * 0.85} ${x + width * 0.9} ${y + height} ${x + width * 0.68} ${y + height}C${x + width * 0.18} ${y + height} ${x} ${y + height * 0.97} ${x} ${y + height * 0.7}C${x} ${y + height * 0.2} ${x + width * 0.06} ${y} ${x + width * 0.35} ${y}Z`;
  if (shape === "ticket") {
    const r = Math.min(width, height) * 0.12;
    return `M${x} ${y}H${x + width}V${y + height / 2 - r}A${r} ${r} 0 0 0 ${x + width} ${y + height / 2 + r}V${y + height}H${x}V${y + height / 2 + r}A${r} ${r} 0 0 0 ${x} ${y + height / 2 - r}Z`;
  }
  const r = shape === "capsule" ? height / 2 : Math.min(radius, height / 2, width / 2);
  return `M${x + r} ${y}H${x + width - r}Q${x + width} ${y} ${x + width} ${y + r}V${y + height - r}Q${x + width} ${y + height} ${x + width - r} ${y + height}H${x + r}Q${x} ${y + height} ${x} ${y + height - r}V${y + r}Q${x} ${y} ${x + r} ${y}Z`;
}
