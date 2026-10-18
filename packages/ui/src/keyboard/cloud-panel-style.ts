/**
 * The two cloud panels' utility strings.
 *
 * They are separate windows rather than settings pages, which is why the clipboard panel carries its
 * own fixed palette (it sits beside the emoji panel and has to match the host's panel chrome) while
 * the dictionary panel uses the app's theme variables. Keeping the two sets apart here makes that
 * difference visible instead of leaving it to be rediscovered from hex codes further down a file.
 */

import { compactFieldLabel } from "../core/form-style";
import { compactMutedNote } from "../core/text-style";
import { searchRow as sharedSearchRow } from "../core/search-style";

// ---- cloud clipboard: its own dark chrome, like the emoji panel beside it ----

const clipText = "text-[#f5f5f7]";
const clipMuted = "text-[#aeb0b7]";
const clipField = "rounded-[7px] border border-[#45454f] bg-[#2b2b33]";

export const clipboardPanel = `min-h-screen bg-[#202027] ${clipText}`;
export const clipboardHeader = `border-b-[#45454f] bg-[#202027] ${clipText} [&>button]:text-[#aeb0b7]`;
export const clipboardBody = "flex min-h-[calc(100vh-38px)] flex-col gap-3 px-6 pt-5 pb-4";
export const clipboardNote = `m-0 text-xs ${clipMuted}`;
export const clipboardToggle = `flex items-center justify-between px-3 py-2.5 ${clipField}`;
export const clipboardSearchRow = sharedSearchRow;
export const clipboardInput = `min-w-0 flex-1 p-[9px] font-[inherit] ${clipField} ${clipText}`;
/** A quiet button in this panel; the primary one below overrides the colours. */
export const clipboardButton = `rounded-md border-0 bg-[#3a3945] px-3 py-2 ${clipText}`;
export const clipboardAddRow = "flex items-end gap-2";
export const clipboardTextArea = `${clipboardInput} resize-y`;
export const clipboardSubmit =
  "rounded-md border-0 bg-[#5fbf84] px-3 py-2 whitespace-nowrap text-[#241c26]";
/**
 * The character counter beside the upload box. It swapped between two class names to signal the
 * over-length state and neither had a rule, so the count read the same at 3,999 and at 4,001 while
 * the upload button silently disabled itself. The warning colour is the point of the swap.
 */
export const clipboardCount = (overLimit: boolean) =>
  `shrink-0 grow-0 basis-auto self-center text-xs tabular-nums ${
    overLimit ? "font-semibold text-[#e7b4bb]" : clipMuted
  }`;
/** The delete-everything confirmation. It had no styling at all -- three bare elements in a row. */
export const clipboardConfirm = `flex flex-wrap items-center gap-2 rounded-[7px] border border-[#e7b4bb] bg-[#2b2b33] p-3 text-xs ${clipText}`;
export const clipboardList = "min-h-0 flex-1 overflow-y-auto";
/** A row, with its first child being the text itself: a full-width, wrapping, quiet button. */
export const clipboardItem =
  "flex items-start gap-2 border-b border-white/[0.09] py-2.5 [&>button:first-child]:flex-1 [&>button:first-child]:border-0 [&>button:first-child]:bg-transparent [&>button:first-child]:p-0 [&>button:first-child]:text-left [&>button:first-child]:whitespace-pre-wrap [&>button:first-child]:wrap-anywhere [&>button:first-child]:text-[#f5f5f7]";
export const clipboardDelete =
  "shrink-0 grow-0 basis-auto rounded-md border-0 bg-[#3a3945] px-2 py-1 text-[#e7b4bb]";
/** 端到端加密的设备区块：本机待允许的说明、等待允许的设备和设备列表，与开关同一种底框。 */
export const clipboardSection = `flex flex-col gap-2 p-3 text-xs ${clipField} ${clipText}`;
/** 等待允许的设备是需要用户处理的提示，用绿色描边与普通区块区分开。 */
export const clipboardAttention = `flex flex-col gap-2 rounded-[7px] border border-[#5fbf84] bg-[#2b2b33] p-3 text-xs ${clipText}`;
export const clipboardSectionTitle = "m-0 text-[13px] font-semibold";
/** 核对码：等宽、加大，两台设备上要逐位对照。 */
export const clipboardCode =
  "m-0 font-mono text-lg tracking-[0.08em] tabular-nums text-[#f5f5f7] select-all";
export const clipboardDevice =
  "flex flex-wrap items-center gap-2 border-t border-white/[0.09] pt-2 first-of-type:border-t-0 first-of-type:pt-0";
export const clipboardDeviceInfo = "flex min-w-0 flex-1 flex-col gap-0.5 [&>small]:text-[#aeb0b7]";
export const clipboardActions = "flex flex-wrap gap-2";

// ---- cloud dictionary: the app's own palette, because it reads as a settings surface ----

export const dictionaryPanel = "min-h-screen bg-chrome text-body";
export const dictionaryBody =
  "flex min-h-[calc(100vh-38px)] flex-col gap-3.5 overflow-y-auto px-6 py-[22px]";
export const dictionaryNote = compactMutedNote;
/** A labelled field: caption above, control below, as every toolbar and form row here is built. */
export const dictionaryField = compactFieldLabel;
export const dictionaryInput =
  "min-w-[120px] rounded-[7px] border border-edge bg-[var(--dropdown-bg)] px-[9px] py-[7px] text-body";
