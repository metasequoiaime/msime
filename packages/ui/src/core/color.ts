/** 计算 `0xRRGGBB` 颜色的 sRGB 相对亮度；解析和透明度处理由调用方负责。 */
export function relativeLuminance(rgb: number): number {
  const channel = (shift: number) => {
    const value = ((rgb >> shift) & 255) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
}
