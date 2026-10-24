import type { ReactNode } from "react";

/** 「添加」的加号，画在和词库行同样的字标方块里，看上去就是往列表里再加一项。图标库没有加号，这里直接画两条居中的线。 */
export function PlusGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M8 2.5v11M2.5 8h11" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

/** 行首的字标：词库名的第一个字，或者操作的符号，和 Android 词库页的徽标一样。 */
export function Badge({ children }: { children: ReactNode }) {
  return (
    <span className="flex size-7 items-center justify-center rounded-[8px] text-[14px] font-semibold [background:color-mix(in_srgb,var(--p-accent-text)_14%,transparent)] [color:var(--p-accent-text)]">
      {children}
    </span>
  );
}
