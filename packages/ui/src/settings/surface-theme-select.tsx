import { Select } from "../core/platform-controls";

export type SurfaceTheme = "follow" | "dark" | "light";

const surfaceThemeLabels: Record<SurfaceTheme, string> = {
  follow: "跟随颜色模式",
  dark: "深色",
  light: "浅色",
};
const defaultSurfaceThemeOrder: readonly SurfaceTheme[] = ["follow", "dark", "light"];

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
      {optionOrder.map((theme) => (
        <option key={theme} value={theme}>
          {surfaceThemeLabels[theme]}
        </option>
      ))}
    </Select>
  );
}
