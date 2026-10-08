/**
 * Class strings for the platform controls in `platform-controls.tsx`. Every length and colour is a `--p-*` token from the `[data-platform]` layer in `styles.css`, so one markup takes each platform's shape. Values that Tailwind could read either as a colour or as a size (`text-`, `shadow-`, `border-`) are written as arbitrary properties so the property is never inferred.
 */

// ---- groups and rows ----

export const group = "flex min-w-0 flex-col gap-[var(--p-g-title-gap)]";
export const groupTitle =
  "m-0 [padding:var(--p-g-title-pad)] [font-size:var(--p-g-title-fs)] [font-weight:var(--p-g-title-w)] [color:var(--p-g-title-fg)]";
/**
 * 细线画在第一个可见行之后的每个可见行上，所以只有一行的分组没有细线，最后一行下方不会多出一条，隐藏的行也不会留下线。
 *
 * 除 HarmonyOS 手机外，所有平台都照旧把它画成 1px 的上边框。HarmonyOS 手机则改用沿行顶边的背景图来画：粗 `--p-row-divider-w`（.5px），从起始边向内 `--p-row-divider-inset`（16px）处开始，一直延伸到末端边，这是边框做不到的。它用展开写法（longhand）书写，让行自身的背景色（按下时的填充）留在下面。每个类名都完整写出，因为样式表只生成它在源码中找到的类名。
 */
export const groupRows =
  "flex flex-col gap-[var(--p-row-gap)] rounded-[var(--p-group-r)] bg-[var(--p-group-bg)] [box-shadow:var(--p-group-shadow)] [&>:not([hidden])~:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)] harmony:[&>:not([hidden])~:not([hidden])]:[border-top:0] harmony:[&>:not([hidden])~:not([hidden])]:[background-image:linear-gradient(var(--p-row-divider),var(--p-row-divider))] harmony:[&>:not([hidden])~:not([hidden])]:[background-position:right_top] harmony:[&>:not([hidden])~:not([hidden])]:[background-size:calc(100%_-_var(--p-row-divider-inset))_var(--p-row-divider-w)] harmony:[&>:not([hidden])~:not([hidden])]:[background-repeat:no-repeat]";

export const row =
  "flex min-h-[var(--p-row-h)] min-w-0 items-center gap-[var(--p-row-ctl-gap)] rounded-[var(--p-row-r)] bg-[var(--p-row-bg)] [padding:var(--p-row-pad)] [box-shadow:var(--p-row-shadow)]";
/** Windows leads each card with a glyph; the other platforms set `--p-row-icon: none`. */
export const rowIcon = "size-5 shrink-0 items-center justify-center [display:var(--p-row-icon)]";
export const rowText = "flex min-w-0 flex-1 flex-col gap-0.5";
export const rowTitle = "block [font-size:var(--p-row-fs)] [color:var(--p-text)]";
export const rowDescription = "block [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const rowControl = "flex shrink-0 items-center gap-2";
/** 跳转行：整行是一个按钮，外形与 `row` 相同，悬停时变色。 */
export const linkRow = `${row} w-full cursor-pointer border-0 text-left hover:bg-[var(--p-hover)]`;

// ---- 更多选项 ----

/** 「更多选项」折叠区本身是组里的一行，靠 `group/more` 让标题的箭头随展开旋转。 */
export const moreOptions = "group/more min-w-0";
export const moreOptionsSummary = `${row} cursor-pointer list-none hover:bg-[var(--p-hover)] [&::-webkit-details-marker]:hidden`;
export const moreOptionsMarker =
  "[font-size:var(--p-sub-fs)] [color:var(--p-sub)] transition-transform group-open/more:rotate-90 motion-reduce:transition-none";