export const dictionaryButton =
  "rounded-lg border border-edge bg-[var(--button-secondary-bg)] px-3 py-[7px] text-body not-disabled:hover:bg-[var(--button-secondary-hover)]";
export const dictionaryToolbar = "flex flex-wrap items-end gap-2.5";
/** 设置页内嵌查询的输入控件等高，操作独占下一行并自动换行。 */
export const dictionaryLiveToolbar =
  "grid min-w-0 grid-cols-[minmax(0,1fr)_minmax(0,2.4fr)] items-end gap-4 max-phone:grid-cols-1 [&_input]:h-9 [&_input]:min-w-0 [&_select]:h-9 [&_select]:w-full [&_select]:rounded-md [&_select]:border [&_select]:border-edge [&_select]:bg-[var(--dropdown-bg)] [&_select]:px-2";
export const dictionaryLiveActions =
  "col-span-full flex min-w-0 flex-wrap gap-2 border-t border-edge pt-3 [&>button]:mt-0 [&>button]:h-8 [&>button]:rounded-md [&>button]:border [&>button]:border-edge [&>button]:bg-raised [&>button]:px-3 [&>button]:text-xs";
/** The width query that decides the search field lives here rather than in a stylesheet. */
const dictionaryFlexibleField = "flex-[1_1_220px] [&>input]:w-full";
export const dictionarySearch = dictionaryFlexibleField;
export const dictionaryActions =
  "flex items-center gap-2 [&>.secondary]:mt-0 [&>.secondary]:cursor-pointer";

/** The phone's kind switch. It replaces a select that is hidden at the same width. */
export const dictionaryKindTabs =
  "hidden max-phone:grid max-phone:grid-cols-4 max-phone:gap-[3px] max-phone:rounded-[9px] max-phone:bg-subtle max-phone:p-[3px]";
export const dictionaryKindTab = (active: boolean) =>
  `min-h-[34px] min-w-0 overflow-hidden rounded-[7px] border-0 px-[3px] py-[5px] text-ellipsis whitespace-nowrap ${
    active ? "bg-raised text-body shadow-card" : "bg-transparent text-secondary"
  }`;
/** Hidden on a roomy window, where the toolbar's own select says the same thing. */
export const dictionaryDesktopOnlyField = "max-phone:hidden";
export const dictionaryMobileHint = "m-0 hidden text-xs text-muted max-phone:block";

export const dictionarySection =
  "flex flex-wrap items-center gap-2.5 rounded-[9px] border border-edge bg-raised p-3.5 [&>h2]:m-0 [&>h2]:flex-[1_0_100%] [&>h2]:text-sm [&>h2]:font-semibold [&>p]:flex-[1_0_100%]";
/** A section's own buttons, including the label that stands in for a file input. */
export const dictionarySectionControl =
  "cursor-pointer rounded-lg border border-edge bg-[var(--button-secondary-bg)] px-3 py-[7px] text-body hover:bg-[var(--button-secondary-hover)] not-disabled:hover:bg-[var(--button-secondary-hover)]";
export const dictionaryFilePreview =
  "flex flex-[1_0_100%] flex-wrap items-center gap-2 border-t border-[var(--divider-color)] pt-2 [&>strong]:flex-auto [&>strong]:wrap-anywhere [&>small]:text-muted";
export const dictionaryFileExcerpt =
  "m-0 max-h-[180px] flex-[1_0_100%] overflow-auto rounded-md bg-subtle p-2 font-mono text-[12px]/[1.45] whitespace-pre-wrap wrap-anywhere text-secondary";
export const dictionarySnapshotFact =
  "flex min-w-[180px] flex-[1_1_220px] flex-col gap-[3px] text-body [&>small]:wrap-anywhere [&>small]:text-muted";

export const dictionaryForm =
  "flex flex-wrap items-end gap-2.5 rounded-[9px] border border-edge bg-raised p-3.5";
export const dictionaryWordField = dictionaryFlexibleField;
export const dictionaryList = "flex min-h-[180px] flex-col gap-2";
/** A row stacks on a phone, where a word and three actions do not fit side by side. */
export const dictionaryItem =
  "flex items-center justify-between gap-3 border-t border-[var(--divider-color)] py-2.5 max-phone:flex-col max-phone:items-stretch max-phone:gap-2 [&_strong]:block [&_strong]:font-medium [&_strong]:wrap-anywhere [&_small]:mt-[3px] [&>.secondary]:mt-0 [&>.secondary]:px-[9px] [&>.secondary]:py-[5px]";
export const dictionaryItemMain =
  "block min-w-0 flex-auto border-0 bg-transparent p-0 text-left text-inherit not-disabled:hover:[&_strong]:underline focus-visible:[&_strong]:underline max-phone:w-full";
export const dictionaryItemStatic = "cursor-default";
export const dictionaryItemActions =
  "flex shrink-0 gap-1.5 max-phone:w-full max-phone:flex-wrap [&>button]:max-phone:min-h-[34px] [&>button]:max-phone:flex-[1_1_96px]";
export const dictionaryEmpty = "m-auto text-muted";
export const dictionaryPagination = "flex items-center justify-center gap-4 text-xs text-secondary";
