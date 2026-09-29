/**
 * The utility strings the two community pages share.
 *
 * Skins and resources are the same gallery with different payloads -- a search row, a heading with
 * scope controls, a card grid, a detail view, a publish dialog -- and they were drifting apart: the
 * skin gallery had a full stylesheet while the resource gallery shipped with class names and no rules
 * at all, so that page rendered unstyled from the day it was added. Naming the shared pieces once is
 * what stops the next page from being able to do that.
 */

export const page = "flex flex-col gap-3.5";

export const searchRow = "flex gap-2";
export const searchInput =
  "min-w-0 flex-1 rounded-[9px] border border-control-border bg-[var(--dropdown-bg)] px-3 py-[9px] font-[inherit] text-body focus:border-accent";
export const searchSubmit =
  "rounded-[9px] border border-accent-soft-border bg-accent-strong px-4 py-2 text-white";

/**
 * Heading and actions side by side, stacking on a phone.
 *
 * They used to stay side by side at every width, with headingBody allowed to shrink so the actions
 * always fit. That worked while there were two buttons; the skin gallery has three, and squeezing
 * them in left the title wrapping one or two characters per line. Below the tight breakpoint the
 * actions take their own row instead, which is also where they have room to be legible.
 */
export const heading =
  "flex items-start justify-between gap-4 px-0.5 py-1 max-tight:flex-col max-tight:items-stretch max-tight:gap-2";
/** The heading's left column has to be allowed to shrink or the actions get pushed off the edge. */
export const headingBody = "min-w-0";
export const headingTitle = "m-0 text-xl [color:var(--p-text)]";
export const headingNote = "mt-[5px] mb-0 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
/** Stacked on a roomy window, laid out in a row once the heading has to share a phone's width. */
/**
 * The actions beside the gallery heading.
 *
 * Wrapping matters below the tight breakpoint. There the column becomes a row so the scope switch
 * sits next to the heading rather than under it, and the skin gallery puts a third button there —
 * 发布我的设计. Three nowrap buttons do not fit a phone, and without a wrap the third is laid out
 * past the right edge with nothing to scroll it into view.
 */
export const headingActions =
  "flex shrink-0 grow-0 basis-auto flex-col items-end gap-2 max-tight:flex-row max-tight:flex-wrap max-tight:items-center max-tight:justify-start [&>button]:m-0 [&>button]:whitespace-nowrap";
/**
 * A scope switch that stays put. Two buttons fit a phone, and the skin gallery has exactly two -- the
 * stylesheet hid these below 560px for every page, which left that gallery with no way to reach 我的作品
 * on a phone at all, because only the resource gallery had the overflow menu meant to replace them.
 */
export const scopeButtons = "flex gap-1.5 [&>button]:m-0 [&>button]:whitespace-nowrap";
/** The collapsing variant, for a page that also renders `scopeMenu` to take over below that width. */
export const scopeButtonsCollapsing = `${scopeButtons} max-tight:hidden`;
export const scopeMenu = "relative hidden max-tight:block";
export const scopeMenuSummary =
  "cursor-pointer list-none rounded-lg border border-control-border bg-[var(--dropdown-bg)] px-3 py-[7px] whitespace-nowrap text-body after:ml-[7px] after:text-muted after:content-['⌄'] open:after:content-['⌃'] [&::-webkit-details-marker]:hidden";
export const scopeMenuList =
  "absolute top-[calc(100%+6px)] right-0 z-[4] flex min-w-32 flex-col gap-[3px] rounded-[9px] border border-edge bg-[var(--dropdown-bg)] p-[5px] shadow-card";
export const scopeMenuItem =
  "m-0 rounded-md border-0 bg-transparent px-[9px] py-[7px] text-left whitespace-nowrap text-body hover:bg-[var(--button-secondary-bg)]";

export const grid = "grid grid-cols-2 gap-x-3 gap-y-3.5 max-phone:grid-cols-1";
/** A gallery tile keeps its own card surface: on Windows a group is transparent and square, which would leave a tile with no edge at all. Only its text takes the platform tokens. */
export const card =
  "flex min-w-0 flex-col items-stretch gap-2 overflow-hidden rounded-[19px] border border-edge bg-card p-2.5 text-left [color:var(--p-text)] shadow-card hover:border-edge-strong";
/** The stage a preview sits on: it clips the artwork and gives it a backdrop of its own. */
export const cardStage = "block overflow-hidden rounded-[11px] bg-[var(--skin-preview-stage-bg)]";
export const cardTitle =
  "overflow-hidden [font-size:var(--p-row-fs)] font-medium text-ellipsis whitespace-nowrap";
export const cardAuthor =
  "overflow-hidden [font-size:var(--p-sub-fs)] text-ellipsis whitespace-nowrap [color:var(--p-sub)]";
export const cardMetrics =
  "flex justify-between gap-2 text-[11px] tabular-nums [color:var(--p-sub)]";

export const more = "m-0 self-center";
export const notice = "my-[26px] text-center text-muted";