/** 展开后的每一行都画上分隔线，与组内各行之间的分隔一致。 */
export const moreOptionsRows =
  "flex flex-col gap-[var(--p-row-gap)] [&>:not([hidden])]:[border-top:1px_solid_var(--p-row-divider)] harmony:[&>:not([hidden])]:[border-top:0] harmony:[&>:not([hidden])]:[background-image:linear-gradient(var(--p-row-divider),var(--p-row-divider))] harmony:[&>:not([hidden])]:[background-position:right_top] harmony:[&>:not([hidden])]:[background-size:calc(100%_-_var(--p-row-divider-inset))_var(--p-row-divider-w)] harmony:[&>:not([hidden])]:[background-repeat:no-repeat]";

// ---- 页面简介 ----

/** 页首说明：页面第一组之前的一段灰字，内边距取组名的 `--p-g-title-pad`，与下面各组的组名对齐。 */
export const pageIntro =
  "m-0 leading-relaxed [padding:var(--p-g-title-pad)] [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";

// ---- switch ----

export const switchWrap = "inline-flex items-center gap-3";
/** Windows prints 开/关 before the switch; `--p-sw-label` is `none` everywhere else. */
export const switchState = "[display:var(--p-sw-label)] [font-size:14px] [color:var(--p-text)]";

// ---- segmented ----

export const segmented =
  "inline-flex max-w-full items-stretch gap-0.5 rounded-[var(--p-seg-r)] bg-[var(--p-seg-bg)] p-0.5 [border:var(--p-seg-border)]";
/** Windows marks the chosen item with a short accent bar; `--p-seg-indicator` is 0 elsewhere, which draws nothing. */
export const segment =
  "relative flex min-h-7 cursor-pointer items-center justify-center rounded-[var(--p-seg-item-r)] px-3 [font-size:13px] [color:var(--p-text)] after:absolute after:bottom-0.5 after:left-1/2 after:hidden after:h-[3px] after:w-[var(--p-seg-indicator)] after:-translate-x-1/2 after:rounded-full after:bg-accent has-[:checked]:bg-[var(--p-seg-on)] has-[:checked]:[color:var(--p-seg-on-fg)] has-[:checked]:[box-shadow:var(--p-seg-on-shadow)] has-[:checked]:after:block has-[:disabled]:cursor-default has-[:disabled]:opacity-50 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-1 has-[:focus-visible]:outline-accent";
/** The radio stays in the tree for its role, arrow keys and focus; only its drawing goes. */
export const segmentInput = "sr-only";

// ---- select ----

/** 保留原生元素和它自带的箭头（见 UPSTREAM.md），只有外框跟随平台。无边框的平台用 `--p-sel-sizing: content` 让宽度贴合当前选中项：原生 select 默认按最长的选项定宽，同一组里选项长短不同的两行会一行文字贴着箭头、一行文字远离箭头。Windows 的下拉框有边框，保持默认的固定宽度。 */
export const select =
  "min-w-0 rounded-[var(--p-r-ctl)] [background:var(--p-sel-bg)] [border:var(--p-sel-border)] [padding:var(--p-sel-pad)] [font-size:var(--p-sel-fs)] [color:var(--p-sel-fg)] [field-sizing:var(--p-sel-sizing,fixed)]";

// ---- checks ----

export const checks = "m-0 flex min-w-0 flex-col gap-2 border-0 p-0";
export const checksLegend = "mb-1 p-0 [font-size:var(--p-row-fs)] [color:var(--p-text)]";
export const checksDescription =
  "-mt-1 mb-1 block [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const check =
  "flex cursor-pointer items-center gap-2.5 [font-size:var(--p-row-fs)] [color:var(--p-text)] has-[:disabled]:cursor-default has-[:disabled]:opacity-50";
/** HarmonyOS 手机的两列复选列表（翻页方式）：简短的标签在说明文字下并排，每个复选框 18px、圆角 4px。 */
export const checksGrid =
  "grid grid-cols-2 gap-x-5 gap-y-0.5 px-4 pb-3 [--p-check-size:18px] [--p-check-r:4px]";
