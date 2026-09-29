/**
 * The settings shell and the surfaces that hang off it: the startup screen, the skin gallery, the
 * floating-toolbar editor, shortcuts, service actions, the dictionary and clipboard managers, the
 * panel launchers, and the desktop window chrome.
 */

/*
 * The window each platform draws around the pages. The `--p-*` tokens the root's `data-platform` selects (see `theme/platform-tokens.ts`) carry the colours and type the platforms share a name for; the `win:` / `mac:` / `linux:` / `hm2:` variants declared in styles.css reach the rest -- widths, paddings and the few surfaces the design colours for one platform only.
 */
export const shell =
  "flex h-full flex-col overflow-hidden bg-[var(--p-chrome)] text-[var(--p-text)] [font-family:var(--p-font)]";

// ---- the startup screen, shown while the host is still handing over preferences ----

export const startup =
  "relative flex h-full min-h-60 w-full flex-col items-center justify-center overflow-hidden bg-chrome text-body [&>h1]:mx-6 [&>h1]:mt-[18px] [&>h1]:mb-0 [&>h1]:text-lg [&>h1]:leading-[30px] [&>h1]:font-semibold [&>h1]:text-body [&>p]:m-0 [&>p]:text-sm [&>p]:leading-6 [&>p]:text-muted";
export const startupSpinner =
  "size-9 animate-startup-spin rounded-full border-[3px] border-edge border-t-accent motion-reduce:animate-none";
/** Three pulsing dots, each a third of a cycle behind the last. */
export const startupDots = "mt-[22px] flex h-2 items-center gap-2.5";
export const startupDot =
  "size-[5px] animate-startup-pulse rounded-full bg-accent motion-reduce:animate-none";
export const startupClose =
  "absolute top-[5px] right-[5px] size-8 rounded-[7px] border-0 bg-transparent p-0 text-[22px] leading-[30px] text-muted hover:bg-[var(--button-secondary-hover)] hover:text-body";

// ---- pages laid out as the design's groups of rows ----

/** The stack of `GroupList`s a page is made of. */
export const groups = "mt-6 flex flex-col gap-6";
/** A host limitation said inside a group, where a row would offer a control that does nothing. */
export const groupNote =
  "m-0 [padding:var(--p-row-pad)] [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
/** Content in a group that is not a single row: a card, an editor, a list of checks. */
export const groupBlock = "min-w-0 [padding:var(--p-row-pad)]";
/** The fixed track a slider row gives its range input, so the row's title keeps the rest of the width. */
export const sliderControl = "block w-40 max-phone:w-32";
/** Rows that belong together inside a group, such as a labelled set of them; divided the way a group divides its own rows. */
export const rowStack =
  "flex min-w-0 flex-col gap-[var(--p-row-gap)] [&>:not([hidden])~:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)]";

// ---- the theme page: its card gallery ----

export const skinIntro = "mt-0 mb-3.5 leading-relaxed text-muted";
export const externalHeading =
  "mx-1 mt-7 mb-3 flex items-start justify-between gap-[18px] [&>div]:min-w-0";
export const externalActions =
  "flex shrink-0 grow-0 basis-auto gap-2 [&>button]:mt-0.5 [&>button]:h-[30px] [&>button]:shrink-0 [&>button]:grow-0 [&>button]:basis-auto";
export const externalDirectory =
  "mt-2 inline-block max-w-[620px] rounded-[5px] bg-[var(--button-secondary-bg)] px-[7px] py-1 break-anywhere text-muted";
export const externalMeta = "mt-1 text-xs break-anywhere text-muted";
export const externalDiagnostics =
  "mx-1 mt-2.5 text-xs break-anywhere text-muted [&>summary]:cursor-pointer [&>ul]:mt-[7px] [&>ul]:mb-0 [&>ul]:pl-5";
export const externalResourceNote = "px-6 pt-0 pb-3";

/** The design lays the themes out as a grid of cards; each of these carries three live previews, so a column is wider than the design's 180px swatches. */
export const skinGrid = "grid grid-cols-[repeat(auto-fill,minmax(min(100%,320px),1fr))] gap-3";
export const skinCard = (selected: boolean) =>
  `block overflow-hidden rounded-[var(--p-group-r)] border bg-[var(--p-group-bg)] p-0 focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-accent hover:border-edge-strong ${
    selected
      ? "border-accent shadow-[0_0_0_1px_var(--accent-color),var(--card-shadow)]"
      : "border-edge shadow-card"
  }`;
