/**
 * The touch-keyboard scheme list, skin picker and skin editor.
 *
 * All three share one shape -- a preview beside a caption, selectable -- so the pieces are named once
 * and the three call sites differ only in what they put in the preview column.
 */

// ---- the scheme list on the input page ----

export const schemeSelect = (chosen: boolean) =>
  `flex w-full min-w-0 items-center justify-between gap-3 rounded-md border-0 bg-transparent px-1 py-1.5 text-left font-[inherit] not-disabled:hover:bg-[var(--button-secondary-bg)] focus-visible:bg-[var(--button-secondary-bg)] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent-soft-border disabled:cursor-not-allowed disabled:text-muted disabled:opacity-58 ${
    chosen ? "font-semibold text-accent" : "text-body"
  }`;

// ---- the skin picker ----

export const skinGrid = "grid grid-cols-2 gap-2.5 max-phone:grid-cols-1";
export const skinCard = (selected: boolean) =>
  `overflow-hidden rounded-[9px] border bg-card transition-[border-color,box-shadow] duration-150 hover:border-edge-strong ${
    selected ? "border-accent shadow-[0_0_0_1px_var(--accent-color)]" : "border-edge"
  }`;
/** Preview, caption, tick. The preview column is fixed so the captions line up down the grid. */
export const skinCardButton =
  "grid w-full cursor-pointer grid-cols-[96px_minmax(0,1fr)_20px] items-center gap-3 border-0 bg-transparent p-2.5 text-left font-[inherit] text-body focus-visible:outline-2 focus-visible:-outline-offset-[3px] focus-visible:outline-accent";
export const skinCardCopy =
  "flex min-w-0 flex-col gap-[3px] [&>strong]:text-sm [&>strong]:font-semibold [&>small]:overflow-hidden [&>small]:text-ellipsis [&>small]:whitespace-nowrap [&>small]:text-muted";
export const skinCardCheck = "text-center text-[17px] font-bold text-accent";

// ---- the skin editor ----

export const editor = "flex flex-col gap-3.5";
/** Heading and its actions stack on a phone, where the buttons need the full width to stay tappable. */
export const editorHeading = "flex items-start justify-between gap-4 max-phone:flex-col";
export const editorHeadingActions =
  "flex shrink-0 grow-0 basis-auto gap-2 max-phone:w-full [&>button]:m-0 [&>button]:max-phone:flex-1";
/** The editor's own primary and destructive buttons, filled rather than outlined. */
export const editorFilled =
  "[&_.primary]:rounded-lg [&_.primary]:border [&_.primary]:border-accent [&_.primary]:bg-accent [&_.primary]:px-3 [&_.primary]:py-[7px] [&_.primary]:text-white [&_.danger]:rounded-lg [&_.danger]:border [&_.danger]:border-danger [&_.danger]:bg-danger [&_.danger]:px-3 [&_.danger]:py-[7px] [&_.danger]:text-white";
export const editorTabs =
  "grid grid-cols-5 overflow-hidden rounded-[9px] border border-edge bg-subtle";
export const editorTab = (selected: boolean) =>
  `min-h-11 border-0 border-r border-edge last:border-r-0 ${
    selected ? "bg-accent-soft font-[650] text-accent" : "bg-transparent text-muted"
  }`;
export const editorControls = "flex flex-col gap-3";
export const controlBlock = "rounded-[10px] border border-edge bg-subtle p-3.5";
export const controlTitle = "mb-2.5 font-semibold";

export const backgroundGrid =
  "grid grid-cols-3 gap-2 [&>button]:flex [&>button]:min-h-16 [&>button]:items-end [&>button]:rounded-[9px] [&>button]:border [&>button]:border-black/12 [&>button]:p-[7px] [&>button>span]:rounded-[5px] [&>button>span]:bg-black/58 [&>button>span]:px-1.5 [&>button>span]:py-0.5 [&>button>span]:text-[11px] [&>button>span]:text-white";
export const formGrid =
  "grid grid-cols-2 gap-x-[18px] gap-y-3.5 max-phone:grid-cols-1 [&>label]:flex [&>label]:min-w-0 [&>label]:flex-col [&>label]:gap-[7px] [&>label]:text-[13px] [&>label]:text-secondary [&_input[type=range]]:w-full [&_select]:w-full";
export const colorInput =
  "size-auto h-8 w-[52px] rounded-[7px] border border-control-border bg-transparent p-0.5";
/** A checkbox reads as a row, not a stacked field, so it overrides the grid's label direction. */
export const checkLabel = "flex-row! items-center";
export const photoButton = "mt-0 inline-flex w-fit cursor-pointer [&>input]:hidden";
export const warning = "col-span-full my-0.5 text-xs text-danger";

export const templateGrid =
  "grid grid-cols-2 gap-2.5 max-phone:grid-cols-1 [&>button]:grid [&>button]:min-w-0 [&>button]:grid-cols-[96px_minmax(0,1fr)] [&>button]:items-center [&>button]:gap-2.5 [&>button]:rounded-[10px] [&>button]:border [&>button]:border-black/12 [&>button]:p-2.5 [&>button]:text-left [&>button]:font-[650] [&>button>span]:overflow-hidden [&>button>span]:text-ellipsis [&>button>span]:whitespace-nowrap";
export const editorPreview = "rounded-[10px] border border-edge bg-subtle p-3";
export const editorActions = "mb-3 flex justify-end gap-2 [&>button]:m-0";
export const editorOpen = "mt-3.5";

// ---- the saved-skin library ----

export const libraryDialog =
  "flex flex-wrap items-end gap-x-3.5 gap-y-2.5 rounded-[10px] border border-accent-soft-border bg-raised p-3.5 [&>label]:flex [&>label]:min-w-[min(260px,100%)] [&>label]:flex-1 [&>label]:flex-col [&>label]:gap-[7px] [&>label]:text-[13px] [&>label]:text-secondary [&_input]:min-h-[34px] [&_input]:rounded-[7px] [&_input]:border [&_input]:border-control-border [&_input]:bg-subtle [&_input]:px-[9px] [&_input]:py-1.5 [&_input]:text-body [&>p]:m-0 [&>p]:flex-[1_1_260px] [&>p]:text-secondary [&>div]:flex [&>div]:shrink-0 [&>div]:grow-0 [&>div]:basis-auto [&>div]:gap-2 [&>div]:max-phone:w-full [&>div>button]:m-0 [&>div>button]:max-phone:flex-1";
export const libraryNotice = "m-0 rounded-lg bg-accent-soft px-3 py-2.5 text-xs text-secondary";
export const libraryEmpty = "m-0 leading-normal text-muted";
export const libraryList = "grid grid-cols-2 gap-2.5 max-phone:grid-cols-1";
export const libraryCard =
  "flex min-w-0 flex-col gap-2 rounded-[10px] border border-edge bg-card p-[9px]";
export const libraryApply =
  "grid w-full min-w-0 grid-cols-[96px_minmax(0,1fr)] items-center gap-2.5 border-0 bg-transparent p-0 text-left text-body [&>div>strong]:overflow-hidden [&>div>strong]:text-ellipsis [&>div>strong]:whitespace-nowrap [&>strong]:overflow-hidden [&>strong]:text-ellipsis [&>strong]:whitespace-nowrap";
export const libraryActions =
  "flex flex-wrap justify-end gap-1.5 [&>.secondary]:m-0 [&>.secondary]:px-[9px] [&>.secondary]:py-[5px] [&>.danger-text]:px-1 [&>.danger-text]:py-[5px]";
