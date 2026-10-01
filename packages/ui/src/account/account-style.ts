/**
 * The account page, its profile modal and the app-icon picker.
 *
 * The stylesheet reached these through descendant selectors on `.account-page` -- every `label`,
 * every `input`, every `p` inside it -- which meant a control could not be moved out of the page
 * without silently losing its styling, and anything dropped into the page picked styling up whether
 * it wanted to or not. Each piece is named here instead.
 */

export const page = "flex flex-col gap-3.5";
export const section = "section m-0";
export const heading = "m-0 [font-size:var(--p-row-fs)] font-semibold [color:var(--p-text)]";
export const note = "mt-[7px] mb-0 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const muted = "text-xs [color:var(--p-sub)]";

export const field = "flex flex-col gap-[7px] text-xs text-secondary";
export const input =
  "min-h-[38px] w-full rounded-lg border border-control-border bg-[var(--dropdown-bg)] px-2.5 py-[7px] font-[inherit] text-body focus:border-accent";
export const primary =
  "w-fit whitespace-nowrap rounded-lg border border-accent-soft-border bg-accent-strong px-3.5 py-[7px] text-white disabled:opacity-60";
/** A row of buttons that wraps rather than overflowing; every account surface uses the same one. */
export const actionRow =
  "flex flex-wrap items-center gap-[9px] [&>.secondary]:m-0 [&>.danger-text]:m-0";
export const stack = "flex flex-col gap-4";

/** A status line from the page's own actions, set apart from the cards instead of floating as bare text above them. */
export const status = "m-0 rounded-lg bg-subtle px-3.5 py-2.5 text-[13px] text-secondary";

// ---- signing in ----

/** The signed-out page is one card: the welcome, the providers and the repair actions, centred in a column narrow enough that the buttons read as a list rather than as banners. */
export const signIn = "flex flex-col items-center gap-5 py-8 text-center";
export const signInHeader = "flex flex-col items-center gap-3 [&_h2]:text-[17px] [&_p]:mt-1.5";
export const signInBody = "flex w-full max-w-[320px] flex-col gap-2.5 text-left";
export const provider =
  "w-full cursor-pointer rounded-lg border border-control-border bg-[var(--dropdown-bg)] px-3.5 py-2.5 text-center font-medium text-body not-disabled:hover:border-edge-strong disabled:cursor-default disabled:opacity-60";
export const submit =
  "w-full rounded-lg border border-accent-soft-border bg-accent-strong px-3.5 py-2.5 font-medium text-white disabled:opacity-60";
/** The actions that repair a broken sign-in rather than perform one: present, but quiet. */
export const signInFooter = "flex flex-wrap items-center justify-center gap-x-4 gap-y-1.5";
export const link =
  "cursor-pointer border-0 bg-transparent p-0 text-xs [color:var(--p-sub)] not-disabled:hover:[color:var(--p-text)] not-disabled:hover:underline disabled:cursor-default disabled:opacity-60";

export const hero = "flex items-center gap-4";
export const avatar = (size: "small" | "medium" | "large") =>
  `grid place-items-center rounded-full bg-accent-strong font-[650] text-white ${
    size === "large"
      ? "size-21 flex-[0_0_84px] text-[32px]"
      : size === "medium"
        ? "size-[58px] flex-[0_0_58px] text-2xl"
        : "size-[46px] flex-[0_0_46px] text-[19px]"
  }`;
export const profileCard =
  "w-full cursor-pointer border-0 text-left disabled:cursor-default disabled:opacity-100";
export const profileChevron = "ml-auto text-2xl leading-none [color:var(--p-sub)]";
export const profilePreview = "flex items-center gap-3 text-lg";
/** The same preview, given the whole page: centred and stacked rather than a row. */
export const profilePreviewLarge =
  "flex flex-col items-center justify-center gap-3 p-[22px] text-center [&>h2]:m-0";
export const profilePageHeader =
  "mb-3.5 flex items-center gap-3 [&>h2]:m-0 [&>h2]:flex-1 [&>h2]:text-center [&>h2]:text-lg [&>.secondary]:shrink-0 [&>.secondary]:grow-0 [&>.secondary]:basis-auto";
export const copyId =
  "cursor-pointer border-0 bg-transparent p-0 font-[inherit] [color:var(--p-accent-text)]";