/** The design's 使用中 mark on the selected card. */
export const skinCardInUse = "ml-2 text-xs font-normal text-accent";
export const skinCardHeader = "flex items-center justify-between gap-4 px-6 py-5";
export const skinCardActions = "flex shrink-0 flex-col items-end gap-2";
export const skinCardBody = "flex min-w-0 flex-col gap-[5px]";
export const skinCardTitle = "text-[15px] font-[550] text-body";
export const skinCardDescription = "text-[13px] leading-normal text-muted";
/** A switch drawn by hand, because it has to sit inside a card that is itself a button. */
export const skinSwitch = (on: boolean) =>
  `relative h-[19px] w-[38px] shrink-0 rounded-[9px] border-0 p-0 ${on ? "bg-accent" : "bg-[var(--toggle-off-bg)]"}`;
export const skinSwitchKnob = (on: boolean) =>
  `absolute top-1/2 left-px size-4 rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.3)] ${
    on ? "translate-x-5 -translate-y-1/2" : "-translate-y-1/2"
  }`;
export const skinPreviewSwitch =
  "h-6 rounded-md border border-edge bg-transparent px-2.5 text-[11px] leading-none text-muted hover:bg-[var(--button-secondary-bg)] hover:text-body";
/*
 * The preview renders the vendored candidate markup, so these reach into class names the upstream
 * skins own (`.candidate`, `.wnd-v .container`). They stay descendant selectors for that reason --
 * as arbitrary variants rather than as stylesheet rules.
 *
 * `skin-card-preview` is the `@utility` of the same name, which carries the rest of the card: the
 * candidate window, the per-skin decorations and the toolbar preview. Three stylesheets were written
 * against that hook, so it has to be on the element by that name.
 */
export const skinCardPreview =
  "skin-card-preview flex flex-col bg-[var(--skin-preview-stage-bg)] py-[9px] [&_.candidate]:max-w-full [&_.candidate]:min-w-0 [&_.candidate]:text-base [&_.wnd-v_.container]:w-fit [&_.wnd-v_.container]:max-w-full";
export const skinPreviewStage = "flex min-w-0 items-start overflow-hidden px-6 py-[9px]";

// ---- the floating toolbar editor ----

export const toolbarPreviewArea =
  "min-h-[190px] overflow-hidden bg-subtle px-6 pt-5 pb-[30px] max-phone:px-4";
export const toolbarPreviewLabel = "mb-7 text-xs text-muted";
export const toolbarPreview = (enabled: boolean) =>
  `mx-auto flex min-h-[35px] w-max max-w-full origin-center items-center gap-1.5 rounded-lg border border-white/15 bg-[#1a1a1a] px-[7px] py-1 whitespace-nowrap text-white shadow-[4px_4px_4px_rgba(0,0,0,0.3)] ${
    enabled ? "" : "opacity-45"
  }`;
export const toolbarHandle = "text-[1.1em] leading-none text-accent";
export const toolbarRequired = "border-r border-white/20 pr-[5px]";
export const toolbarItem = "rounded-[5px] bg-white/12 px-[5px] py-[3px] text-[0.7em]";
export const toolbarRequiredLabel = "ml-auto text-xs text-muted";

// ---- shortcuts ----

export const shortcutIntro = "leading-relaxed text-secondary";
/** A key chord shown as the control of a shortcut row. */
export const shortcutKey =
  "min-w-30 rounded-[5px] border border-edge bg-[var(--button-secondary-bg)] px-2 py-1 text-center font-[inherit] text-xs text-body";
export const shortcutRowDanger = "text-danger";

// ---- service actions ----

export const serviceRow =
  "flex items-center justify-between gap-4 pt-3 [&>div]:flex [&>div]:flex-wrap [&>div]:items-center [&>div]:gap-2 [&_.secondary]:mt-0 [&_[role=status]]:text-xs [&_[role=status]]:text-muted [&_[role=alert]]:text-xs [&_[role=alert]]:text-muted";
