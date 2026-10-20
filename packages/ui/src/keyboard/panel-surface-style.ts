/** 手写和语音浮窗的布局与配色。手写板使用自身明暗属性下的局部颜色变量，语音沿用分组主题。 */

import { minWidthZero } from "../core/row-style";

export const panelText = "text-[#f5f5f7] group-data-[panel-theme=light]:text-[#202124]";
export const panelMuted = "text-[#aeb0b7] group-data-[panel-theme=light]:text-[#656a73]";
export const panelSurface = "bg-[#202027] group-data-[panel-theme=light]:bg-[#f7f8fa]";
const panelEdge = "border-[#45454f] group-data-[panel-theme=light]:border-[#c8ccd5]";
const panelField = `rounded-[7px] border bg-[#2b2b33] ${panelEdge} ${panelText} group-data-[panel-theme=light]:bg-white`;

export const panelHeader = `group-data-[panel-theme=light]:border-b-[#d9dce3] border-b-[#45454f] ${panelSurface} ${panelText} [&>button]:text-[#aeb0b7] group-data-[panel-theme=light]:[&>button]:text-[#656a73]`;

// ---- voice ----

export const voicePanel = `group min-h-screen ${panelSurface} ${panelText}`;
export const voiceBody =
  "mx-auto flex min-h-[calc(100vh-38px)] max-w-[560px] flex-col items-stretch gap-3.5 px-8 pt-[38px] pb-7 [&>h1]:m-0 [&>h1]:text-center [&>h1]:text-[22px]";
export const voiceIcon = "self-center text-5xl leading-none";
export const voiceNote = `m-0 text-center text-[13px] ${panelMuted}`;
export const voiceLanguage =
  "flex items-center justify-between gap-4 text-[13px] text-[#d7d7dc] group-data-[panel-theme=light]:text-[#3d424b]";
export const voiceLanguageInput = `min-w-[190px] p-2 ${panelField}`;
export const voiceTextArea = `min-h-25 resize-y p-2.5 font-[inherit] ${panelField}`;
const voiceAction = "min-h-[42px] rounded-lg border-0 font-semibold";
export const voiceRecord = `${voiceAction} bg-[#5fbf84] text-[#241c26]`;
export const voiceSubmit = `${voiceAction} bg-[#3a3945] text-[#f5f5f7] group-data-[panel-theme=light]:bg-[#e4e7ed] group-data-[panel-theme=light]:text-[#202124]`;

// ---- handwriting ----

// 面板明暗由自身属性决定，避免全局设置页主题覆盖浮窗的画布和系统按钮。字体先取 macOS 的系统字体，其他平台（Linux 的 WebKitGTK）没有它们时依次退到 Cantarell 和常见的思源/Noto 中文字体，而不是由 fontconfig 随发行版挑一个 sans-serif。
export const handwritingPanel =
  "group flex h-screen flex-col overflow-auto bg-[var(--hw-bg)] text-[var(--hw-text)] [font-family:-apple-system,BlinkMacSystemFont,'PingFang_SC',Cantarell,'Noto_Sans_SC','Noto_Sans_CJK_SC','Source_Han_Sans_SC',sans-serif] [color-scheme:dark] [--hw-bg:#242628] [--hw-card:#2d3032] [--hw-canvas:#282b2d] [--hw-text:#f0f1f0] [--hw-muted:#a2aaa7] [--hw-edge:#414644] [--hw-guide:#424a46] [--hw-accent:#84c5a0] [--hw-soft:#35483e] data-[panel-theme=light]:[color-scheme:light] data-[panel-theme=light]:[--hw-bg:#f4f5f4] data-[panel-theme=light]:[--hw-card:#fff] data-[panel-theme=light]:[--hw-canvas:#fafbf9] data-[panel-theme=light]:[--hw-text:#25312b] data-[panel-theme=light]:[--hw-muted:#6b7870] data-[panel-theme=light]:[--hw-edge:#dce2dd] data-[panel-theme=light]:[--hw-guide:#dce5dd] data-[panel-theme=light]:[--hw-accent:#28764d] data-[panel-theme=light]:[--hw-soft:#e6f1e9] [&_button]:font-[inherit] [&_button]:transition-colors [&_button]:disabled:cursor-default [&_button]:disabled:opacity-40 [&_button]:focus-visible:outline-2 [&_button]:focus-visible:outline-offset-2 [&_button]:focus-visible:outline-[var(--hw-accent)]";