export const modalBackdrop = "fixed inset-0 z-20 grid place-items-center bg-black/42 p-5";
export const modal =
  "flex max-h-[min(720px,90vh)] w-[min(520px,100%)] flex-col gap-3.5 overflow-auto rounded-2xl border border-edge bg-[var(--dropdown-bg)] p-[22px] text-body shadow-[0_18px_60px_rgb(0_0_0/25%)]";
export const modalHeading = "flex items-center justify-between gap-3 [&>h2]:m-0";

/** Two columns of facts, one on a phone where a value would otherwise be squeezed to a few glyphs. */
export const details =
  "m-0 grid grid-cols-2 gap-3 pt-1 max-phone:grid-cols-1 [&>div]:min-w-0 [&>div]:rounded-lg [&>div]:bg-subtle [&>div]:p-3 [&_dt]:text-xs [&_dt]:[color:var(--p-sub)] [&_dd]:mt-[5px] [&_dd]:mb-0 [&_dd]:ml-0 [&_dd]:break-anywhere [&_dd]:[color:var(--p-text)]";
export const code = "flex flex-col gap-3 pt-0.5";

// ---- the signed-in settings, drawn as the platform's grouped rows ----

export const rowInput =
  "h-7 w-[220px] max-w-full min-w-0 rounded-md border border-control-border bg-[var(--dropdown-bg)] px-2 font-[inherit] text-[13px] text-body focus:border-accent max-phone:w-[140px]";
export const rowValue = "[font-size:var(--p-row-fs)] [color:var(--p-sub)]";
export const rowButton = "secondary m-0 whitespace-nowrap";
/** The destructive action in a row: a bordered button in the danger colour, so it reads as a button beside its neighbours rather than as stray red text. */
export const rowDanger =
  "m-0 cursor-pointer whitespace-nowrap rounded-lg border border-[var(--border-color)] bg-[var(--button-secondary-bg)] px-3 py-[7px] text-danger not-disabled:hover:bg-[var(--button-secondary-hover)] disabled:cursor-default disabled:opacity-60";
export const dangerButton =
  "cursor-pointer whitespace-nowrap rounded-lg border border-danger bg-danger px-3.5 py-[7px] text-white disabled:cursor-default disabled:opacity-60";
/** Something that belongs to a group but is not a row of its own, such as a confirmation, given the row's surface and padding. */
export const rowBlock = "bg-[var(--p-row-bg)] [padding:var(--p-row-pad)]";
export const footnote = "m-0 px-1 text-xs leading-relaxed [color:var(--p-sub)]";
export const confirmation = "rounded-lg border border-edge bg-subtle p-3.5 [&>p]:mt-0 [&>p]:mb-3";

export const communityActions =
  "flex items-center justify-between gap-4 [&>div]:min-w-0 [&>.secondary]:m-0 [&>.secondary]:shrink-0 [&>.secondary]:grow-0 [&>.secondary]:basis-auto";

export const mobileMenu = "flex flex-col gap-2.5";
export const mobileMenuSummary =
  "w-fit cursor-pointer list-none rounded-lg border border-control-border px-3.5 py-[7px] text-body after:ml-2 after:text-muted after:content-['⌄'] open:after:content-['⌃'] [&::-webkit-details-marker]:hidden";
export const mobileMenuBody = "flex flex-wrap items-center gap-[9px]";

// ---- the app icon picker ----

export const iconSettings =
  "flex flex-col gap-3.5 [&>div:first-child]:flex [&>div:first-child]:flex-col [&>div:first-child]:gap-0";
export const iconGrid = "grid grid-cols-[repeat(auto-fit,minmax(132px,1fr))] gap-2.5";
export const iconCard = (selected: boolean) =>
  `flex min-w-0 flex-col items-center gap-[9px] rounded-xl border bg-subtle px-[9px] pt-3.5 pb-3 text-center text-body not-disabled:hover:border-edge-strong ${
    selected ? "border-accent shadow-[0_0_0_1px_var(--accent-color)]" : "border-edge"
  }`;
export const iconPreview =
  "grid size-17 place-items-center rounded-[17px] font-serif text-[31px] font-[650] text-white shadow-[0_2px_7px_rgba(0,0,0,0.18)]";
export const iconCopy =
  "flex min-w-0 flex-col gap-[3px] [&>strong]:text-[13px] [&>strong]:font-semibold [&>small]:min-h-[30px] [&>small]:text-[11px] [&>small]:leading-[1.4] [&>small]:text-muted";
export const iconState = (selected: boolean) =>
  `text-[11px] ${selected ? "font-semibold text-accent" : "text-muted"}`;