export const serviceRowDanger =
  "relative items-start [&>span]:flex [&>span]:min-w-0 [&>span]:flex-col [&>span]:gap-1.5 [&>span_label]:flex [&>span_label]:items-center [&>span_label]:gap-1.5 [&>span_label]:text-xs [&>span_label]:text-muted [&>span_label_input]:m-0";
export const serviceConfirmation =
  "mt-1 flex basis-full flex-col gap-2.5 rounded-[9px] border border-danger bg-raised p-3 [&>p]:m-0 [&>p]:leading-relaxed [&>p]:text-secondary [&>div]:flex [&>div]:flex-wrap [&>div]:gap-2 [&_.danger]:rounded-lg [&_.danger]:border [&_.danger]:border-danger [&_.danger]:bg-danger [&_.danger]:px-3 [&_.danger]:py-[7px] [&_.danger]:text-white";

export const settingsActions =
  "flex flex-wrap items-center justify-end gap-3 [&>span]:text-xs [&>span]:text-muted [&>button]:rounded-lg [&>button]:border [&>button]:border-accent-soft-border [&>button]:bg-accent-strong [&>button]:px-[18px] [&>button]:py-[7px] [&>button]:text-white";
export const settingsWarning = "mt-1.5 mb-0 text-[13px] leading-normal text-[#a2543a]";

// ---- clipboard, quick phrases, personal dictionary ----

/** The history as rows of the group it sits in, divided the way the group divides its own rows. */
export const clipboardList =
  "flex min-w-0 flex-col [&>:not([hidden])~:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)]";
export const clipboardRow =
  "flex min-h-[var(--p-row-h)] min-w-0 items-center justify-between gap-3 [padding:var(--p-row-pad)] [&_.secondary]:mt-0 [&_.secondary]:shrink-0 [&_.secondary]:grow-0 [&_.secondary]:basis-auto [&_.secondary]:px-[9px] [&_.secondary]:py-1";
export const clipboardActions = "flex shrink-0 flex-wrap justify-end gap-1.5";
/** The entry's text on one line and its pinned state and time under it, in the platform's secondary type. */
export const clipboardEntry =
  "flex min-w-0 flex-1 flex-col gap-0.5 [&>span]:overflow-hidden [&>span]:text-ellipsis [&>span]:whitespace-nowrap [&>span]:[font-size:var(--p-row-fs)] [&>span]:[color:var(--p-text)] [&>small]:[font-size:var(--p-sub-fs)] [&>small]:[color:var(--p-sub)]";
export const clipboardEmpty =
  "m-0 px-4 py-6 text-center [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";

/** A group's content that is not a row: the manager's actions, statuses, editor and results. */
export const managerBlock = "flex min-w-0 flex-col gap-3 [padding:var(--p-row-pad)]";
/** Prose at the head of a manager block, in the platform's secondary type. */
export const managerNote = "m-0 leading-relaxed [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const managerActions = "flex flex-wrap gap-1.5 [&_.secondary]:mt-0 [&_.primary]:mt-0";
export const importPreview =
  "flex max-h-70 flex-col gap-[7px] overflow-y-auto rounded-lg border border-edge bg-raised px-3 py-2.5 [&>span]:text-xs [&>span]:text-muted [&>div]:flex [&>div]:flex-col [&>div]:gap-0.5 [&>div]:border-t [&>div]:border-[var(--divider-color)] [&>div]:pt-[7px] [&>div_span]:break-anywhere [&_code]:text-[11px] [&_code]:text-muted";

export const field = "flex flex-col gap-[5px] text-xs text-secondary";
export const fieldInput =
  "min-w-25 rounded-md border border-edge bg-[var(--dropdown-bg)] px-2 py-1.5 text-body";
export const fieldSelect = `${fieldInput} min-w-[150px]`;
/** A prompt edited in place, the full width of its group. */
export const promptInput = `${fieldInput} min-h-32 w-full resize-y leading-relaxed`;
export const phraseForm = "mt-3.5 flex flex-wrap items-end gap-2.5";
export const keyHint = "text-[11px] text-muted";
export const bundledBadge =
  "rounded border border-edge px-1 py-px text-[11px] leading-none text-muted";
