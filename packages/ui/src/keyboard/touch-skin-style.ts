/**
 * The touch-keyboard scheme list, skin picker and skin editor.
 *
 * All three share one shape -- a preview beside a caption, selectable -- so the pieces are named once
 * and the three call sites differ only in what they put in the preview column.
 */

import { flexSpacer, looseStack } from "../core/page-style";
import { inlineItems } from "../core/row-style";

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

export const editor = looseStack;
/** 内嵌时的标题行：标题在左，「完成」在右。 */
export const editorHeading = "flex items-start justify-between gap-4";
/** 编辑器自己的主按钮和删除按钮是实心的，不是描边的。 */
export const editorFilled =
  "[&_.primary]:rounded-full [&_.primary]:border [&_.primary]:border-accent [&_.primary]:bg-accent [&_.primary]:px-4 [&_.primary]:py-[7px] [&_.primary]:text-white [&_.danger]:rounded-full [&_.danger]:border [&_.danger]:border-danger [&_.danger]:bg-danger [&_.danger]:px-4 [&_.danger]:py-[7px] [&_.danger]:text-white";

/** 预览卡：上面是键盘预览，下面是撤销、重做、保存和使用。全屏时它吸在顶部，往下翻控件时预览一直看得见；吸顶时用页面底色垫住卡片上方，翻上去的控件不会从卡片边缘露出来。 */
export const previewDock = (sticky: boolean) =>
  sticky
    ? "sticky top-[env(safe-area-inset-top,0px)] z-10 -mx-4 bg-[var(--p-bg)] px-4 pt-1 pb-2"
    : "";
export const previewCard =
  "flex flex-col gap-2.5 overflow-hidden rounded-[var(--p-group-r)] bg-[var(--p-group-bg)] p-2.5 [box-shadow:var(--p-group-shadow)]";
/** 预览限宽：手机上整宽的键盘有两百多像素高，吸在顶部会挤掉一半屏幕，缩到 300 后下面还能放下五六行控件；桌面的键盘更宽更扁，放宽到 560。 */
export const previewFrame = (phone: boolean) =>
  `mx-auto w-full overflow-hidden rounded-[10px] ${phone ? "max-w-[300px]" : "max-w-[560px]"}`;
export const previewBar = inlineItems;
/** 撤销、重做是图标按钮，名称在 `aria-label` 上。 */
export const iconButton =
  "m-0 inline-flex size-9 shrink-0 items-center justify-center rounded-full border-0 bg-transparent p-0 [color:var(--p-text)] not-disabled:hover:bg-[var(--p-seg-bg)] disabled:opacity-35 focus-visible:outline-2 focus-visible:outline-accent";
export const previewBarSpacer = flexSpacer;
/** 次要的胶囊按钮：强调色文字，淡强调色底。 */
export const tonalButton =
  "m-0 inline-flex min-h-9 items-center justify-center gap-1.5 rounded-full border-0 bg-accent-soft px-4 font-semibold text-accent disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-accent";

/** AI 抽卡入口：整行一张淡强调色的卡片，左边标题和一句说明，右边箭头。 */
export const aiEntry =
  "m-0 flex w-full items-center gap-3 rounded-[var(--p-group-r)] border-0 bg-accent-soft px-4 py-3 text-left disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-accent [&>svg]:shrink-0 [&>svg]:text-accent";
export const aiEntryText =
  "flex min-w-0 flex-1 flex-col gap-0.5 [&>strong]:text-[15px] [&>strong]:font-semibold [&>strong]:text-accent [&>small]:[font-size:var(--p-sub-fs)] [&>small]:[color:var(--p-sub)]";

/** 分类标签画成分段控件，颜色取自各平台分段控件的同一组令牌。 */
export const editorTabs =
  "flex gap-0.5 rounded-[var(--p-seg-r)] bg-[var(--p-seg-bg)] p-0.5 [border:var(--p-seg-border)]";
export const editorTab = (selected: boolean) =>
  `m-0 min-h-9 flex-1 rounded-[var(--p-seg-item-r)] border-0 px-1 text-[14px] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent ${
    selected
      ? "bg-[var(--p-seg-on)] font-semibold [color:var(--p-seg-on-fg)] [box-shadow:var(--p-seg-on-shadow)]"
      : "bg-transparent [color:var(--p-sub)]"
  }`;
/** 各分类的分组之间的间距与设置页分组之间相同。 */
export const editorControls = "flex flex-col gap-[var(--p-groups-gap,20px)]";

/** 分组里不是一行设置的内容（预设网格、模板网格、图库）的内边距，与行的左右边距对齐。 */
export const groupBody = "p-3.5";
/** 按宽度排列：手机上三列，桌面上一行排得下更多。 */
export const backgroundGrid =
  "grid grid-cols-[repeat(auto-fill,minmax(84px,1fr))] gap-x-2.5 gap-y-3";
