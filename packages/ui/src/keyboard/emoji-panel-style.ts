import * as surface from "./panel-surface-style";

/** 表情面板搜索框与剪贴板记录共用字段配色，明暗由面板根属性控制。 */
export const panelField =
  "border border-[#3a3a44] bg-[#2b2b33] text-[#f5f5f7] group-data-[panel-theme=light]:border-[#c8ccd5] group-data-[panel-theme=light]:bg-white group-data-[panel-theme=light]:text-[#202124]";

/** 记录和它的 × 删除按钮共用的外框；× 浮在记录右端，悬停或聚焦这一行时才出现。 */
export const clipboardRowFrame = "group/row relative flex min-w-0 flex-1";

/** 有删除按钮时给右端留出 × 的位置，文字不会从它下面穿过。 */
export const clipboardRow = (withDelete: boolean) =>
  `w-full flex-1 min-w-0 overflow-hidden rounded-lg py-3 pl-3.5 ${withDelete ? "pr-10" : "pr-3.5"} text-left font-[inherit] text-ellipsis whitespace-nowrap ${panelField} hover:border-[#5fbf84] hover:bg-[#303038] focus-visible:border-[#5fbf84] focus-visible:bg-[#303038] group-data-[panel-theme=light]:hover:border-[#2c7a4b] group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:border-[#2c7a4b] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3]`;

/** × 删除按钮与 macOS 原生剪贴板行一样只在悬停或聚焦这一行时显示。没有悬停的设备不用它，改用行旁的「删除」按钮（见 `EmojiClipboardEntries`）；这里的 hover:hover 条件只是兜底，媒体特性判断与组件不一致时 × 仍然看得见、删得掉。 */
export const clipboardDeleteButton =
  "absolute top-1/2 right-1.5 flex size-7 -translate-y-1/2 items-center justify-center rounded-md border-0 bg-transparent p-0 font-[inherit] text-lg leading-none text-[#aeb0b7] hover:bg-white/10 hover:text-[#f5f5f7] focus-visible:opacity-100 disabled:cursor-wait group-data-[panel-theme=light]:text-[#656a73] group-data-[panel-theme=light]:hover:bg-black/[0.06] group-data-[panel-theme=light]:hover:text-[#202124] [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-focus-within/row:opacity-100 [@media(hover:hover)]:group-hover/row:opacity-100";

export const clipboardSideButton = `shrink-0 grow-0 basis-auto rounded-lg p-2 font-[inherit] ${panelField} hover:border-[#5fbf84] focus-visible:border-[#5fbf84] group-data-[panel-theme=light]:hover:border-[#2c7a4b] group-data-[panel-theme=light]:focus-visible:border-[#2c7a4b]`;

/** 网格条目保持统一尺寸；流式条目按内容伸缩并允许换行，优先于紧凑尺寸。 */
export const panelTile = (flow: boolean, compact = false) =>
  `flex min-w-0 items-center justify-center overflow-hidden rounded-[9px] border border-transparent bg-transparent text-center font-emoji-text leading-[1.2] wrap-anywhere ${surface.panelText} hover:border-[#555560] hover:bg-[#303038] focus-visible:border-[#555560] focus-visible:bg-[#303038] group-data-[panel-theme=light]:hover:border-[#c8ccd5] group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:border-[#c8ccd5] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3] disabled:cursor-wait disabled:opacity-55 ${
    flow
      ? "max-w-full flex-[0_1_auto] min-h-12 px-3 py-2.5 text-xl whitespace-pre-wrap"
      : compact
        ? "h-10 min-h-10 p-1 text-xl"
        : "min-h-[66px] p-1.5 text-[29px]"
  }`;

/** 长文本预览气泡，尺寸和配色取自 macOS 原生剪贴板的悬停预览：13px、最多 12 行按词换行、圆角 10、深色底 96% 不透明。不接收指针，也不进无障碍树（整行文字已经是按钮的名称）。 */
export const clipboardPreviewBubble =
  "pointer-events-none fixed z-20 line-clamp-12 overflow-hidden rounded-[10px] bg-[#1c1c22]/96 px-3.5 py-2.5 text-[13px] leading-[1.35] wrap-break-word whitespace-pre-wrap text-[#f5f5f7] shadow-[0_6px_18px_rgba(0,0,0,0.28)] group-data-[panel-theme=light]:bg-[#2b2b33]/96";

/** 行的右键菜单：只有「发到云剪贴板」一项，样式沿用面板字段配色。 */
export const clipboardContextMenu = `fixed z-30 flex min-w-36 flex-col rounded-lg p-1 shadow-[0_6px_18px_rgba(0,0,0,0.28)] ${panelField}`;
export const clipboardContextMenuItem =
  "rounded-md border-0 bg-transparent px-3 py-1.5 text-left font-[inherit] text-[13px] text-inherit hover:bg-[#303038] focus-visible:bg-[#303038] focus-visible:outline-none disabled:cursor-wait disabled:opacity-55 group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3]";