const listRow =
  "flex items-center justify-between gap-3 border-t border-[var(--divider-color)] pt-2 [&>span:first-child]:min-w-0 [&>span:first-child]:break-anywhere [&_.secondary]:mt-0 [&_.secondary]:px-[9px] [&_.secondary]:py-1";
export const phraseList =
  "mt-3.5 mb-0 flex max-h-[clamp(180px,calc(100vh-350px),550px)] list-none flex-col gap-2 overflow-x-hidden overflow-y-auto p-0 [scrollbar-gutter:stable]";
export const phraseListItem = listRow;
export const failures =
  "mt-3.5 rounded-lg border border-edge-strong bg-raised px-3 py-2.5 [&>p]:m-0 [&>p]:text-xs [&>p]:text-danger [&>ul]:mt-2 [&>ul]:mb-0 [&>ul]:flex [&>ul]:list-none [&>ul]:flex-col [&>ul]:gap-2 [&>ul]:p-0";
export const failureItem = `${listRow} [&>span:first-child]:flex [&>span:first-child]:flex-col [&>span:first-child]:gap-0.5 [&_small]:text-muted [&>button]:mt-0 [&>button]:px-[9px] [&>button]:py-1`;
export const empty = "text-muted";

// ---- panel launchers ----

/** A launcher's live preview, shown as the last block of its group. */
export const groupPreview =
  "min-w-0 border-t border-[var(--p-row-divider)] px-6 pt-5 pb-[30px] max-phone:px-4";
export const launchCard = "overflow-hidden p-0";
export const launchRow = "px-6 py-5";
export const openButton = "mt-0 min-w-18 shrink-0 grow-0 basis-auto";
export const panelPreview =
  "min-h-[330px] border-t border-edge bg-subtle px-6 pt-5 pb-[30px] max-phone:px-4";
export const panelPreviewLabel = "mb-[18px] text-xs text-muted";

// ---- the desktop window chrome ----

/*
 * Only Windows and Linux draw a caption in the page: macOS keeps its native traffic lights over the sidebar, and HarmonyOS 2-in-1 and the phones leave the frame to the system. Windows 11 is 48 high with the brand on the left, a search box centred on the window and 46px caption buttons; the GNOME headerbar is 46 high, with a left pane that lines up with the 240px sidebar and the page name in the middle.
 */
export const titlebar =
  "relative flex h-[var(--titlebar-height)] flex-shrink-0 items-center justify-between gap-3 bg-[var(--p-chrome)] pl-3 text-[var(--p-text)] select-none win:[--titlebar-height:48px] linux:border-b linux:border-[var(--p-hair)] linux:pl-0 linux:[--titlebar-height:46px]";
export const titlebarBrand =
  "flex min-w-0 items-center gap-2 [&>img]:size-[18px] [&>img]:shrink-0 linux:w-60 linux:shrink-0 linux:self-stretch linux:justify-center linux:border-r linux:border-[var(--p-hair)] max-phone:linux:w-auto max-phone:linux:border-r-0 max-phone:linux:px-3";
export const title =
  "min-w-0 overflow-hidden text-xs font-normal tracking-[0.2px] text-ellipsis whitespace-nowrap linux:text-[15px] linux:font-bold linux:tracking-normal";
export const titlebarSubtitle = "shrink-0 text-xs text-[var(--p-sub)]";
/** GNOME names the page in the middle of the headerbar; the page's own heading stays the accessible name, so this copy is hidden from assistive technology. */
export const titlebarPageTitle =
  "min-w-0 flex-1 overflow-hidden text-center text-[15px] font-bold text-ellipsis whitespace-nowrap";
/** The Windows 11 caption search: 360 wide, centred on the window rather than on the space left between brand and buttons, and dropped where the window is too narrow to seat it clear of both. */
export const titlebarSearch =
  "absolute top-1/2 left-1/2 flex h-8 w-[360px] -translate-x-1/2 -translate-y-1/2 items-center gap-2 rounded-[4px] border border-[rgba(255,255,255,0.07)] bg-[rgba(255,255,255,0.0605)] px-3 text-sm text-[var(--p-sub)] focus-within:shadow-[inset_0_-2px_0_var(--accent-color)] light-theme:border-[rgba(0,0,0,0.0578)] light-theme:bg-[rgba(255,255,255,0.7)] max-narrow:hidden";
