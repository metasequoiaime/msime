import { Select } from "../core/platform-controls";

export type SurfaceTheme = "follow" | "dark" | "light";

export interface SurfaceThemeSelectProps {
  label: string;
  value: SurfaceTheme;
  onChange: (value: SurfaceTheme) => void;
}

/** Shared light/dark override options for settings surfaces. */
export function SurfaceThemeSelect({ label, value, onChange }: SurfaceThemeSelectProps) {
  return (
    <Select
      aria-label={label}
      value={value}
      onChange={(event) => onChange(event.target.value as SurfaceTheme)}
    >
      <option value="follow">跟随全局</option>
      <option value="dark">深色</option>
      <option value="light">浅色</option>
    </Select>
  );
}