export const backgroundPreset =
  "m-0 flex min-w-0 flex-col items-center gap-1.5 border-0 bg-transparent p-0 text-[13px] [color:var(--p-text)] focus-visible:outline-none [&:focus-visible>span:first-child]:outline-2 [&:focus-visible>span:first-child]:outline-offset-2 [&:focus-visible>span:first-child]:outline-accent";
/** 色块本身：选中时外圈一道强调色。 */
export const backgroundSwatch = (selected: boolean) =>
  `relative flex aspect-[16/10] w-full items-center justify-center rounded-[10px] shadow-[inset_0_0_0_1px_rgba(0,0,0,.08)] ${
    selected ? "outline-2 outline-offset-2 outline-accent" : ""
  }`;
export const backgroundCheck =
  "flex size-6 items-center justify-center rounded-full bg-black/45 text-white [&>svg]:size-3.5";

/** 颜色行末端：当前色值和一个圆形色块，色块就是原生取色器，只换了画法。 */
export const colorControl = "flex items-center gap-2.5";
export const colorValue = "font-mono text-[13px] uppercase [color:var(--p-sub)]";
export const colorSwatch =
  "size-8 shrink-0 cursor-pointer appearance-none rounded-full border-0 bg-transparent p-0 [box-shadow:inset_0_0_0_1px_var(--p-hair),0_0_0_1px_var(--p-hair)] disabled:cursor-default disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent [&::-webkit-color-swatch-wrapper]:p-0 [&::-webkit-color-swatch]:rounded-full [&::-webkit-color-swatch]:border-0 [&::-moz-color-swatch]:rounded-full [&::-moz-color-swatch]:border-0";
/** 行末端的文字按钮（选择照片、移除照片之类）。 */
export const rowAction =
  "m-0 rounded-full border-0 bg-transparent px-2 py-1 font-medium text-accent disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-accent [&>input]:hidden";
export const rowActionDanger = `${rowAction} text-danger!`;
export const warning = "m-0 px-4 py-2.5 text-[13px] text-danger";

const skinTileGrid = "grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-2.5";
export const templateGrid = skinTileGrid;
export const templateCard =
  "m-0 flex min-w-0 flex-col gap-2 overflow-hidden rounded-[12px] border-0 p-2 text-left font-semibold [box-shadow:inset_0_0_0_1px_rgba(0,0,0,.08)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent [&>span]:overflow-hidden [&>span]:px-1 [&>span]:text-[13px] [&>span]:text-ellipsis [&>span]:whitespace-nowrap";
/** 分组下方居中的文字按钮（重置、优化对比度）。 */
export const footerAction = "flex justify-center";
export const editorOpen = "mt-3.5";

// ---- the saved-skin library ----

export const libraryDialog =
  "flex flex-wrap items-end gap-x-3.5 gap-y-2.5 rounded-[var(--p-group-r)] border border-accent-soft-border bg-[var(--p-group-bg)] p-3.5 [&>label]:flex [&>label]:min-w-[min(260px,100%)] [&>label]:flex-1 [&>label]:flex-col [&>label]:gap-[7px] [&>label]:text-[13px] [&>label]:text-secondary [&_input]:min-h-[34px] [&_input]:rounded-[7px] [&_input]:border [&_input]:border-control-border [&_input]:bg-subtle [&_input]:px-[9px] [&_input]:py-1.5 [&_input]:text-body [&>p]:m-0 [&>p]:flex-[1_1_260px] [&>p]:text-secondary [&>div]:flex [&>div]:shrink-0 [&>div]:grow-0 [&>div]:basis-auto [&>div]:gap-2 [&>div]:max-phone:w-full [&>div>button]:m-0 [&>div>button]:max-phone:flex-1";
export const libraryNotice =
  "m-0 rounded-[var(--p-group-r)] bg-accent-soft px-4 py-2.5 text-[13px] [color:var(--p-text)]";
export const libraryEmpty = "m-0 leading-normal [color:var(--p-sub)]";
export const libraryList = skinTileGrid;
export const libraryCard =
  "flex min-w-0 flex-col gap-2 rounded-[12px] p-2 [box-shadow:inset_0_0_0_1px_var(--p-hair)]";
export const libraryApply =
  "flex w-full min-w-0 flex-col gap-1.5 border-0 bg-transparent p-0 text-left [color:var(--p-text)] [&>div>strong]:overflow-hidden [&>div>strong]:text-ellipsis [&>div>strong]:whitespace-nowrap [&>strong]:overflow-hidden [&>strong]:text-ellipsis [&>strong]:whitespace-nowrap";
export const libraryActions =
  "flex flex-wrap justify-end gap-1.5 [&>.secondary]:m-0 [&>.secondary]:px-[9px] [&>.secondary]:py-[5px] [&>.danger-text]:px-1 [&>.danger-text]:py-[5px]";