export const handwritingHeader =
  "flex h-14 shrink-0 items-center justify-between border-0 border-b border-solid border-[var(--hw-edge)] px-6 text-[13px] font-medium select-none [&>span]:flex [&>span]:items-center [&>span]:gap-2.5 [&_svg]:text-[var(--hw-accent)] [&>button]:flex [&>button]:size-7 [&>button]:items-center [&>button]:justify-center [&>button]:rounded-full [&>button]:border-0 [&>button]:bg-transparent [&>button]:p-0 [&>button]:text-xl [&>button]:text-[var(--hw-muted)] [&>button:hover]:bg-[var(--hw-edge)] [&>button:hover]:text-[var(--hw-text)]";
/** 画布与候选卡片占满可用高度，小窗口改为纵向排列。 */
export const handwritingBody =
  "mx-auto grid w-full max-w-[1060px] flex-1 box-border grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)] items-start gap-6 p-6 max-phone:grid-cols-1 max-phone:gap-4 max-phone:p-4";
export const inkSection = `${minWidthZero} flex flex-col items-center gap-3.5`;
export const inkCanvas =
  "block aspect-square w-full max-w-[calc(100vh-155px)] box-border cursor-crosshair touch-none rounded-2xl border border-solid border-[var(--hw-edge)] bg-[var(--hw-canvas)] shadow-[0_2px_8px_#00000008] focus-visible:outline-2 focus-visible:-outline-offset-[3px] focus-visible:outline-[var(--hw-accent)] [&_polyline]:fill-none [&_polyline]:stroke-[var(--hw-text)] [&_polyline]:stroke-4 [&_polyline]:[stroke-linecap:round] [&_polyline]:[stroke-linejoin:round] [&_circle]:fill-[var(--hw-text)] [&_text]:fill-[var(--hw-muted)] [&_text]:text-[17px]";
export const inkGuides =
  "pointer-events-none fill-none stroke-[var(--hw-guide)] stroke-1 [stroke-dasharray:5_7]";
const handwritingButton =
  "rounded-lg border border-solid border-[var(--hw-edge)] bg-[var(--hw-card)] text-[var(--hw-text)] enabled:hover:border-[var(--hw-accent)] enabled:hover:bg-[var(--hw-soft)]";
export const inkActions = `flex w-full max-w-[calc(100vh-155px)] gap-2 [&>button]:min-h-9 [&>button]:min-w-0 [&>button]:flex-1 [&>button]:px-1 [&>button]:text-xs [&>button]:whitespace-nowrap`;
export const inkAction = handwritingButton;
export const handwritingActions = `flex flex-wrap items-center gap-2 text-xs text-[var(--hw-muted)] [&>button]:min-h-7 [&>button]:rounded-md [&>button]:border-0 [&>button]:bg-transparent [&>button]:px-3 [&>button]:text-[var(--hw-muted)] [&>button]:enabled:hover:bg-[var(--hw-soft)] [&>button[aria-pressed=true]]:bg-[var(--hw-soft)] [&>button[aria-pressed=true]]:font-medium [&>button[aria-pressed=true]]:text-[var(--hw-accent)]`;
export const recognitionSection =
  "flex min-h-[min(490px,calc(100vh-105px))] min-w-0 self-stretch flex-col rounded-2xl border border-solid border-[var(--hw-edge)] bg-[var(--hw-card)] p-5 box-border max-phone:min-h-64 [&>h2]:m-0 [&>h2]:mb-3 [&>h2]:text-[15px] [&>h2]:font-semibold [&>p]:mb-0 [&>p]:mt-4 [&>p]:border-0 [&>p]:border-t [&>p]:border-solid [&>p]:border-[var(--hw-edge)] [&>p]:pt-3 [&>p]:text-xs [&>p]:leading-relaxed [&>p]:text-[var(--hw-muted)]";
export const candidateGrid = "mt-5 grid grid-cols-4 content-start gap-2 empty:mt-0";
export const candidate = "flex min-w-0 flex-col gap-1";
export const candidateSubmit = `w-full min-w-0 min-h-16 p-1.5 leading-tight wrap-anywhere whitespace-normal ${handwritingButton}`;
export const candidateCopy =
  "rounded-md border-0 bg-transparent px-2 py-1 text-xs text-[var(--hw-muted)] enabled:hover:bg-[var(--hw-soft)] enabled:hover:text-[var(--hw-accent)]";