export const windowControls =
  "flex flex-shrink-0 items-center gap-0.5 [&>button]:inline-flex [&>button]:h-[var(--titlebar-height)] [&>button]:w-[46px] [&>button]:cursor-default [&>button]:items-center [&>button]:justify-center [&>button]:rounded-none [&>button]:border-0 [&>button]:bg-transparent [&>button]:p-0 [&>button]:text-base [&>button]:text-inherit [&>button:hover]:bg-[var(--titlebar-btn-hover)] [&>button:active]:bg-[var(--titlebar-btn-active)] [&>button:focus-visible]:-outline-offset-[3px] linux:mr-3 linux:gap-2.5 linux:[&>button]:size-6 linux:[&>button]:rounded-full linux:[&>button]:bg-[var(--p-hover)] linux:[&>button:focus-visible]:outline-offset-2";
/*
 * Close turns red on hover, so the glyph has to stay white there. Everywhere else on a light theme it is inverted, and without this the icon would flip to dark on the red -- which is the one place the inversion is wrong. GNOME's close is a grey circle like its neighbours, so Linux takes the red, and the white glyph with it, back out.
 */
export const windowClose =
  "window-close hover:bg-[#c42b1c]! hover:text-white active:bg-[#a72216]! active:text-white light-theme:hover:[&_img]:[filter:none] light-theme:active:[&_img]:[filter:none] linux:hover:bg-[var(--titlebar-btn-hover)]! linux:active:bg-[var(--titlebar-btn-active)]! linux:light-theme:hover:[&_img]:[filter:invert(1)_brightness(0.2)] linux:light-theme:active:[&_img]:[filter:invert(1)_brightness(0.2)]";
/** The glyphs ship white, so a light theme inverts them -- except on the close button, which turns red. The filter is spelled out because Tailwind's `invert` and `brightness` utilities always compose as brightness-then-invert, which turns a white glyph light grey instead of near-black. */
export const windowIcon =
  "block size-[9px] h-2.5 object-contain [pointer-events:none] light-theme:[filter:invert(1)_brightness(0.2)] linux:size-2 linux:h-2";

/*
 * macOS has no caption of its own here: the traffic lights sit over the top of the sidebar, and the page names itself in a 52px toolbar that stays at the top of the content as it scrolls, over a blurred copy of what passes beneath. Both are drag regions. The lights keep the toolbar's left edge clear when the window is narrow enough to drop the sidebar.
 */
export const macosDragRow = "-mx-3 h-[52px] shrink-0";
export const macosToolbar =
  "sticky top-0 z-10 flex h-[52px] shrink-0 items-center gap-3.5 border-b border-[rgba(255,255,255,0.08)] bg-[rgba(31,31,33,0.82)] px-5 backdrop-blur-[20px] backdrop-saturate-[1.8] select-none light-theme:border-[rgba(0,0,0,0.07)] light-theme:bg-[rgba(242,242,244,0.82)] max-phone:pl-[88px] [&>h1]:m-0 [&>h1]:min-w-0 [&>h1]:truncate [&>h1]:text-[15px] [&>h1]:font-bold";

// ---- secret fields ----

export const secretInput = "inline-flex items-center gap-1.5";
export const secretToggle =
  "cursor-pointer rounded-md border border-current bg-transparent px-2 py-0.5 text-xs text-inherit disabled:cursor-default disabled:opacity-50";

// ---- the sidebar ----

/*
 * Its own compositing layer: without `contain` a reflow in the content beside it re-rasterises the whole column. The scrollbar is styled through the WebKit pseudo-elements, which have no utility form. Windows 11 leaves the column on the window's Mica at 280; macOS tints it at 260 behind a hairline; GNOME and HarmonyOS 2-in-1 are 240, GNOME with a hairline and HarmonyOS in one tone with the content.
 */
