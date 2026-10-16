import type { ThemeMode } from "./theme-preferences";

/** 主题页的全局明暗模式选项，供共享设置页和宿主投影复用。 */
export const themeModeOptions = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
] as const satisfies readonly { value: ThemeMode; label: string }[];