export const checkGridItem =
  "flex min-h-[34px] min-w-0 cursor-pointer items-center gap-2.5 text-[13px] [color:var(--p-text)] has-[:disabled]:cursor-default has-[:disabled]:opacity-50";

// ---- 选择行（HarmonyOS 手机） ----

/** HarmonyOS 手机的选择行和分段行：整行是一个按钮，点开选择弹窗，末端显示当前值和一个小箭头。 */
export const pickerRow = `${row} w-full cursor-pointer border-0 text-left [font-family:inherit] active:bg-[var(--p-press)] disabled:cursor-default disabled:opacity-50`;
export const pickerValue =
  "flex max-w-[55%] min-w-0 shrink-0 items-center gap-1.5 text-[16px] [color:var(--p-sub)]";
export const pickerValueText = "min-w-0 truncate";

// ---- 导航列表（HarmonyOS 手机的设置首页和「我的」） ----

export const navGroup = "overflow-hidden rounded-[20px] bg-[var(--p-group-bg)]";
/** 细线沿第一个可见行之后每个可见行的文字列顶边画出，所以它从标签开始的位置起、到末端内边距处止，最后一行下方没有细线。 */
export const navGroupRows =
  "[&>:not([hidden])~:not([hidden])_[data-nav-rule]]:[background:linear-gradient(var(--p-hair),var(--p-hair))_left_top/100%_1px_no-repeat]";
export const navRow =
  "m-0 flex w-full min-w-0 items-stretch gap-3 border-0 bg-transparent px-3.5 py-0 text-left [font-family:inherit] [color:var(--p-text)] active:bg-[var(--p-press)] disabled:opacity-[.38] aria-disabled:opacity-[.38]";
export const navRowButton = "cursor-pointer disabled:cursor-default";
export const navRowIcon = {
  root: "flex size-[29px] shrink-0 items-center justify-center self-center [color:var(--p-text)]",
  me: "flex size-7 shrink-0 items-center justify-center self-center [color:var(--p-accent-text)]",
} as const;
export const navRowBody = {
  root: "flex min-h-[52px] min-w-0 flex-1 items-center gap-3",
  me: "flex min-h-[48px] min-w-0 flex-1 items-center gap-3 py-2",
} as const;
export const navRowText = "flex min-w-0 flex-1 flex-col gap-0.5";
export const navRowTitle = {
  root: "text-[17px] font-normal",
  me: "text-[16px] font-normal",
} as const;
export const navRowDescription = "text-[12px] [color:var(--p-sub)]";
export const navRowValue = {
  root: "max-w-[55%] min-w-0 shrink-0 truncate text-[16px] [color:var(--p-sub)]",
  me: "max-w-[55%] min-w-0 shrink-0 truncate text-[14px] [color:var(--p-sub)]",
} as const;
export const navRowChevron = "-ml-1 shrink-0 self-center opacity-55 [color:var(--p-sub)]";
export const navRowTrailing = "flex shrink-0 items-center gap-2";

// ---- navigation ----

/** Windows adds the 3×16 accent pill on the selected item; `--p-nav-pill` is 0 elsewhere. */
export const navItem =
  "relative flex h-[var(--p-nav-h)] w-full min-w-0 items-center gap-3 rounded-[var(--p-nav-r)] border-0 bg-transparent text-left [padding:var(--p-nav-pad)] [font-size:14px] [color:var(--p-text)] before:absolute before:top-1/2 before:left-0 before:hidden before:h-4 before:w-[var(--p-nav-pill)] before:-translate-y-1/2 before:rounded-full before:bg-accent hover:bg-[var(--p-hover)] aria-[current=page]:bg-[var(--p-nav-selected-bg)] aria-[current=page]:before:block";
export const navIcon = "flex size-5 shrink-0 items-center justify-center";
export const navLabel = "min-w-0 flex-1 truncate";
