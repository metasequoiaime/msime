import type { ReactNode } from "react";
import * as settings from "./settings-style";

type SettingsExternalMetaProps = {
  children: ReactNode;
  as?: "p" | "span";
  role?: "alert" | "status";
};

export function SettingsExternalMeta({ children, as = "p", role }: SettingsExternalMetaProps) {
  const Element = as;
  return (
    <Element className={settings.externalMeta} role={role}>
      {children}
    </Element>
  );
}
