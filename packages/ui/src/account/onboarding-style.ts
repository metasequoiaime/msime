/**
 * The first-run walkthrough.
 *
 * It is the only surface in the app that owns the whole window rather than living inside the settings
 * chrome, which is why it sets its own background and centres a fixed measure: header, body and footer
 * all share one column so the text does not run the full width of a desktop window.
 */

const column = "mx-auto w-[min(560px,100%)]";

export const page =
  "flex min-h-full w-full flex-col overflow-y-auto bg-chrome px-[max(22px,5vw)] pt-7 pb-[22px] text-body max-tight:pt-[18px]";
export const header = `${column} flex items-center gap-4 [&>img]:size-[58px] [&>img]:rounded-[18px] [&>img]:bg-raised [&>img]:p-[11px] [&>h1]:m-0 [&>h1]:text-[21px] [&>h1]:font-[650]`;
export const skip = "ml-auto border-0 bg-transparent px-0 py-[5px] text-[13px] text-secondary";
export const progress = "mt-0 mb-[3px] text-xs tabular-nums text-accent";

export const body = `${column} flex-1 pt-7 pb-5 max-tight:pt-[22px]`;
export const section = "flex flex-col gap-[18px]";
export const sectionTitle = "m-0 text-[27px] font-bold tracking-[-0.02em] max-tight:text-2xl";
export const lead = "mt-[-8px] mb-1 text-[15px] leading-relaxed text-secondary";

/** A setup step: a number, a growing middle, and text that wraps inside it. */
const rowText =
  "[&_strong]:block [&_strong]:text-[15px] [&_strong]:font-semibold [&_small]:mt-1 [&_small]:text-[13px] [&_small]:leading-relaxed [&_small]:text-secondary";
/** The last step's perks: one card each, a tinted glyph and a label (dc.html `onbPerks`). */
export const perks = "m-0 flex list-none flex-col gap-2.5 p-0";
export const perk =
  "flex items-center gap-3 rounded-[14px] border border-edge bg-card px-3.5 py-3 text-[15px] [&>span]:grid [&>span]:size-8 [&>span]:flex-none [&>span]:place-items-center [&>span]:rounded-[9px] [&>span]:bg-accent-soft [&>span]:text-lg [&>span]:text-accent";
export const note = "mt-0.5 mb-0 text-xs leading-[1.7] text-muted";

export const setupCard =
  "flex flex-col gap-0 rounded-[18px] border border-edge bg-card px-[18px] py-[5px] shadow-card";
/** The connector between steps is drawn by the step itself, absolutely placed under its number. */
export const setupStep = `relative flex min-h-[76px] items-start gap-[13px] py-3.5 ${rowText} [&>span:nth-child(2)]:min-w-0 [&>span:nth-child(2)]:flex-1 [&>span:nth-child(2)]:pt-[3px] [&>i]:absolute [&>i]:top-11 [&>i]:left-3.5 [&>i]:h-[46px] [&>i]:w-0.5 [&>i]:bg-accent-soft-border`;
export const setupNumber =
  "grid size-[30px] flex-[0_0_30px] place-items-center rounded-full bg-accent-strong font-bold text-white";

export const systemActions = "flex flex-wrap gap-[9px] [&>button]:m-0";
/** This page's buttons are larger than the settings ones: they are the only action on the screen. */
export const buttons =
  "[&_.primary]:rounded-[10px] [&_.primary]:border [&_.primary]:border-accent [&_.primary]:bg-accent-strong [&_.primary]:px-[18px] [&_.primary]:py-2.5 [&_.primary]:text-white [&_.secondary]:m-0 [&_.secondary]:px-[18px] [&_.secondary]:py-2.5";

export const schemeList = "flex flex-col gap-[11px]";
export const schemeOption = (selected: boolean) =>
  `flex items-center gap-3.5 rounded-2xl border bg-card p-[17px] text-left text-body [&>span:first-child]:text-[23px] [&>span:first-child]:text-accent [&>span:nth-child(2)]:min-w-0 [&>span:nth-child(2)]:flex-1 [&_strong]:block [&_strong]:text-base [&_strong]:font-semibold [&_small]:mt-1 [&_b]:text-[21px] [&_b]:text-accent ${
    selected
      ? "border-accent shadow-[0_0_0_1px_var(--accent-color),var(--card-shadow)]"
      : "border-edge shadow-card"
  }`;

export const error = `${column} mt-0 mb-3 text-[13px] text-danger`;
export const footer = `${column} flex flex-col items-stretch gap-2.5`;
/** The page indicator. The current page's dot stretches rather than changing only colour. */
export const dots = "mb-[3px] flex justify-center gap-[7px]";
export const dot = (active: boolean) =>
  `h-2 rounded-full ${active ? "w-6 bg-accent" : "w-2 bg-[var(--border-color)]"}`;
export const next = "w-full";
export const back = "self-center border-0 bg-transparent px-[9px] py-[3px] text-secondary";
/** 稍后再说 under the primary action on the last step (dc.html `onb.hasAlt`). */
export const later =
  "m-0 self-center border-0 bg-transparent px-[9px] py-[3px] text-[15px] text-accent";

