import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { SurfaceThemeSelect, type SurfaceTheme } from "./surface-theme-select";

export interface SurfaceThemeRowProps {
  title: string;
  description: ReactNode;
  value: SurfaceTheme | undefined;
  onChange: (value: SurfaceTheme) => void;
}

/** A settings row for a surface-specific light/dark override. */
export function SurfaceThemeRow({ title, description, value, onChange }: SurfaceThemeRowProps) {
  return (
    <Row title={title} description={description}>
      <SurfaceThemeSelect label={title} value={value ?? "follow"} onChange={onChange} />
    </Row>
  );
}
