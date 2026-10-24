import type { ReactNode } from "react";
export { PlusGlyph } from "../core/plus-glyph";

/** 行首的字标：词库名的第一个字，或者操作的符号，和 Android 词库页的徽标一样。 */
export function Badge({ children }: { children: ReactNode }) {
  return (
    <span className="flex size-7 items-center justify-center rounded-[8px] text-[14px] font-semibold [background:color-mix(in_srgb,var(--p-accent-text)_14%,transparent)] [color:var(--p-accent-text)]">
      {children}
    </span>
  );
}
