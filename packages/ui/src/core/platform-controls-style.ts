/**
 * Class strings for the platform controls in `platform-controls.tsx`. Every length and colour is a `--p-*` token from the `[data-platform]` layer in `styles.css`, so one markup takes each platform's shape. Values that Tailwind could read either as a colour or as a size (`text-`, `shadow-`, `border-`) are written as arbitrary properties so the property is never inferred.
 */

// ---- groups and rows ----

export const group = "flex min-w-0 flex-col gap-[var(--p-g-title-gap)]";
export const groupTitle =
  "m-0 [padding:var(--p-g-title-pad)] [font-size:var(--p-g-title-fs)] [font-weight:var(--p-g-title-w)] [color:var(--p-g-title-fg)]";
/** The hairline sits on every shown row after the first shown one, so a group of one has none, the last row never carries a trailing one, and a hidden row leaves no line behind. */
export const groupRows =
  "flex flex-col gap-[var(--p-row-gap)] rounded-[var(--p-group-r)] bg-[var(--p-group-bg)] [box-shadow:var(--p-group-shadow)] [&>:not([hidden])~:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)]";

export const row =
  "flex min-h-[var(--p-row-h)] min-w-0 items-center gap-3 rounded-[var(--p-row-r)] bg-[var(--p-row-bg)] [padding:var(--p-row-pad)] [box-shadow:var(--p-row-shadow)]";
/** Windows leads each card with a glyph; the other platforms set `--p-row-icon: none`. */
export const rowIcon = "size-5 shrink-0 items-center justify-center [display:var(--p-row-icon)]";
export const rowText = "flex min-w-0 flex-1 flex-col gap-0.5";
export const rowTitle = "block [font-size:var(--p-row-fs)] [color:var(--p-text)]";
export const rowDescription = "block [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const rowControl = "flex shrink-0 items-center gap-2";
/** 跳转行：整行是一个按钮，外形与 `row` 相同，悬停时变色。 */
export const linkRow = `${row} w-full cursor-pointer border-0 text-left hover:bg-[var(--p-hover)]`;

// ---- 更多选项 ----

/** 「更多选项」折叠区本身是组里的一行，靠 `group/more` 让标题的箭头随展开旋转。 */
export const moreOptions = "group/more min-w-0";
export const moreOptionsSummary = `${row} cursor-pointer list-none hover:bg-[var(--p-hover)] [&::-webkit-details-marker]:hidden`;
export const moreOptionsMarker =
  "[font-size:var(--p-sub-fs)] [color:var(--p-sub)] transition-transform group-open/more:rotate-90 motion-reduce:transition-none";
/** 展开后的每一行都画上分隔线，与组内各行之间的分隔一致。 */
export const moreOptionsRows =
  "flex flex-col gap-[var(--p-row-gap)] [&>:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)]";

// ---- 页面简介 ----

/** 页首说明：页面第一组之前的一段灰字，内边距取组名的 `--p-g-title-pad`，与下面各组的组名对齐。 */
export const pageIntro =
  "m-0 leading-relaxed [padding:var(--p-g-title-pad)] [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";

// ---- switch ----

export const switchWrap = "inline-flex items-center gap-3";
/** Windows prints 开/关 before the switch; `--p-sw-label` is `none` everywhere else. */
export const switchState = "[display:var(--p-sw-label)] [font-size:14px] [color:var(--p-text)]";

// ---- segmented ----

export const segmented =
  "inline-flex max-w-full items-stretch gap-0.5 rounded-[var(--p-seg-r)] bg-[var(--p-seg-bg)] p-0.5 [border:var(--p-seg-border)]";
/** Windows marks the chosen item with a short accent bar; `--p-seg-indicator` is 0 elsewhere, which draws nothing. */
export const segment =
  "relative flex min-h-7 cursor-pointer items-center justify-center rounded-[var(--p-seg-item-r)] px-3 [font-size:13px] [color:var(--p-text)] after:absolute after:bottom-0.5 after:left-1/2 after:hidden after:h-[3px] after:w-[var(--p-seg-indicator)] after:-translate-x-1/2 after:rounded-full after:bg-accent has-[:checked]:bg-[var(--p-seg-on)] has-[:checked]:[color:var(--p-seg-on-fg)] has-[:checked]:[box-shadow:var(--p-seg-on-shadow)] has-[:checked]:after:block has-[:disabled]:cursor-default has-[:disabled]:opacity-50 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-1 has-[:focus-visible]:outline-accent";
/** The radio stays in the tree for its role, arrow keys and focus; only its drawing goes. */
export const segmentInput = "sr-only";

// ---- select ----

/** The native element and its own arrow are kept (see UPSTREAM.md); only the box around it follows the platform. */
export const select =
  "min-w-0 rounded-[var(--p-r-ctl)] [background:var(--p-sel-bg)] [border:var(--p-sel-border)] [padding:var(--p-sel-pad)] [font-size:var(--p-sel-fs)] [color:var(--p-sel-fg)]";

// ---- checks ----

export const checks = "m-0 flex min-w-0 flex-col gap-2 border-0 p-0";
export const checksLegend = "mb-1 p-0 [font-size:var(--p-row-fs)] [color:var(--p-text)]";
export const checksDescription =
  "-mt-1 mb-1 block [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const check =
  "flex cursor-pointer items-center gap-2.5 [font-size:var(--p-row-fs)] [color:var(--p-text)] has-[:disabled]:cursor-default has-[:disabled]:opacity-50";

// ---- navigation ----

/** Windows adds the 3×16 accent pill on the selected item; `--p-nav-pill` is 0 elsewhere. */
export const navItem =
  "relative flex h-[var(--p-nav-h)] w-full min-w-0 items-center gap-3 rounded-[var(--p-nav-r)] border-0 bg-transparent text-left [padding:var(--p-nav-pad)] [font-size:14px] [color:var(--p-text)] before:absolute before:top-1/2 before:left-0 before:hidden before:h-4 before:w-[var(--p-nav-pill)] before:-translate-y-1/2 before:rounded-full before:bg-accent hover:bg-[var(--p-hover)] aria-[current=page]:bg-[var(--p-nav-selected-bg)] aria-[current=page]:before:block";
export const navIcon = "flex size-5 shrink-0 items-center justify-center";
export const navLabel = "min-w-0 flex-1 truncate";