export const sidebar =
  "relative z-1 isolate flex w-50 shrink-0 flex-col overflow-y-auto py-3 [contain:layout_style_paint] [scrollbar-gutter:stable] [transform:translateZ(0)] [&::-webkit-scrollbar]:w-2 [&::-webkit-scrollbar-track]:bg-transparent [&::-webkit-scrollbar-thumb]:rounded-sm [&::-webkit-scrollbar-thumb]:bg-[var(--scrollbar-thumb)] [&::-webkit-scrollbar-thumb]:transition-colors [&::-webkit-scrollbar-thumb:hover]:bg-[var(--scrollbar-thumb-hover)] max-phone:hidden win:w-[280px] win:px-1 win:pt-1 mac:w-[260px] mac:border-r mac:border-[rgba(0,0,0,0.5)] mac:bg-[rgba(46,46,48,0.96)] mac:px-3 mac:pt-0 mac:pb-4 mac:light-theme:border-[rgba(0,0,0,0.08)] mac:light-theme:bg-[rgba(232,232,234,0.96)] linux:w-60 linux:border-r linux:border-[var(--p-hair)] linux:bg-[#2e2e32] linux:p-2 linux:light-theme:bg-[#ebebed] hm2:w-60 hm2:bg-[#1a1a1a] hm2:px-3 hm2:pt-1 hm2:pb-4 hm2:light-theme:bg-[#f1f3f5] ipad:w-80 ipad:border-r ipad:border-[var(--p-hair)] ipad:bg-[var(--p-bg)] ipad:px-4 ipad:pt-2 ipad:pb-5";
/** The iPad sidebar's large 设置 title, sized like a page title. */
export const sidebarTitle =
  "m-0 px-1 pt-1.5 pb-2.5 text-[length:var(--p-title-fs)] leading-tight [font-weight:var(--p-title-w)]";
export const sidebarHeader =
  "mb-2 flex items-center gap-1.5 px-3 pt-0 pb-3 [&>img]:size-[25px] [&>img]:translate-y-px [&>img]:rounded-md [&>span]:ml-[3px] [&>span]:text-[19px] [&>span]:font-medium";
/*
 * A group of sidebar items, which are the shared `NavItem`: its height, radius, padding and selected fill come from the `--p-nav-*` tokens, Windows' 3x16 accent bar included. Windows insets each item 2px by 4px and gaps its icon 16px; the rest set the space between groups (macOS 18, HarmonyOS 12, GNOME 10).
 */
export const sidebarSection = (first: boolean) =>
  `flex shrink-0 flex-col gap-0.5 win:gap-1 win:[&>button]:gap-4 ipad:gap-0 ipad:overflow-hidden ipad:rounded-[26px] ipad:bg-[var(--p-group-bg)] ${
    first
      ? ""
      : "mt-3.5 mac:mt-[18px] linux:mt-2.5 hm2:mt-3 ipad:mt-5 win:mt-1 win:border-t win:border-[rgba(255,255,255,0.0837)] win:pt-1 win:light-theme:border-[rgba(0,0,0,0.0803)]"
  }`;
/** The item glyphs ship light and are inverted on a light theme. Windows draws them at 16, macOS at 15, HarmonyOS at 17, GNOME at 18 and iPadOS at 20. */
export const sidebarGlyph =
  "block size-4 object-contain opacity-90 light-theme:[filter:invert(1)_brightness(0.25)] mac:size-[15px] linux:size-[18px] hm2:size-[17px] ipad:size-5";
/** Filters the sidebar by page name. macOS draws it as a rounded field under the traffic lights, HarmonyOS as a capsule, iPadOS as the system's grey 10pt-radius field. */
export const sidebarSearch =
  "mb-2 flex h-8 shrink-0 items-center gap-2 rounded-lg bg-[rgba(255,255,255,0.07)] px-2.5 text-[13px] text-[var(--p-sub)] focus-within:outline-2 focus-within:outline-accent light-theme:bg-[rgba(0,0,0,0.055)] hm2:mt-1 hm2:mb-1.5 hm2:h-9 hm2:rounded-[18px] hm2:bg-[rgba(255,255,255,0.08)] hm2:light-theme:bg-[rgba(0,0,0,0.05)] ipad:mb-3 ipad:h-9 ipad:rounded-[10px] ipad:bg-[rgba(118,118,128,0.24)] ipad:px-2 ipad:text-[17px] ipad:light-theme:bg-[rgba(118,118,128,0.12)]";
/** The field inside either search box; the box around it draws the focus. */
export const searchInput =
  "min-w-0 flex-1 appearance-none border-0 bg-transparent p-0 text-[var(--p-text)] outline-none [font:inherit] placeholder:text-[var(--p-sub)] focus-visible:outline-none [&::-webkit-search-decoration]:appearance-none";
