import type { ComponentPropsWithoutRef, ReactNode } from "react";
import * as settings from "./settings-style";

export interface SettingsPhraseFormProps extends Omit<
  ComponentPropsWithoutRef<"div">,
  "children" | "className"
> {
  children?: ReactNode;
  className?: string;
}

/** Shared wrapping layout for editable phrase and mention forms. */
export function SettingsPhraseForm({ children, className, ...props }: SettingsPhraseFormProps) {
  return (
    <div {...props} className={`${settings.phraseForm}${className ? ` ${className}` : ""}`}>
      {children}
    </div>
  );
}
