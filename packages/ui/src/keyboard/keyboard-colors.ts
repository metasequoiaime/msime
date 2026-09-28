import { skinLuminance } from "./touch-keyboard-skin-design";

export function mixKeyboardColor(first: string, second: string, amount: number) {
  const parse = (value: string) => Number.parseInt(value.replace(/^#/, ""), 16);
  const a = parse(first);
  const b = parse(second);
  if (!Number.isFinite(a) || !Number.isFinite(b)) return first;
  const channel = (shift: number) =>
    Math.round(((a >> shift) & 0xff) * (1 - amount) + ((b >> shift) & 0xff) * amount);
  return `#${((channel(16) << 16) | (channel(8) << 8) | channel(0)).toString(16).padStart(6, "0")}`;
}

export function readableKeyboardText(value: string) {
  const parsed = Number.parseInt(value.replace(/^#/, ""), 16);
  return Number.isFinite(parsed) && skinLuminance(parsed) > 0.179 ? "#000000" : "#ffffff";
}

export function keyboardRgba(value: string, opacity: number) {
  const parsed = Number.parseInt(value.replace(/^#/, ""), 16);
  if (!Number.isFinite(parsed)) return value;
  const channel = (shift: number) => (parsed >> shift) & 0xff;
  return `rgba(${channel(16)}, ${channel(8)}, ${channel(0)}, ${Math.max(0, Math.min(1, opacity))})`;
}
