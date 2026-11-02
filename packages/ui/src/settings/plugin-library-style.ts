/** macOS 插件库只使用平台配色；容器宽度决定卡片列数，不受窗口侧栏宽度影响。 */
export const library = "@container/plugin flex min-w-0 flex-col gap-5";
export const shortcuts = "grid grid-cols-1 gap-3 @min-[380px]/plugin:grid-cols-2";
export const shortcut =
  "group flex min-h-20 w-full min-w-0 cursor-pointer items-center gap-3.5 rounded-xl border border-[var(--p-hair)] bg-[var(--p-group-bg)] p-4 text-left [font-family:inherit] hover:border-accent-soft-border hover:bg-accent-soft focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";
export const shortcutIcon =
  "flex size-10 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent";
export const shortcutText = "flex min-w-0 flex-1 flex-col gap-1";
export const shortcutTitle = "text-sm font-semibold text-[var(--p-text)]";
export const secondary = "text-xs leading-relaxed text-[var(--p-sub)]";
export const heading = "flex flex-wrap items-center justify-between gap-3";
export const title = "m-0 text-[17px] font-semibold text-[var(--p-text)]";
export const count = "ml-2 text-xs font-normal text-[var(--p-sub)]";
export const toolbar = "flex flex-col gap-3";
export const search =
  "flex min-w-0 items-center gap-2 rounded-lg border border-[var(--p-hair)] bg-[var(--p-group-bg)] px-3 py-2 text-[var(--p-sub)] focus-within:border-accent focus-within:outline-1 focus-within:outline-accent";
export const searchInput =
  "w-full min-w-0 border-0 bg-transparent p-0 text-[13px] text-[var(--p-text)] outline-none [font-family:inherit] placeholder:text-[var(--p-sub)]";
export const filters = "flex flex-wrap gap-1.5";
export const filter =
  "inline-flex min-h-7 cursor-pointer items-center gap-1.5 rounded-md border border-transparent bg-transparent px-2.5 py-1 text-xs text-[var(--p-sub)] [font-family:inherit] hover:bg-[var(--p-hover)] aria-pressed:border-accent-soft-border aria-pressed:bg-accent-soft aria-pressed:text-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";
export const sections = "flex min-w-0 flex-col gap-5";
export const sectionHeading = "mb-2.5 flex items-center justify-between gap-2";
/** 分类名和它旁边的「?」排在一行。 */
export const sectionTitleLine = "flex items-center gap-1";
export const sectionTitle = "m-0 text-xs font-semibold text-[var(--p-sub)]";
export const grid = "grid grid-cols-1 gap-2.5 @min-[500px]/plugin:grid-cols-2";
export const card =
  "group flex min-h-[76px] w-full min-w-0 cursor-pointer items-center gap-3 rounded-xl border border-[var(--p-hair)] bg-[var(--p-group-bg)] p-3.5 text-left [font-family:inherit] hover:border-accent-soft-border hover:bg-accent-soft focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";
export const packIcon =
  "flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[var(--p-hover)] text-[var(--p-sub)] group-hover:text-accent";
export const cardBody = "flex min-w-0 flex-1 flex-col gap-1.5";
export const cardTitle = "truncate text-[13px] font-semibold text-[var(--p-text)]";
export const cardMeta =
  "flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] leading-normal text-[var(--p-sub)]";
export const marker = "text-[11px] leading-normal text-accent";
export const chevron = "size-3 shrink-0 text-[var(--p-sub)] opacity-60";
export const note = "m-0 mb-3 text-xs leading-relaxed text-[var(--p-sub)]";
export const empty =
  "rounded-xl border border-dashed border-[var(--p-hair)] px-5 py-8 text-center text-[13px] leading-relaxed text-[var(--p-sub)]";
export const footer =
  "border-t border-[var(--p-hair)] pt-3 text-xs leading-relaxed text-[var(--p-sub)] [&>summary]:w-fit [&>summary]:cursor-pointer [&>p]:mb-0";
