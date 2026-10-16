import { Select } from "../core/platform-controls";
import type { SurfaceTheme } from "./theme-preferences";

export type { SurfaceTheme } from "./theme-preferences";

const surfaceThemeLabels: Record<SurfaceTheme, string> = {
  follow: "跟随颜色模式",
  dark: "深色",
  light: "浅色",
};
const defaultSurfaceThemeOrder: readonly SurfaceTheme[] = ["follow", "dark", "light"];

/** 明暗覆盖的 `<option>` 列表。是返回数组的普通函数而不是组件：`SelectRow` 在 HarmonyOS 上只认直接的 `<option>` 子元素。 */
export function surfaceThemeOptions(
  optionOrder: readonly SurfaceTheme[] = defaultSurfaceThemeOrder,
) {
  return optionOrder.map((theme) => (
    <option key={theme} value={theme}>
      {surfaceThemeLabels[theme]}
    </option>
  ));
}

export interface SurfaceThemeSelectProps {
  label: string;
  value: SurfaceTheme;
  onChange: (value: SurfaceTheme) => void;
  optionOrder?: readonly SurfaceTheme[];
}

/** Shared light/dark override options for settings surfaces. */
export function SurfaceThemeSelect({
  label,
  value,
  onChange,
  optionOrder = defaultSurfaceThemeOrder,
}: SurfaceThemeSelectProps) {
  return (
    <Select
      aria-label={label}
      value={value}
      onChange={(event) => onChange(event.target.value as SurfaceTheme)}
    >
      {surfaceThemeOptions(optionOrder)}
    </Select>
  );
}
