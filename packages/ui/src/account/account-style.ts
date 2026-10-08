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
/** The edit dialog's avatar, which opens the file dialog: round like the avatar it holds, with a focus ring and no button chrome. */
export const avatarButton =
  "m-0 cursor-pointer rounded-full border-0 bg-transparent p-0 not-disabled:hover:opacity-85 disabled:cursor-default focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";
export type AvatarSize = "small" | "medium" | "large" | "card" | "hero";
/** `card`（56px）和 `hero`（84px）用于 HarmonyOS 手机的 我的 卡片和 个人资料 页头：底色就是强调色本身，文字用它对应的前景色，这样季节的深色强调色上首字母仍然清晰可读。 */
export const avatar = (size: AvatarSize) =>
  size === "card"
    ? "grid size-14 flex-[0_0_56px] place-items-center rounded-full text-[22px] font-bold [background:var(--accent-color)] [color:var(--p-on-accent)]"
    : size === "hero"
      ? "grid size-21 flex-[0_0_84px] place-items-center rounded-full text-[34px] font-bold [background:var(--accent-color)] [color:var(--p-on-accent)]"
      : `grid place-items-center rounded-full bg-accent-strong font-[650] text-white ${
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

// ---- HarmonyOS 手机上的 我的 ----

/** 标签页的纵向堆叠：外壳在它上方画 30px 的 我的 标题，卡片与各分组之间间隔 18px。 */
export const mePage = "flex flex-col gap-[18px]";
/** 个人资料卡片：未登录时点击登录，已登录时打开 个人资料。 */
export const meCard =
  "m-0 flex w-full min-w-0 cursor-pointer items-center gap-3.5 rounded-[20px] border-0 bg-[var(--p-group-bg)] p-4 text-left [font-family:inherit] [color:var(--p-text)] active:bg-[var(--p-press)] disabled:cursor-default";
/** 未登录卡片的头像：分段控件灰底上的一个问号。 */
export const meCardAnonymous =
  "grid size-14 flex-[0_0_56px] place-items-center rounded-full bg-[var(--p-seg-bg)] text-[22px] font-bold [color:var(--p-sub)]";
export const meCardText = "flex min-w-0 flex-1 flex-col gap-[3px]";
export const meCardName = "truncate text-[18px] font-bold";
export const meCardSubtitle = "truncate text-[13px] [color:var(--p-sub)]";
export const meCardChevron = "shrink-0 opacity-55 [color:var(--p-sub)]";
export const meGroup = "flex min-w-0 flex-col gap-[7px]";
export const meGroupTitle = "m-0 px-1 text-[14px] font-medium [color:var(--p-sub)]";
export const meFooter = "m-0 px-4 text-center text-xs leading-relaxed [color:var(--p-sub)]";

// ---- 我的 的子页面（个人资料、其他平台下载） ----

/**
 * 在 我的 里推入的页面自带 44px 的页头（'‹ 我的' 和居中的标题），所以页面显示期间，外壳在它上方的 我的 大标题会移出布局。大标题仍留在无障碍树中，为 `main` 命名。外壳用 `data-tab-page` 标记标签页的内容列，并把自己的页头作为直接子元素放在里面。
 */
export const subpage = "flex flex-col gap-5 [[data-tab-page]:has(&)>header]:sr-only";
export const subpageHeader = "relative flex h-11 items-center";
export const subpageBack =
  "relative z-[1] m-0 -ml-1.5 flex h-11 cursor-pointer items-center gap-1 border-0 bg-transparent px-1.5 text-[17px] [font-family:inherit] [color:var(--p-accent-text)] active:opacity-60 disabled:cursor-default disabled:opacity-50";
export const subpageTitle =
  "pointer-events-none absolute inset-x-[88px] m-0 truncate text-center text-[17px] font-semibold [color:var(--p-text)]";

// ---- 个人资料 ----

export const profileHero = "flex flex-col items-center gap-2 pt-2 pb-0.5 text-center";
export const profileHeroName =
  "m-0 max-w-full truncate text-[22px] font-bold [color:var(--p-text)]";
export const profileHeroEmail = "max-w-full truncate text-[14px] [color:var(--p-sub)]";
export const profileHeroPill =
  "rounded-full bg-[var(--accent-soft)] px-2.5 py-[3px] text-xs [color:var(--p-accent-text)]";
export const profileGroup = "flex min-w-0 flex-col gap-1.5";
export const profileGroupTitle = "m-0 px-4 text-[13px] font-normal [color:var(--p-sub)]";
export const profileGroupFooter = "m-0 px-4 text-xs leading-relaxed [color:var(--p-sub)]";
export const profileRows =
  "overflow-hidden rounded-[20px] bg-[var(--p-group-bg)] [&>*+*]:[box-shadow:inset_0_1px_0_var(--p-hair)]";
export const profileRow =
  "m-0 flex min-h-[50px] w-full min-w-0 items-center gap-3 border-0 bg-transparent px-4 text-left [font-family:inherit] [color:var(--p-text)]";
export const profileRowButton =
  "cursor-pointer active:bg-[var(--p-press)] disabled:cursor-default disabled:opacity-50";
export const profileRowLabel = "shrink-0 text-[16px]";
export const profileRowValue =
  "min-w-0 flex-1 truncate text-right text-[15px] [color:var(--p-sub)]";
export const profileRowChevron = "shrink-0 opacity-55 [color:var(--p-sub)]";
/** 设计稿里的破坏性操作：一整行居中的红色文字。 */
export const profileDangerRow =
  "m-0 flex min-h-[50px] w-full cursor-pointer items-center justify-center border-0 bg-transparent px-4 text-[16px] [font-family:inherit] [color:#FF3B30] active:bg-[var(--p-press)] disabled:cursor-default disabled:opacity-50";
export const profileConfirmation = "[&>div]:m-0";

// ---- 底部弹窗（登录水杉、昵称） ----

export const sheetScrim =
  "fixed inset-0 z-50 animate-ms-fade-in bg-[rgba(0,0,0,0.35)] motion-reduce:animate-none";
export const sheet =
  "fixed inset-x-0 bottom-0 z-50 flex max-h-[92vh] flex-col gap-3 overflow-y-auto overscroll-contain rounded-t-[20px] bg-[var(--p-bg)] px-5 pt-2 pb-[calc(34px+env(safe-area-inset-bottom))] [font-family:var(--p-font,inherit)] [color:var(--p-text)] animate-ms-sheet-up motion-reduce:animate-none";
export const sheetGrabber = "mx-auto h-[5px] w-9 shrink-0 rounded-full bg-[var(--p-hair)]";
export const sheetHeader = "flex items-start gap-3 pt-2";
export const sheetHeading = "flex min-w-0 flex-1 flex-col gap-1.5";
export const sheetTitle = "m-0 text-[22px] font-bold [color:var(--p-text)]";
export const sheetSubtitle = "m-0 text-[14px] leading-snug [color:var(--p-sub)]";
export const sheetClose =
  "m-0 flex size-[30px] shrink-0 cursor-pointer items-center justify-center rounded-full border-0 bg-[var(--p-seg-bg)] p-0 [color:var(--p-text)] active:opacity-70 disabled:cursor-default disabled:opacity-50";
export const sheetBody = "flex flex-col gap-3";
/** 登录选项：用强调色浅色填充的 50px 色调按钮。 */
export const sheetChoice =
  "m-0 h-[50px] w-full cursor-pointer rounded-xl border-0 bg-[var(--accent-soft)] px-4 text-[17px] font-semibold [font-family:inherit] [color:var(--p-accent-text)] active:opacity-80 disabled:cursor-default disabled:opacity-60 aria-pressed:shadow-[inset_0_0_0_1.5px_var(--accent-color)]";
/** 这一步的主操作：可以执行时用强调色，在那之前是分段控件的灰底配灰色文字。 */
export const sheetPrimary =
  "m-0 h-[50px] w-full cursor-pointer rounded-xl border-0 px-4 text-[17px] font-semibold [font-family:inherit] bg-[var(--accent-color)] [color:var(--p-on-accent)] active:opacity-80 disabled:cursor-default disabled:bg-[var(--p-seg-bg)] disabled:[color:var(--p-sub)]";
export const sheetInput =
  "h-[50px] w-full rounded-xl border border-[var(--p-hair)] bg-[var(--p-group-bg)] px-3.5 text-[17px] [font-family:inherit] [color:var(--p-text)] outline-none placeholder:[color:var(--p-sub)] focus:border-[var(--accent-color)] disabled:opacity-60";
export const sheetField = "flex flex-col gap-1.5";
export const sheetHint = "m-0 text-[13px] leading-snug [color:var(--p-sub)]";
export const sheetHintError = "m-0 text-[13px] leading-snug text-danger";
export const sheetFooter = "m-0 text-center text-xs [color:var(--p-sub)]";
export const sheetFooterLink =
  "m-0 cursor-pointer border-0 bg-transparent p-0 text-xs [font-family:inherit] [color:var(--p-sub)] underline-offset-2 active:underline";
export const sheetQuiet = "flex flex-wrap items-center justify-center gap-x-4 gap-y-1.5";

// ---- 其他平台下载 ----

export const downloadHero =
  "flex min-w-0 items-center gap-3.5 rounded-[20px] bg-[var(--accent-soft)] p-4 [color:var(--p-text)]";
export const downloadHeroTile =
  "flex size-11 flex-none items-center justify-center rounded-xl [background:var(--accent-color)] [color:var(--p-on-accent)]";
export const downloadHeroText = "flex min-w-0 flex-1 flex-col gap-0.5";
export const downloadHeroTitle = "text-[15px] font-semibold";
export const downloadHeroUrl = "truncate text-[13px] [color:var(--p-sub)]";
export const downloadPill =
  "m-0 shrink-0 cursor-pointer whitespace-nowrap rounded-full border-0 px-3.5 py-1.5 text-[13px] font-semibold [font-family:inherit] [background:var(--accent-color)] [color:var(--p-on-accent)] active:opacity-80";
export const downloadRows = "overflow-hidden rounded-[20px] bg-[var(--p-group-bg)]";
export const downloadRow =
  "m-0 flex w-full min-w-0 items-center gap-3 border-x-0 border-t border-b-0 border-solid border-[var(--p-hair)] bg-transparent px-3.5 py-3 text-left [font-family:inherit] [color:var(--p-text)] first:border-t-0";
export const downloadRowButton = "cursor-pointer active:bg-[var(--p-press)]";
export const downloadTile =
  "flex size-9 flex-none items-center justify-center rounded-[9px] bg-[var(--p-seg-bg)] [color:var(--p-text)]";
export const downloadRowText = "flex min-w-0 flex-1 flex-col gap-0.5";
export const downloadRowName = "text-[15px] font-semibold";
export const downloadRowMeta = "text-xs [color:var(--p-sub)]";
export const downloadRowPill = (current: boolean) =>
  `shrink-0 whitespace-nowrap rounded-full px-3 py-[5px] text-[13px] font-semibold ${
    current
      ? "bg-[var(--p-seg-bg)] [color:var(--p-sub)]"
      : "bg-[var(--accent-soft)] [color:var(--p-accent-text)]"
  }`;
