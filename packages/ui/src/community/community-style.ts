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
  "flex shrink-0 grow-0 basis-auto flex-col items-end gap-2 max-tight:flex-row max-tight:flex-wrap max-tight:items-center max-tight:justify-start [&>button]:m-0 [&>button]:whitespace-nowrap [&>.primary]:rounded-lg [&>.primary]:border [&>.primary]:border-accent-soft-border [&>.primary]:bg-accent-strong [&>.primary]:px-3 [&>.primary]:py-[7px] [&>.primary]:text-white harmony:[&>.primary]:[color:var(--p-on-accent)] hm2:[&>.primary]:[color:var(--p-on-accent)]";
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

/** `danger` has no shared style of its own, and the shared `secondary` carries a 12px top margin; without these the confirming button falls back to the bare button look and sits higher than 取消, as settings' `serviceConfirmation` already handles. */
export const confirmation =
  "flex flex-col gap-3 rounded-[10px] border border-danger bg-raised p-3.5 [&>p]:m-0 [&>p]:leading-relaxed [&>p]:text-secondary [&>div]:flex [&>div]:flex-wrap [&>div]:items-center [&>div]:gap-2 [&_.secondary]:mt-0 [&_.danger]:rounded-lg [&_.danger]:border [&_.danger]:border-danger [&_.danger]:bg-danger [&_.danger]:px-3 [&_.danger]:py-[7px] [&_.danger]:text-white";
export const confirmationActions = "flex flex-wrap items-center gap-2";
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
/** The desktop community's two galleries, candidate-window skins and plugin packs: the same strip as `categoryTabs` with two columns. */
export const categoryTabsPair =
  "grid grid-cols-2 gap-[3px] rounded-[9px] bg-subtle p-[3px] [&>button]:min-h-[34px] [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-transparent [&>button]:text-secondary [&>button[aria-selected=true]]:bg-raised [&>button[aria-selected=true]]:text-body [&>button[aria-selected=true]]:shadow-card";
/** The kind filter above the plugin gallery. */
export const kindFilter = "flex flex-wrap gap-1.5 [&>button]:m-0 [&>button]:whitespace-nowrap";

/*
 * HarmonyOS 手机的「社区」标签页（`look="harmony"`）。设计稿绘制的图库不带桌面端外框：胶囊分段控件、胶囊搜索框、可横向滚动的标签行、无边框的 20px 卡片，以及不打开条目就能直接操作的 tonal 胶囊。下面每个类都只用于这种外观，其他宿主沿用上面那套图库样式，保持不变。
 */

/** 标签页的纵向堆叠：分段控件、搜索框、标签行和列表之间各隔 16px。 */
export const harmonyPage = "flex min-w-0 flex-col gap-4";
/** 「皮肤 | 词库 | 短语」三段控件：通栏胶囊轨道，选中项上浮起一个胶囊。 */
export const harmonySegments =
  "grid grid-cols-3 gap-0.5 rounded-full bg-[var(--p-seg-bg)] p-0.5 [&>button]:m-0 [&>button]:rounded-full [&>button]:border-0 [&>button]:bg-transparent [&>button]:py-1.5 [&>button]:text-[13px] [&>button]:[color:var(--p-sub)] [&>button[aria-selected=true]]:bg-[var(--p-seg-on)] [&>button[aria-selected=true]]:[color:var(--accent-color)] [&>button[aria-selected=true]]:shadow-[var(--p-seg-on-shadow)]";
/** 单个胶囊搜索框，按 Enter 提交；设计稿没有单独的「搜索」按钮。 */
export const harmonySearch =
  "box-border h-10 w-full min-w-0 rounded-full border-0 bg-[var(--p-seg-bg)] px-4 font-[inherit] text-[14px] [color:var(--p-text)] outline-none placeholder:[color:var(--p-sub)] focus-visible:outline-2 focus-visible:outline-offset-0 focus-visible:outline-[var(--accent-color)]";
/** 搜索框下方的标签行。它横向滚动而不换行，手机上所有标签都保持在一行。 */
export const harmonyChips =
  "flex min-w-0 items-center gap-2 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden";
export const harmonyChip =
  "m-0 shrink-0 rounded-full border-0 bg-[var(--p-seg-bg)] px-3 py-1.5 text-[13px] whitespace-nowrap [color:var(--p-sub)] aria-pressed:bg-accent-soft aria-pressed:font-semibold aria-pressed:[color:var(--accent-color)]";
/** 标签行末尾的范围与发布入口：纯强调色文字，比标签更低调。 */
export const harmonyChipAction =
  "m-0 shrink-0 rounded-full border-0 bg-transparent px-2 py-1.5 text-[13px] whitespace-nowrap [color:var(--accent-color)] aria-pressed:font-semibold";