// ---- the modal sheet: iPad and the HarmonyOS 2-in-1 ----

/** On an iPad or a 2-in-1 the flow is a 480-wide sheet centred on the window rather than a full-width page (dc.html `onbModal`). */
export const sheetBackdrop =
  "relative flex min-h-full w-full items-center justify-center overflow-y-auto bg-chrome p-6 text-body";
/** The design dims whatever is behind the sheet to rgba(0,0,0,.38); the flow has nothing behind it but the window, so the window is what is dimmed. */
export const sheetScrim = "absolute inset-0 bg-black/38";
/** The sheet's corner is the platform's modal radius: 14 on an iPad, 24 on a 2-in-1 (dc.html `onb.r`). */
export const sheet = (desktop: boolean) =>
  `relative flex max-h-[min(600px,94vh)] w-[480px] max-w-full flex-col overflow-y-auto border border-edge bg-raised px-7 py-6 shadow-[0_30px_80px_rgba(0,0,0,0.4)] ${
    desktop ? "rounded-[24px]" : "rounded-[14px]"
  }`;

// ---- the desktop button bar (HarmonyOS 2-in-1) ----

/** 36 high and fully rounded on a 2-in-1 (dc.html `ob.btnH`, `ob.btnR`); the secondary button takes the accent as its text. */
export const deskFooter = `${column} mt-4 flex items-center gap-2 [&>button]:m-0 [&>button]:h-9 [&>button]:min-w-18 [&>button]:rounded-[18px]! [&>button]:py-0! [&>button]:text-[13px] [&>.secondary]:border-0 [&>.secondary]:bg-[var(--p-hover)] [&>.secondary]:text-accent`;
export const deskLink = "border-0 bg-transparent px-3.5 text-accent";
export const deskSpacer = "flex-1";

// ---- Android: a linear progress bar over the page and a pair of buttons under it ----

export const progressTrack = `${column} mb-5 h-1 overflow-hidden rounded-full bg-[var(--border-color)]`;
export const progressFill =
  "block h-full rounded-full bg-accent transition-[width] duration-300 motion-reduce:transition-none";
export const pairFooter = `${column} flex items-center justify-between gap-3 pt-4 [&_button]:m-0 [&_.primary]:h-10 [&_.primary]:rounded-[20px]! [&_.primary]:px-6! [&_.primary]:py-0! [&_.primary]:text-sm [&_.primary]:font-medium`;
export const pairEnd = "flex items-center gap-2";
/** Material's text button: accent text, a tint on hover, no container (dc.html `ob.andLeft`). */
export const textButton =
  "h-10 rounded-[20px] border-0 bg-transparent px-3 text-sm font-medium text-accent hover:bg-accent-soft";

// ---- step 3: the candidate bar with its translation line ----

export const glossPreview =
  "flex gap-2 overflow-hidden rounded-2xl border border-edge bg-card p-3 shadow-card";
export const glossCandidate = (first: boolean) =>
  `flex min-w-0 flex-col items-center rounded-[10px] px-3 py-2 ${first ? "bg-accent-soft" : ""} [&>span]:text-[17px] [&>small]:mt-0.5 [&>small]:text-[11px] [&>small]:text-secondary`;
export const glossRow =
  "flex items-center justify-between gap-4 rounded-2xl border border-edge bg-card px-[17px] py-3.5 shadow-card [&_strong]:block [&_strong]:text-[15px] [&_strong]:font-semibold [&_small]:mt-1 [&_small]:block [&_small]:text-[13px] [&_small]:leading-relaxed [&_small]:text-secondary";

// ---- the splash ----

/** The window behind the splash, so the near-black reaches every edge rather than stopping at the flow's padding. */
export const splashShell = "flex min-h-full w-full bg-[#0E100E]";
/** The first-launch splash (dc.html L1740-1745): the mark in its frame over a green glow, the name and the Latin name under it, whatever the theme. */
export const splash =
  "relative flex min-h-full w-full cursor-pointer flex-col items-center justify-center gap-[18px] overflow-hidden border-0 bg-[#0E100E] p-0 text-white";
export const splashGlow =
  "absolute size-80 rounded-full bg-[radial-gradient(circle,rgba(127,224,142,0.28)_0%,rgba(127,224,142,0)_65%)] animate-splash-glow motion-reduce:animate-none";
export const splashLogo = "relative size-[124px] animate-splash-pop motion-reduce:animate-none";
export const splashFrame = "fill-[#252525] stroke-[#A8DF8E] [stroke-width:4]";
export const splashStroke =
  "fill-none stroke-white [stroke-dasharray:1] [stroke-linecap:round] [stroke-width:9] animate-splash-draw motion-reduce:animate-none";
export const splashName =
  "relative text-2xl font-bold tracking-[0.06em] animate-splash-name motion-reduce:animate-none";
export const splashTagline =
  "relative text-[13px] tracking-[0.18em] text-white/62 animate-splash-tagline motion-reduce:animate-none";
export const splashHint =
  "absolute bottom-[max(36px,env(safe-area-inset-bottom))] text-xs text-white/40 animate-splash-hint motion-reduce:animate-none";