export const handwritingEmpty =
  "flex flex-1 flex-col items-center justify-center gap-3 py-9 text-center text-[var(--hw-muted)] [&>span]:flex [&>span]:size-14 [&>span]:items-center [&>span]:justify-center [&>span]:rounded-2xl [&>span]:bg-[var(--hw-soft)] [&>span]:text-[var(--hw-accent)] [&>p]:m-0 [&>p]:text-[13px] [&>small]:text-xs";

// ---- the handwriting illustration on the settings page, which uses the app's palette ----

export const mock = "mx-auto w-[min(560px,100%)]";
export const mockCanvas =
  "flex min-h-[230px] items-center justify-center rounded-xl border border-dashed border-edge-strong bg-card text-muted";
export const mockStroke = "rotate-[-7deg] font-kai text-[110px] leading-none text-secondary";
export const mockCandidates =
  "mt-2.5 flex gap-2 [&>span]:flex-1 [&>span]:rounded-md [&>span]:border [&>span]:border-edge [&>span]:p-2";

// ---- the on-screen keyboard panel ----

/*
 * This panel's palette is sixteen `--kb-*` custom properties, and `keyboardSkinStyles` sets every one
 * of them on the root from the active skin. The stylesheet also carried a default for each, plus a
 * light-theme override, and neither could ever apply: the component does not render without calling
 * that function. The tokens are the theming mechanism; the utilities below only read them.
 */
export const keyboardPanel =
  "flex h-screen min-h-0 flex-col rounded-lg bg-[var(--kb-background)] font-[var(--kb-font-family)] text-[var(--kb-text)]";
export const keyboardHeader =
  "flex min-h-9 shrink-0 items-center gap-2 bg-[var(--kb-background)] px-2.5 py-1.5 text-xs text-[var(--kb-heading)] select-none [&>button]:h-7 [&>button]:shrink-0 [&>button]:rounded [&>button]:border-0 [&>button]:bg-transparent [&>button]:px-2 [&>button]:text-xs [&>button]:leading-none [&>button]:whitespace-nowrap [&>button]:text-[var(--kb-text)] [&>button:hover]:bg-[var(--kb-hover)] [&>button:last-child]:w-7 [&>button:last-child]:p-0 [&>button:last-child]:text-xl";
export const keyboardNotice =
  "min-w-0 flex-1 text-xs leading-5 break-words whitespace-normal text-[var(--kb-heading)]";
export const keyboardBody = "min-h-0 flex-1 px-[7px] pt-0 pb-[7px]";
export const keyboardLayout = "flex h-full flex-col gap-[var(--keyboard-row-gap,4px)]";
export const keyboardRow = "flex min-h-0 flex-1 gap-[var(--keyboard-key-gap,4px)]";
export const keyboardKeyHint =
  "block max-w-full overflow-hidden text-[9px]/[1.1] text-[var(--kb-heading)] opacity-80 whitespace-nowrap";
/*
 * A key. `--kb-current-fill` is the indirection that lets an action key swap its own fill without
 * restating every other declaration, and the three materials layer on the same base.
 */
export const keyboardKey = (options: {
  material: "flat" | "raised" | "glass" | "paper";
  active: boolean;
  wide: boolean;
}) =>
  [
    "min-h-0 min-w-0 flex-1 rounded-[var(--kb-key-radius)] border-[length:var(--kb-border-width)] border-solid border-[var(--kb-border-color)] p-0 font-[inherit] shadow-[var(--kb-shadow)]",
    "[--kb-current-fill:var(--kb-key-fill)] bg-[var(--kb-current-fill)] text-[var(--kb-text)]",
    // The latched fill below is `!important` so it beats hover; the pressed fill must be too, or pressing a latched modifier shows no feedback. The stylesheet this replaced had `:active` after `.active` at equal specificity, so pressed always won.
    "hover:bg-[var(--kb-hover)] active:bg-[var(--kb-pressed)]!",
    options.wide ? "text-xs" : "text-[15px]",
    options.active ? "bg-[var(--kb-active)]!" : "",
    options.material === "raised"
      ? "-translate-y-px shadow-[var(--kb-shadow),0_2px_0_rgba(0,0,0,0.18)] active:translate-y-0"
      : "",
    options.material === "glass" ? "backdrop-blur-[8px]" : "",
    options.material === "paper"
      ? "bg-[repeating-linear-gradient(0deg,transparent_0_7px,var(--kb-paper-line)_7px_8px)] [background-size:auto]"
      : "",
  ]
    .filter(Boolean)
    .join(" ");