export const searchGlyph = "size-[15px] shrink-0";
export const sidebarEmpty = "m-0 px-3 py-2 text-[13px] text-[var(--p-sub)]";
export const previewLabel = "mt-auto mr-5 mb-0 ml-5 pt-6 text-xs text-muted max-phone:hidden";

// ---- the content column ----

/** The row under any caption: sidebar beside content. A phone stacks the tab bar under the content instead; an iPad is a grid of the 320px sidebar and the detail over a full-width tab bar. */
export const body =
  "flex min-h-0 min-w-0 flex-1 overflow-hidden max-phone:flex-col ipad:grid ipad:grid-cols-[320px_minmax(0,1fr)] ipad:grid-rows-[minmax(0,1fr)_auto]";

/*
 * Windows 11 seats the pages on a lighter layer inset from the Mica, rounded at its top-left corner where the caption and sidebar meet it; macOS fills behind the toolbar; GNOME uses the window background; HarmonyOS 2-in-1 continues the sidebar's tone with no seam.
 */
export const content =
  "min-h-0 min-w-0 flex-1 overflow-y-auto [scrollbar-gutter:stable] win:rounded-tl-lg win:border-t win:border-l win:border-[rgba(0,0,0,0.1)] win:bg-[#1c1c1c] win:light-theme:border-[rgba(0,0,0,0.06)] win:light-theme:bg-[rgba(255,255,255,0.5)] mac:bg-[#1f1f21] mac:light-theme:bg-[#f2f2f4] linux:bg-[var(--p-bg)] hm2:bg-[#121212] hm2:light-theme:bg-[#f1f3f5] max-phone:win:rounded-none max-phone:win:border-l-0";
/** The page column: Windows 36/56/48 inside the layer, macOS 24/48/44 with no width limit, GNOME 24/32/28 up to 680, HarmonyOS 2-in-1 8/28/28 up to 760, the iPad detail 16/28/28 up to 720. */
export const contentColumn =
  "mx-auto w-full max-w-[900px] px-7 pt-3.5 pb-6 max-phone:px-3 max-phone:py-3 win:px-14 win:pt-9 win:pb-12 mac:max-w-none mac:px-12 mac:pt-6 mac:pb-11 linux:max-w-[680px] linux:px-8 linux:pt-6 linux:pb-7 hm2:max-w-[760px] hm2:px-7 hm2:pt-2 hm2:pb-7 ipad:max-w-[720px] ipad:px-7 ipad:pt-4 ipad:pb-7 max-phone:win:px-4 max-phone:win:pt-5 max-phone:mac:px-4 max-phone:linux:px-4 max-phone:hm2:px-4";
/** The page's large title, sized by the platform's `--p-title-*` tokens. macOS puts it in the toolbar instead. */
export const pageHeader = "mb-6 flex items-center gap-2.5 hm2:mb-4";
/** The way back from a sub-page (AI 辅助, 背单词, 帮助) to the page it opens from, above the title. */
export const backLink =
  "mb-1 inline-flex min-h-8 items-center self-start rounded-md border-0 bg-transparent px-0 text-[13px] text-accent hover:underline focus-visible:outline-2 focus-visible:outline-accent";
export const pageTitle =
  "m-0 text-[length:var(--p-title-fs)] leading-tight [font-weight:var(--p-title-w)]";
/**
 * The compact title bar a phone page fades in once its large title scrolls away. It sticks to the top of the scrolling content and takes no room of its own (the negative margin cancels its height). iOS and HarmonyOS centre 17/600 on a 44px blurred bar over a hairline; Android left-aligns 22/400 on a 64px surface-container bar.
 */
export const collapsedTitle = (shown: boolean) =>
  `pointer-events-none hidden h-11 items-center justify-center overflow-hidden border-b-[0.5px] border-[var(--p-hair)] bg-[rgba(18,18,20,0.8)] px-14 text-[17px] font-semibold whitespace-nowrap text-[var(--p-text)] backdrop-blur-[20px] backdrop-saturate-[1.8] transition-opacity duration-[180ms] max-phone:sticky max-phone:top-0 max-phone:z-10 max-phone:-mb-11 max-phone:flex light-theme:bg-[rgba(249,249,249,0.8)] android:-mb-16 android:h-16 android:justify-start android:border-b-0 android:bg-[#1d201d] android:px-4 android:text-[22px] android:font-normal android:[-webkit-backdrop-filter:none] android:[backdrop-filter:none] android:light-theme:bg-[#ebefe7] ${
    shown ? "opacity-100" : "opacity-0"
  }`;