export const harmonyChipDivider = "h-4 w-px shrink-0 bg-[var(--p-hair)]";
/** 列表下方的「加载更多」，做成低调的文字按钮。 */
export const harmonyMore =
  "m-0 self-center rounded-full border-0 bg-transparent px-4 py-2 text-[13px] [color:var(--accent-color)]";
export const harmonyNotice = "m-0 py-6 text-center text-[13px] [color:var(--p-sub)]";

export const harmonyGrid = "grid grid-cols-2 gap-3";
/** 皮肤卡片：在分组底色上无边框、无阴影。点整张卡片打开详情；卡片底部的胶囊直接执行操作，不打开详情。 */
export const harmonySkinCard =
  "flex min-w-0 cursor-pointer flex-col overflow-hidden rounded-[20px] bg-[var(--p-group-bg)] text-left [color:var(--p-text)]";
/** 预览距卡片顶部和两侧各 8px，自身上方两角为圆角，让键盘看起来像卡片里的一块屏幕。它绘制手机缩略图（`ScreenKeyboardPreview thumbnail`），即设计稿中工具栏加四行按键的键盘。 */
export const harmonySkinStage = "block px-2 pt-2";
export const harmonySkinStageClip =
  "block overflow-hidden rounded-t-lg bg-[var(--skin-preview-stage-bg)]";
export const harmonySkinFooter = "flex min-w-0 items-center gap-2 px-2.5 pt-2.5 pb-3";
/** 卡片底部的文字列。它是卡片上可获得焦点的控件，键盘或读屏器可以借它像点击卡片那样打开详情。 */
export const harmonySkinText =
  "m-0 flex min-w-0 flex-1 cursor-pointer flex-col gap-0.5 border-0 bg-transparent p-0 text-left font-[inherit] [color:inherit]";
export const harmonySkinName = "truncate text-[14px] font-semibold [color:var(--p-text)]";
export const harmonySkinMeta = "truncate text-[11px] [color:var(--p-sub)]";
/** 皮肤的「获取 / 使用 / 使用中」胶囊；颜色来自 `harmonyPillTonal` 或 `harmonyPillDone`。 */
export const harmonySkinPill =
  "m-0 shrink-0 rounded-full border-0 px-3 py-1 text-[12px] font-semibold whitespace-nowrap";
/** 仍有操作可做的胶囊（获取、使用、添加）；操作进行中时变暗。 */
export const harmonyPillTonal = "bg-accent-soft [color:var(--accent-color)] disabled:opacity-60";
/** 没有操作可做后胶囊停留的状态：皮肤上是「使用中」，词库或回复模板上是「已添加」。它处于禁用状态，但不变暗。 */
export const harmonyPillDone = "bg-[var(--p-seg-bg)] [color:var(--p-sub)] disabled:opacity-100";

/** 词库和回复模板列表：一张由多行组成的分组卡片。 */
export const harmonyList =
  "flex min-w-0 flex-col overflow-hidden rounded-[20px] bg-[var(--p-group-bg)]";
export const harmonyRow =
  "flex min-w-0 cursor-pointer items-center gap-3 border-t border-[var(--p-hair)] px-3.5 py-3 first:border-t-0";
/** 行首徽标：tonal 底色方块上显示名称的首字。 */
export const harmonyRowTile =
  "grid size-10 shrink-0 place-items-center rounded-[10px] bg-accent-soft text-[16px] font-bold [color:var(--accent-color)]";
export const harmonyRowText =
  "m-0 flex min-w-0 flex-1 cursor-pointer flex-col gap-0.5 border-0 bg-transparent p-0 text-left font-[inherit] [color:inherit]";
export const harmonyRowName = "truncate text-[15px] font-semibold [color:var(--p-text)]";
export const harmonyRowMeta = "text-[12px] [color:var(--p-sub)]";
export const harmonyRowDescription = "truncate text-[12px] [color:var(--p-sub)]";
/** 行的「添加 / 已添加」胶囊；颜色来自 `harmonyPillTonal` 或 `harmonyPillDone`。 */
export const harmonyRowPill =
  "m-0 shrink-0 rounded-full border-0 px-3.5 py-[5px] text-[13px] font-semibold whitespace-nowrap";
/** 「短语」分段里回复模板上方的分节标题。Android 用 14px 顶部内边距把它和上方的短语包隔开；本宿主没有短语包，它直接位于标签行之下，堆叠间距已经留出了这段空间。 */
export const harmonySectionTitle =
  "m-0 px-5 pb-2 text-[14px] font-medium [color:var(--accent-color)]";