export const back =
  "w-fit rounded-[7px] border-0 bg-transparent px-2.5 py-1.5 text-secondary hover:bg-[var(--button-secondary-bg)] hover:text-body";
export const detail = "m-0 flex flex-col gap-4";
export const detailStage = "overflow-hidden rounded-xl bg-[var(--skin-preview-stage-bg)]";
export const detailTitle = "flex items-start justify-between gap-3";
export const detailBadge =
  "shrink-0 grow-0 basis-auto rounded-full bg-accent-soft px-2 py-1 text-[11px] text-accent";
export const description = "m-0 leading-[1.7] whitespace-pre-wrap break-anywhere text-secondary";
export const metrics = "m-0 text-xs leading-relaxed text-muted";
/** A block that is separated from the one above it by a rule rather than by space alone. */
export const divided = "border-t border-[var(--divider-color)] pt-3.5";
export const action = "min-h-[42px] self-stretch";
export const actionNotice = "m-0 rounded-[9px] bg-accent-soft px-3 py-2.5 text-xs text-secondary";

export const confirmation =
  "flex flex-col gap-3 rounded-[10px] border border-danger bg-raised p-3.5 [&>p]:m-0 [&>p]:leading-relaxed [&>p]:text-secondary";
export const confirmationActions = "flex flex-wrap gap-2";
export const destructive = "rounded-lg border border-danger bg-danger px-3 py-[7px] text-white";

export const backdrop = "fixed inset-0 z-20 grid place-items-center overflow-auto bg-black/45 p-6";
export const dialog =
  "flex max-h-[calc(100vh-48px)] w-[min(620px,100%)] flex-col gap-3.5 overflow-auto rounded-[14px] border border-edge-strong bg-raised p-5 text-body shadow-[0_18px_60px_rgba(0,0,0,0.28)]";
export const dialogHeading = "flex items-center justify-between gap-3";
export const dialogTitle = "m-0 text-lg text-body";
export const dialogClose =
  "size-8 rounded-lg border-0 bg-transparent text-2xl leading-none text-muted not-disabled:hover:bg-[var(--button-secondary-bg)] not-disabled:hover:text-body";
export const field = "flex flex-col gap-[7px] text-xs text-secondary";
export const fieldControl =
  "box-border w-full rounded-lg border border-control-border bg-[var(--dropdown-bg)] px-2.5 py-2 font-[inherit] text-body";
export const textArea = `${fieldControl} resize-y leading-normal`;
export const agreement = "flex flex-row items-start gap-2 text-xs leading-normal text-secondary";
export const agreementBox = "mt-0.5 size-4 shrink-0 grow-0 accent-[var(--accent-color)]";
export const warning = "m-0 text-xs leading-relaxed text-muted";
export const dialogActions =
  "flex justify-end gap-2 border-t border-[var(--divider-color)] pt-1 [&>button]:m-0";

/*
 * The resource gallery's own pieces. These had class names and no rules at all, so this page rendered
 * with browser defaults from the commit that added it -- a tile with no border, no padding and no
 * grid. The shapes below follow the skin gallery, which is the same gallery with a different payload.
 */

/** A resource has no artwork, so the tile leads with a glyph standing in for its kind. */
export const resourceIcon =
  "grid size-9 place-items-center rounded-[11px] bg-accent-soft text-lg [color:var(--p-accent-text)]";
export const resourceDescription =
  "line-clamp-2 min-h-8 [font-size:var(--p-sub-fs)] leading-relaxed [color:var(--p-sub)]";

/** The add-an-entry row: four narrow fields and a button, wrapping rather than squeezing on a phone. */
export const entryForm =
  "grid grid-cols-[repeat(auto-fit,minmax(110px,1fr))] items-end gap-2 rounded-[10px] bg-subtle p-2.5 [&>button]:m-0";
/** The pending entries. Scrolls rather than growing the dialog past the viewport. */
export const entryList =
  "flex max-h-[200px] flex-col gap-1.5 overflow-y-auto [&>div]:flex [&>div]:items-center [&>div]:justify-between [&>div]:gap-2 [&>div]:rounded-lg [&>div]:bg-subtle [&>div]:px-2.5 [&>div]:py-1.5 [&>div]:text-xs [&>div>button]:m-0";
export const promptPreview =
  "m-0 max-h-[220px] overflow-auto rounded-[10px] bg-subtle p-3 text-xs leading-relaxed whitespace-pre-wrap break-anywhere text-secondary";
export const entryPreview =
  "max-h-[220px] overflow-y-auto rounded-[10px] bg-subtle p-2.5 text-xs [&>div]:flex [&>div]:justify-between [&>div]:gap-2 [&>div]:py-1";

/** The category strip above the gallery: one column per tab, like the phone's other tab strips. */
export const categoryTabs =
  "grid grid-cols-3 gap-[3px] rounded-[9px] bg-subtle p-[3px] [&>button]:min-h-[34px] [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-transparent [&>button]:text-secondary [&>button[aria-selected=true]]:bg-raised [&>button[aria-selected=true]]:text-body [&>button[aria-selected=true]]:shadow-card";
