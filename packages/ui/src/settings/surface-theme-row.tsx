import type { ReactNode } from "react";
import { SelectRow } from "./select-row";
import { surfaceThemeOptions } from "./surface-theme-select";
import type { SurfaceTheme } from "./theme-preferences";

export interface SurfaceThemeRowProps {
  title: string;
  description: ReactNode;
  value: SurfaceTheme | undefined;
  onChange: (value: SurfaceTheme) => void;
}

/** 某个界面单独的明暗覆盖。HarmonyOS 手机上整行点开选择面板，与其他 `SelectRow` 一致。 */
export function SurfaceThemeRow({ title, description, value, onChange }: SurfaceThemeRowProps) {
  return (
    <SelectRow
      title={title}
      description={description}
      aria-label={title}
      value={value ?? "follow"}
      onChange={(event) => onChange(event.target.value as SurfaceTheme)}
    >
      {surfaceThemeOptions()}
    </SelectRow>
  );
}
