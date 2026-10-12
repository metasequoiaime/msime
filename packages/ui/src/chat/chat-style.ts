import { touchHairline } from "../core/touch-style";
import { mutedText } from "../core/text-style";
import { controlRow } from "../core/row-style";

/**
 * The account chat playground.
 *
 * Every colour here used to reference a token this project does not define -- `--muted`, `--border`,
 * `--surface`, `--text`, `--accent`, `--panel`, `--danger` -- names carried in from somewhere else
 * when the page was added. An undefined custom property makes the declaration invalid at computed
 * value time, so each of them silently did nothing: the message area had no background and a border
 * the colour of its text, and a sent message was `color: #fff` over a background that resolved to
 * transparent, which is white text on the page. They are wired to the real tokens below.
 */

export const page = "flex min-h-[560px] flex-col gap-3.5";
export const toolbar =
  "flex items-center justify-between gap-4 [&_strong]:block [&_small]:block [&_small]:text-muted";

export const messages =
  "min-h-70 flex-1 overflow-auto rounded-[14px] border border-edge bg-card p-[18px]";
export const empty =
  "grid max-w-[480px] gap-2 px-2.5 py-[26px] text-muted [&>strong]:text-body [&>span]:text-muted";

export const message = (mine: boolean) => `mt-0 mb-3 flex ${mine ? "justify-end" : ""}`;
/** The bubble. A sent one is filled with the strong accent, which white text can actually sit on. */
export const bubble = (mine: boolean) =>
  `max-w-[82%] rounded-[14px] px-3.5 py-[11px] whitespace-pre-wrap wrap-anywhere ${
    mine ? "bg-accent-strong text-white" : "bg-raised text-body"
  }`;
export const pending = mutedText;
export const error =
  "flex items-center justify-between gap-3 rounded-[10px] border border-danger/35 px-3 py-2.5 text-danger";

export const composer =
  "grid gap-2 [&>textarea]:min-h-[86px] [&>textarea]:resize-y [&>small]:text-muted";
export const actions = "flex justify-end gap-2";
export const login = "self-start";

// ---- 触屏（鸿蒙手机等）：和「试用键盘」同一套样子 ----

// 细线用内阴影而不是 border：HarmonyOS 的 WebView 有缩放，1px 边框算出来是 0.92px，ArkWeb 重新光栅化圆角边框时会把下半截涂成边框色。

/** 整页：顶部一行小胶囊、中间对话、底部输入栏，对话区自己滚动。 */
export const touchPage = "flex min-h-0 flex-1 flex-col gap-2.5";
export const touchBar = controlRow;
export const touchChip = `m-0 inline-flex shrink-0 cursor-pointer items-center gap-1 whitespace-nowrap rounded-full ${touchHairline} bg-[var(--p-group-bg)] px-3 py-1.5 text-[13px] [font-family:inherit] [color:var(--p-text)] active:opacity-70 disabled:cursor-default disabled:opacity-40`;
export const touchLog = "-mx-1 flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto px-1 pt-1 pb-2";
const touchBubble =
  "m-0 max-w-[82%] rounded-[18px] px-3.5 py-2.5 text-[15px] leading-[1.5] wrap-anywhere";
export const touchMine = `${touchBubble} self-end whitespace-pre-wrap [background:var(--accent-color)] [color:var(--p-on-accent)]`;
export const touchReply = `${touchBubble} self-start bg-[var(--p-group-bg)] [color:var(--p-text)] [&_ol]:my-1 [&_ol]:pl-5 [&_p]:my-0 [&_p+p]:mt-2 [&_pre]:overflow-x-auto [&_pre]:whitespace-pre [&_ul]:my-1 [&_ul]:pl-5`;
export const touchPending = `${touchBubble} self-start bg-[var(--p-group-bg)] [color:var(--p-sub)]`;
/** 出错不再是红框：对话里一行次要文字，后面跟「重试」。 */
export const touchNotice =
  "flex items-center gap-2 self-start px-1 text-[13px] [color:var(--p-sub)]";
/** 空对话和未登录时，对话区正中的提示。 */
export const touchEmpty =
  "m-auto flex max-w-[280px] flex-col items-center gap-2 px-4 py-8 text-center [&>strong]:text-[17px] [&>strong]:font-semibold [&>strong]:[color:var(--p-text)] [&>span]:text-[14px] [&>span]:leading-[1.5] [&>span]:[color:var(--p-sub)]";
export const touchEmptyIcon =
  "mb-1 flex size-14 items-center justify-center rounded-full [background:color-mix(in_srgb,var(--accent-color)_14%,transparent)] [color:var(--accent-color)]";
export const touchLogin =
  "m-0 mt-3 cursor-pointer rounded-full border-0 px-6 py-2.5 text-[15px] font-semibold [font-family:inherit] [background:var(--accent-color)] [color:var(--p-on-accent)] active:opacity-80";
export const touchComposer = "flex shrink-0 items-end gap-2 pb-[env(safe-area-inset-bottom,0px)]";
export const touchField = `m-0 min-h-10 min-w-0 flex-1 resize-none rounded-[20px] ${touchHairline} bg-[var(--p-group-bg)] px-3.5 py-2 text-[16px] leading-[22px] [color:var(--p-text)] [font-family:inherit] outline-none placeholder:[color:var(--p-sub)]`;
export const touchSend =
  "m-0 flex size-10 shrink-0 cursor-pointer items-center justify-center rounded-full border-0 p-0 [background:var(--accent-color)] [color:var(--p-on-accent)] active:opacity-80 disabled:cursor-default disabled:opacity-40";