// ---- the phone tab bar ----

/*
 * The four bottom tabs of a touch host, shown below phone width and, on an iPad, under the split at every width. iOS and iPadOS float them in a glass capsule clear of the screen edges; Android is Material 3's 80px navigation bar; HarmonyOS is a flat 76px bar. The bottom inset clears the gesture area on each.
 */
export const mobileTabBar =
  "hidden max-phone:order-2 max-phone:grid max-phone:shrink-0 max-phone:grid-cols-4 ios:mx-5 ios:mb-[max(0.5rem,env(safe-area-inset-bottom,0px))] ios:rounded-full ios:bg-[rgba(40,40,42,0.78)] ios:p-[5px] ios:shadow-[0_0_0_0.5px_rgba(255,255,255,0.12),0_8px_24px_rgba(0,0,0,0.5)] ios:backdrop-blur-[20px] ios:backdrop-saturate-[1.8] ios:light-theme:bg-[rgba(255,255,255,0.78)] ios:light-theme:shadow-[0_0_0_0.5px_rgba(0,0,0,0.06),0_8px_24px_rgba(0,0,0,0.12)] android:min-h-20 android:bg-[#1d201d] android:pt-3 android:pb-[env(safe-area-inset-bottom,0px)] android:light-theme:bg-[#ebefe7] harmony:min-h-[76px] harmony:bg-[#141414] harmony:pt-2 harmony:pb-[env(safe-area-inset-bottom,0px)] harmony:light-theme:bg-[#f1f3f5] ipad:col-span-2 ipad:row-start-2 ipad:mx-5 ipad:mb-[max(0.5rem,env(safe-area-inset-bottom,0px))] ipad:grid ipad:grid-cols-4 ipad:rounded-full ipad:bg-[rgba(40,40,42,0.78)] ipad:p-[5px] ipad:shadow-[0_0_0_0.5px_rgba(255,255,255,0.12),0_8px_24px_rgba(0,0,0,0.5)] ipad:backdrop-blur-[20px] ipad:backdrop-saturate-[1.8] ipad:light-theme:bg-[rgba(255,255,255,0.78)] ipad:light-theme:shadow-[0_0_0_0.5px_rgba(0,0,0,0.06),0_8px_24px_rgba(0,0,0,0.12)]";
/** A tab: the accent when selected (Android keeps the text colour and lets the indicator carry it), the secondary text colour otherwise. iOS fills the selected tab's pill. */
export const mobileTab = (selected: boolean) =>
  `flex min-w-0 cursor-pointer flex-col items-center border-0 bg-transparent p-0 font-medium ios:gap-px ios:rounded-full ios:pt-[5px] ios:pb-1 ios:text-[10px] android:gap-1 android:text-xs harmony:gap-0.5 harmony:text-[10px] ipad:gap-px ipad:rounded-full ipad:pt-[5px] ipad:pb-1 ipad:text-[10px] ${
    selected
      ? "text-accent android:font-semibold android:text-[var(--p-text)] ios:bg-[rgba(255,255,255,0.1)] ios:light-theme:bg-[rgba(0,0,0,0.06)]"
      : "text-[var(--p-sub)]"
  }`;
/** The box behind the glyph. Android's is the 64x32 Material 3 active indicator. */
export const mobileTabPill = (selected: boolean) =>
  `flex h-[26px] w-8 items-center justify-center rounded-2xl android:h-8 android:w-16 harmony:h-7 ${
    selected ? "android:bg-accent-soft" : ""
  }`;
/** The glyph, masked out of the text colour; `--tab-icon` carries the image. HarmonyOS draws it at 24, the others at 22. */
export const mobileTabIcon =
  "block size-[22px] bg-current [mask-image:var(--tab-icon)] [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain] [-webkit-mask-image:var(--tab-icon)] [-webkit-mask-position:center] [-webkit-mask-repeat:no-repeat] [-webkit-mask-size:contain] harmony:size-6";
