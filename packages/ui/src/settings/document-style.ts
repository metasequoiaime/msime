/**
 * The prose pages -- about, the guides, feedback -- and the AI skin generator's cards.
 *
 * These are the surfaces built out of running text rather than controls, so the shapes here are
 * mostly about measure and rhythm: a hero, subsections divided by a rule, and rows that read as a
 * list even though each one is a button.
 */

export const page = "leading-[1.7] [&>p]:mt-0 [&>p]:mb-3.5 [&>p]:text-secondary";
export const subsection =
  "mt-6 border-t border-[var(--divider-color)] pt-[18px] [&>p:last-child]:mb-0";
/** The page's lead: the mark or eyebrow, a title and a line of prose, as the first block of an untitled group. */
export const hero =
  "flex items-center gap-[18px] px-5 py-5 [&_p]:mt-[7px] [&_p]:mb-0 [&_p]:[font-size:var(--p-sub-fs)] [&_p]:[color:var(--p-sub)]";
export const eyebrow = "text-xs tracking-[0.04em] [color:var(--p-sub)]";
export const heroTitle = "mt-0.5 text-[22px] font-semibold [color:var(--p-text)]";
export const note = "flex flex-col gap-[5px] text-secondary [&>span]:text-xs [&>span]:text-muted";

/**
 * The guide cards: a term on the left, what it does on the right. The macOS reference window leads
 * each card with a single row, rules it off, and runs the rest together; the first row is the one
 * that carries the whole card, so it gets the air.
 */
export const guide = "flex flex-col gap-3";
export const guideRow =
  "grid grid-cols-[minmax(0,170px)_minmax(0,1fr)] items-baseline gap-5 max-narrow:grid-cols-1 max-narrow:gap-1";
export const guideLead = "border-b border-[var(--divider-color)] pb-3.5";
export const guideTerm = "text-body";
export const guideText = "m-0! text-secondary";

export const mark =
  "flex size-[62px] items-center justify-center rounded-2xl bg-raised p-2.5 [&>img]:size-12";
/** A row of a group that opens a page or a link; the group draws the dividers between rows. */
export const linkRow =
  "flex min-h-[var(--p-row-h)] w-full cursor-pointer items-center justify-between gap-4 rounded-[var(--p-row-r)] border-0 bg-[var(--p-row-bg)] text-left [padding:var(--p-row-pad)] [color:var(--p-sub)] hover:bg-[var(--p-hover)]";
export const linkTitle = "[font-size:var(--p-row-fs)] [color:var(--p-text)]";

export const version = "mt-1 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const versionRow = "cursor-default items-start hover:bg-[var(--p-row-bg)]";
export const updateButton = "mt-0 shrink-0 grow-0 basis-auto";
export const updateStatus = "mt-[5px]! mb-0! text-xs";
export const updateResult =
  "[padding:var(--p-row-pad)] [color:var(--p-sub)] [&>p]:mt-0 [&>p]:mb-2 [&>p]:text-xs [&_code]:inline-block [&_code]:max-w-full [&_code]:break-anywhere";
export const updateWarning = "text-danger!";

/** Icon, body, action. The icon column narrows on a phone, where 42px of gutter is a lot to give up. */
export const feedbackCard =
  "grid min-h-[var(--p-row-h)] grid-cols-[42px_minmax(0,1fr)_auto] items-center gap-3.5 [padding:var(--p-row-pad)] max-phone:grid-cols-[36px_minmax(0,1fr)] [&>.secondary]:mt-0 [&>.secondary]:whitespace-nowrap [&>.secondary]:max-phone:col-start-2 [&>.secondary]:max-phone:justify-self-start";
export const feedbackIcon =
  "flex size-[38px] items-center justify-center rounded-[10px] bg-accent-soft text-xs font-bold [color:var(--p-accent-text)]";
export const feedbackBody =
  "min-w-0 [&>p]:mt-[3px] [&>p]:mb-[5px] [&>p]:[font-size:var(--p-sub-fs)] [&>p]:[color:var(--p-sub)] [&_code]:block [&_code]:break-anywhere [&_code]:text-xs [&_code]:[color:var(--p-sub)]";
export const feedbackTitle = "[font-size:var(--p-row-fs)] font-semibold [color:var(--p-text)]";

// ---- the AI skin generator ----

export const generation = "w-[min(760px,100%)]";
export const generationNote = "m-0 text-xs leading-normal text-muted";
/** Three face-down cards, fanned. The rotation is what makes them read as a draw rather than a grid. */
export const mysteryCards = "grid grid-cols-3 gap-2.5 py-3";
export const mysteryCard = (index: number) =>
  `flex min-h-[130px] flex-col items-center justify-between rounded-[14px] p-3.5 text-[11px] tracking-[0.08em] text-white [&>span]:text-[34px] [&>span]:font-light [&>span]:tracking-normal ${
    index === 1
      ? "-translate-y-1.5 bg-[linear-gradient(145deg,#5b528d,#29234c)]"
      : index === 2
        ? "rotate-4 bg-[linear-gradient(145deg,#9b6b45,#4c2e26)]"
        : "-rotate-4 bg-[linear-gradient(145deg,#4d806b,#1a493c)]"
  }`;
export const cardList = "grid grid-cols-3 gap-3 max-narrow:grid-cols-1";
export const card =
  "flex min-w-0 flex-col gap-2 rounded-xl border border-edge bg-subtle p-3 [&>h3]:m-0 [&>h3]:text-sm [&>p]:m-0 [&>p]:min-h-[52px] [&>p]:text-[11px] [&>p]:leading-normal [&>p]:text-muted max-narrow:[&>p]:min-h-0 [&_.screen-keyboard-artwork]:min-h-40 [&_.screen-keyboard-artwork]:w-full";
export const cardActions = "flex flex-wrap gap-1.5 [&>button]:m-0 [&>button]:flex-[1_1_100%]";
export const publishForm =
  "mt-1 flex flex-col gap-2.5 rounded-[10px] border border-edge-strong bg-raised p-3 [&>h3]:m-0 [&>p]:m-0";
