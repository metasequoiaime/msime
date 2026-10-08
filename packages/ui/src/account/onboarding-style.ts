/**
 * The first-run walkthrough.
 *
 * It is the only surface in the app that owns the whole window rather than living inside the settings
 * chrome, which is why it sets its own background and centres a fixed measure: header, body and footer
 * all share one column so the text does not run the full width of a desktop window.
 */

const column = "mx-auto w-[min(560px,100%)]";

export const page =
  "flex min-h-full w-full flex-col overflow-y-auto bg-chrome px-[max(22px,5vw)] pt-7 pb-[22px] text-body max-tight:pt-[18px]";
export const header = `${column} flex items-center gap-4 [&>img]:size-[58px] [&>img]:rounded-[18px] [&>img]:bg-raised [&>img]:p-[11px] [&>h1]:m-0 [&>h1]:text-[21px] [&>h1]:font-[650]`;
export const skip = "ml-auto border-0 bg-transparent px-0 py-[5px] text-[13px] text-secondary";
export const progress = "mt-0 mb-[3px] text-xs tabular-nums text-accent";

export const body = `${column} flex-1 pt-7 pb-5 max-tight:pt-[22px]`;
export const section = "flex flex-col gap-[18px]";
export const sectionTitle = "m-0 text-[27px] font-bold tracking-[-0.02em] max-tight:text-2xl";
export const lead = "mt-[-8px] mb-1 text-[15px] leading-relaxed text-secondary";

/** A setup step: a number, a growing middle, and text that wraps inside it. */
const rowText =
  "[&_strong]:block [&_strong]:text-[15px] [&_strong]:font-semibold [&_small]:mt-1 [&_small]:text-[13px] [&_small]:leading-relaxed [&_small]:text-secondary";
/** The last step's perks: one card each, a tinted glyph and a label (dc.html `onbPerks`). */
export const perks = "m-0 flex list-none flex-col gap-2.5 p-0";
export const perk =
  "flex items-center gap-3 rounded-[14px] border border-edge bg-card px-3.5 py-3 text-[15px] [&>span]:grid [&>span]:size-8 [&>span]:flex-none [&>span]:place-items-center [&>span]:rounded-[9px] [&>span]:bg-accent-soft [&>span]:text-lg [&>span]:text-accent";
export const note = "mt-0.5 mb-0 text-xs leading-[1.7] text-muted";

export const setupCard =
  "flex flex-col gap-0 rounded-[18px] border border-edge bg-card px-[18px] py-[5px] shadow-card";
/** The connector between steps is drawn by the step itself, absolutely placed under its number. */
export const setupStep = `relative flex min-h-[76px] items-start gap-[13px] py-3.5 ${rowText} [&>span:nth-child(2)]:min-w-0 [&>span:nth-child(2)]:flex-1 [&>span:nth-child(2)]:pt-[3px] [&>i]:absolute [&>i]:top-11 [&>i]:left-3.5 [&>i]:h-[46px] [&>i]:w-0.5 [&>i]:bg-accent-soft-border`;
export const setupNumber =
  "grid size-[30px] flex-[0_0_30px] place-items-center rounded-full bg-accent-strong font-bold text-white";

export const systemActions = "flex flex-wrap gap-[9px] [&>button]:m-0";
/** 这一页的按钮比设置页的大：它们是屏幕上唯一的操作。HarmonyOS 的强调色随季节变化，深色季节的强调色偏浅，所以主按钮文字用强调色自己的 on 色。 */
export const buttons =
  "[&_.primary]:rounded-[10px] [&_.primary]:border [&_.primary]:border-accent [&_.primary]:bg-accent-strong [&_.primary]:px-[18px] [&_.primary]:py-2.5 [&_.primary]:text-white data-[platform=harmony]:[&_.primary]:[color:var(--p-on-accent)] data-[platform=hm2]:[&_.primary]:[color:var(--p-on-accent)] [&_.secondary]:m-0 [&_.secondary]:px-[18px] [&_.secondary]:py-2.5";

export const schemeList = "flex flex-col gap-[11px]";
export const schemeOption = (selected: boolean) =>
  `flex items-center gap-3.5 rounded-2xl border bg-card p-[17px] text-left text-body [&>span:first-child]:text-[23px] [&>span:first-child]:text-accent [&>span:nth-child(2)]:min-w-0 [&>span:nth-child(2)]:flex-1 [&_strong]:block [&_strong]:text-base [&_strong]:font-semibold [&_small]:mt-1 [&_b]:text-[21px] [&_b]:text-accent ${
    selected
      ? "border-accent shadow-[0_0_0_1px_var(--accent-color),var(--card-shadow)]"
      : "border-edge shadow-card"
  }`;

export const error = `${column} mt-0 mb-3 text-[13px] text-danger`;
export const footer = `${column} flex flex-col items-stretch gap-2.5`;
/** The page indicator. The current page's dot stretches rather than changing only colour. */
export const dots = "mb-[3px] flex justify-center gap-[7px]";
export const dot = (active: boolean) =>
  `h-2 rounded-full ${active ? "w-6 bg-accent" : "w-2 bg-[var(--border-color)]"}`;
export const next = "w-full";
export const back = "self-center border-0 bg-transparent px-[9px] py-[3px] text-secondary";
/** 稍后再说 under the primary action on the last step (dc.html `onb.hasAlt`). */
export const later =
  "m-0 self-center border-0 bg-transparent px-[9px] py-[3px] text-[15px] text-accent";

// ---- the modal sheet: iPad and the HarmonyOS 2-in-1 ----

/** On an iPad or a 2-in-1 the flow is a 480-wide sheet centred on the window rather than a full-width page (dc.html `onbModal`). */
export const sheetBackdrop =
  "relative flex min-h-full w-full items-center justify-center overflow-y-auto bg-chrome p-6 text-body";
/** The design dims whatever is behind the sheet to rgba(0,0,0,.38); the flow has nothing behind it but the window, so the window is what is dimmed. */
export const sheetScrim = "absolute inset-0 bg-black/38";
/** The sheet's corner is the platform's modal radius: 14 on an iPad, 24 on a 2-in-1 (dc.html `onb.r`). */
export const sheet = (desktop: boolean) =>
  `relative flex max-h-[min(600px,94vh)] w-[480px] max-w-full flex-col overflow-y-auto border border-edge bg-raised px-7 py-6 shadow-[0_30px_80px_rgba(0,0,0,0.4)] ${
    desktop ? "rounded-[24px]" : "rounded-[14px]"
  }`;

/** 把 2-in-1 按钮栏里的按钮推到「跳过」右侧。 */
export const deskSpacer = "flex-1";

// ---- Android: a linear progress bar over the page and a pair of buttons under it ----

export const progressTrack = `${column} mb-5 h-1 overflow-hidden rounded-full bg-[var(--border-color)]`;
export const progressFill =
  "block h-full rounded-full bg-accent transition-[width] duration-300 motion-reduce:transition-none";
export const pairFooter = `${column} flex items-center justify-between gap-3 pt-4 [&_button]:m-0 [&_.primary]:h-10 [&_.primary]:rounded-[20px]! [&_.primary]:px-6! [&_.primary]:py-0! [&_.primary]:text-sm [&_.primary]:font-medium`;
export const pairEnd = "flex items-center gap-2";
/** Material's text button: accent text, a tint on hover, no container (dc.html `ob.andLeft`). */
export const textButton =
  "h-10 rounded-[20px] border-0 bg-transparent px-3 text-sm font-medium text-accent hover:bg-accent-soft";

// ---- step 3: the candidate bar with its translation line ----

export const glossPreview =
  "flex gap-2 overflow-hidden rounded-2xl border border-edge bg-card p-3 shadow-card";
export const glossCandidate = (first: boolean) =>
  `flex min-w-0 flex-col items-center rounded-[10px] px-3 py-2 ${first ? "bg-accent-soft" : ""} [&>span]:text-[17px] [&>small]:mt-0.5 [&>small]:text-[11px] [&>small]:text-secondary`;
export const glossRow =
  "flex items-center justify-between gap-4 rounded-2xl border border-edge bg-card px-[17px] py-3.5 shadow-card [&_strong]:block [&_strong]:text-[15px] [&_strong]:font-semibold [&_small]:mt-1 [&_small]:block [&_small]:text-[13px] [&_small]:leading-relaxed [&_small]:text-secondary";

// ---- the splash ----

/** The window behind the splash, so the near-black reaches every edge rather than stopping at the flow's padding. */
export const splashShell = "flex min-h-full w-full bg-[#0E100E]";
/** The first-launch splash (dc.html L1740-1745): the mark in its frame over a green glow, the name and the Latin name under it, whatever the theme. */
export const splash =
  "relative flex min-h-full w-full cursor-pointer flex-col items-center justify-center gap-[18px] overflow-hidden border-0 bg-[#0E100E] p-0 text-white";
export const splashGlow =
  "absolute size-80 rounded-full bg-[radial-gradient(circle,rgba(127,224,142,0.28)_0%,rgba(127,224,142,0)_65%)] animate-splash-glow motion-reduce:animate-none";
export const splashLogo = "relative size-[124px] animate-splash-pop motion-reduce:animate-none";
export const splashFrame = "fill-[#252525] stroke-[#A8DF8E] [stroke-width:4]";
export const splashStroke =
  "fill-none stroke-white [stroke-dasharray:1] [stroke-linecap:round] [stroke-width:9] animate-splash-draw motion-reduce:animate-none";
export const splashName =
  "relative text-2xl font-bold tracking-[0.06em] animate-splash-name motion-reduce:animate-none";
export const splashTagline =
  "relative text-[13px] tracking-[0.18em] text-white/62 animate-splash-tagline motion-reduce:animate-none";
export const splashHint =
  "absolute bottom-[max(36px,env(safe-area-inset-bottom))] text-xs text-white/40 animate-splash-hint motion-reduce:animate-none";

// ---- HarmonyOS（dc.html `flowOnb`）：手机页面和 2-in-1 弹窗都读取设置根页面所用的平台 token，所以引导流程跟随系统外观和用户的应用主题 ----

/** 手机页面（dc.html `onb.pad` 8 24 28）：计数行、可滚动的主体和居中的底栏。页面接收横向滑动，所以浏览器只保留纵向平移，滑动中也不会选中文字。 */
export const hPage =
  "flex min-h-full w-full touch-pan-y flex-col bg-[var(--p-bg)] px-6 pt-2 pb-7 [font-family:var(--p-font)] text-[var(--p-text)] select-none";
/** 步骤上方的一行：左边是计数，右边是「跳过」，每一步都有（dc.html `ob.topSkip`）。 */
export const hTopRow = `${column} flex h-8 flex-none items-center justify-between`;
export const hCounter = "text-[13px] tabular-nums text-[var(--p-sub)]";
export const hSkip =
  "m-0 border-0 bg-transparent p-0 text-[15px] text-[var(--p-accent-text)] active:opacity-70";
/** 2-in-1 弹窗里步骤上方的计数（dc.html `ob.countTxt`），「跳过」改放在按钮栏里。 */
export const hDeskCounter = "m-0 text-xs text-[var(--p-sub)]";

/** 一个步骤：图标块、眉题、标题和正文，然后是该步骤自己的内容（dc.html gap 14、padding-top 28；2-in-1 相同）。 */
export const hBody = (desktop: boolean) =>
  `${column} grid flex-1 content-start gap-3.5 [grid-template-columns:minmax(0,1fr)] ${desktop ? "pt-4" : "pt-7"}`;
export const hIconTile = (desktop: boolean) =>
  `flex items-center justify-center bg-[var(--accent-soft)] text-[var(--p-accent-text)] ${
    desktop ? "size-[52px] rounded-[14px]" : "size-[72px] rounded-[22px]"
  }`;
export const hKicker = "text-[13px] font-semibold tracking-[0.04em] text-[var(--p-accent-text)]";
export const hTitle = (desktop: boolean) =>
  `m-0 leading-[1.25] font-bold ${desktop ? "text-xl" : "text-[28px]"}`;
export const hLead = "m-0 text-[15px] leading-[1.6] text-[var(--p-sub)] [text-wrap:pretty]";
/** 步骤内容位于正文下方 6 处（dc.html `margin-top: 6px`）。 */
export const hContent = "mt-1.5 flex flex-col";

/** 页面上的卡片：平台的分组颜色，无边框无阴影，手机圆角 20，2-in-1 圆角 14（dc.html `onb.cardR`）。 */
const hCard = (desktop: boolean) =>
  `bg-[var(--p-group-bg)] ${desktop ? "rounded-[14px]" : "rounded-[20px]"}`;

// 第 1 步：两项设置检查。
export const hChecks = (desktop: boolean) => `${hCard(desktop)} flex flex-col overflow-hidden`;
export const hCheck = (divider: boolean) =>
  `flex items-center gap-3 p-3.5 ${divider ? "border-t border-[var(--p-hair)]" : ""}`;
/** 检查项前的 24px 标记：完成时是强调色的勾，待处理时是橙色的 !，宿主尚未回报时是空心圆环。 */
export const hCheckBadge = (done: boolean | null) =>
  `flex size-6 flex-none items-center justify-center rounded-full text-[13px] font-semibold ${
    done === true
      ? "bg-[var(--accent-color)] text-[var(--p-on-accent)]"
      : done === false
        ? "bg-[#FF9500] text-white"
        : "[box-shadow:inset_0_0_0_1.5px_var(--p-hair)]"
  }`;
export const hCheckLabel = "min-w-0 flex-1 text-base";
export const hCheckAction =
  "m-0 flex-none border-0 bg-transparent p-0 text-[15px] text-[var(--p-accent-text)] active:opacity-70";
/** 没有设置客户端时，该步骤保留带编号的说明，放在一张使用本平台颜色的卡片里。 */
export const hSetupCard = (desktop: boolean) =>
  `${hCard(desktop)} flex flex-col px-[18px] py-[5px]`;
export const hSystemActions = "mt-3 flex flex-wrap gap-[9px] [&>button]:m-0";
export const hNote = "mt-3 mb-0 text-xs leading-[1.7] text-[var(--p-sub)]";

// 第 2 步：每个方案一张卡片，右侧是单选按钮。
export const hSchemes = "flex flex-col gap-2.5";
/** 方案卡片；选中的那张带 2px 内嵌强调色圆环（dc.html `o.ring`）。 */
export const hScheme = (desktop: boolean, selected: boolean) =>
  `${hCard(desktop)} m-0 flex w-full items-center gap-3 border-0 px-4 py-3.5 text-left text-[var(--p-text)] ${
    selected ? "[box-shadow:inset_0_0_0_2px_var(--accent-color)]" : ""
  }`;
export const hSchemeText =
  "flex min-w-0 flex-1 flex-col gap-0.5 [&>strong]:text-base [&>strong]:font-semibold [&>span]:text-[13px] [&>span]:text-[var(--p-sub)]";
/** 22px 单选按钮：选中时是围着浅色圆心的 6px 强调色圆环，未选中时是 1.5px 的次要色圆环（dc.html `o.dot`）。 */
export const hRadio = (selected: boolean) =>
  `box-border size-[22px] flex-none rounded-full ${
    selected
      ? "border-[6px] border-solid border-[var(--accent-color)]"
      : "border-[1.5px] border-solid border-[var(--p-sub)]"
  }`;

// 第 3 步：候选条的静态画面，以及它所预览的开关。
export const hGlossStrip = (desktop: boolean) =>
  `flex gap-1 overflow-hidden px-2.5 py-3 ${desktop ? "rounded-[14px]" : "rounded-[20px]"}`;
export const hGlossCell = (first: boolean) =>
  `flex flex-none flex-col items-center px-2.5 leading-[1.25] [&>span]:text-[19px] ${
    first
      ? "[&>span]:font-semibold [&>span]:text-[var(--p-accent-text)]"
      : "[&>span]:font-normal [&>span]:text-[var(--p-text)]"
  } [&>small]:m-0 [&>small]:text-[11px] [&>small]:leading-[1.25]`;
export const hGlossRow = (desktop: boolean) =>
  `${hCard(desktop)} mt-3 flex items-center justify-between gap-4 px-3.5 py-3 text-base`;

// 第 4 步：登录带来的好处。
export const hPerks = "m-0 flex list-none flex-col gap-2.5 p-0";
export const hPerk = (desktop: boolean) =>
  `${hCard(desktop)} flex items-center gap-3 px-3.5 py-3 text-[15px]`;
export const hPerkTile =
  "flex size-8 flex-none items-center justify-center rounded-[9px] bg-[var(--accent-soft)] text-[var(--p-accent-text)]";

export const hError = `${column} mt-3 mb-0 text-[13px] text-danger`;

/** 手机底栏（dc.html `ob.cent`）：页码圆点、全宽操作按钮，最后一步还有「稍后再说」。 */
export const hFooter = `${column} flex flex-none flex-col items-center gap-3.5 pt-4`;
export const hDots = "flex gap-1.5";
export const hDot = (active: boolean) =>
  `h-1.5 rounded-[3px] ${active ? "w-5 bg-[var(--accent-color)]" : "w-1.5 bg-[var(--p-hair)]"}`;
export const hCta =
  "m-0 h-[50px] w-full rounded-[25px] border-0 bg-[var(--accent-color)] p-0 text-[17px] font-semibold text-[var(--p-on-accent)] active:opacity-85";
export const hLater =
  "m-0 border-0 bg-transparent p-0 text-[15px] text-[var(--p-accent-text)] active:opacity-70";

// 2-in-1：使用本平台颜色的弹窗及其按钮栏（dc.html `onbDeskBar`）。

export const hSheetBackdrop =
  "relative flex min-h-full w-full items-center justify-center overflow-y-auto bg-[var(--p-bg)] p-6 text-[var(--p-text)]";
/** 弹窗的 24 圆角就是 2-in-1 的模态圆角（dc.html `onb.r`）；弹窗放在平台的 chrome 灰底上，让里面的白色卡片显得突出。 */
export const hSheet =
  "relative flex max-h-[min(600px,94vh)] w-[480px] max-w-full flex-col overflow-y-auto rounded-[24px] bg-[var(--p-chrome)] px-7 py-6 [font-family:var(--p-font)] text-[var(--p-text)] shadow-[0_30px_80px_rgba(0,0,0,0.4)]";
export const hDeskBar = `${column} mt-4 flex flex-none items-center gap-2 [&>button]:m-0 [&>button]:h-9 [&>button]:border-0 [&>button]:text-[13px]`;
export const hDeskLink = "bg-transparent px-3.5 text-[var(--p-accent-text)]";
/** 「上一步」：强调色下的浅色底（dc.html `ob.secBg`），深色模式下是一层白色薄纱。 */
export const hDeskBack = (dark: boolean) =>
  `min-w-18 rounded-[18px] px-4 text-[var(--accent-color)] ${dark ? "bg-white/10" : "bg-black/6"}`;
export const hDeskNext =
  "min-w-18 rounded-[18px] bg-[var(--accent-color)] px-[18px] font-medium text-[var(--p-on-accent)]";

// ---- HarmonyOS 启动页（dc.html L2692-2698）：圆盘落在带强调色调的底色上，标志旋转入场并描画出来，底部一条进度条逐渐填满 ----

/** 底色：强调色混入近黑色，所以随平台和季节强调色变化。 */
export const hSplashShell =
  "flex min-h-full w-full bg-[color-mix(in_srgb,var(--accent-color)_20%,#0A0B0A)]";
export const hSplash =
  "relative flex min-h-full w-full cursor-pointer flex-col items-center justify-center gap-[18px] overflow-hidden border-0 bg-[color-mix(in_srgb,var(--accent-color)_20%,#0A0B0A)] p-0 text-white";
export const hSplashGlow =
  "absolute size-80 rounded-full bg-[radial-gradient(circle,color-mix(in_srgb,var(--accent-color)_34%,transparent)_0%,transparent_65%)] animate-splash-glow motion-reduce:animate-none";
/** 180px 圆盘、它的涟漪圆环和标志共用一个盒子，作为整体落下。 */
export const hSplashDiscBox =
  "relative size-[180px] flex-none animate-splash-circ-in motion-reduce:animate-none";
/** 圆盘后的白色光晕，放大到 1.2 倍并淡出，共两次（dc.html `msRipple`）；动画前后都保持不可见。 */
export const hSplashRipple =
  "absolute inset-0 rounded-full bg-white/70 opacity-0 animate-splash-ripple motion-reduce:hidden";
export const hSplashDisc =
  "absolute inset-0 rounded-full bg-[color-mix(in_srgb,var(--accent-color)_12%,#FFFFFF)] shadow-[0_12px_40px_rgba(0,0,0,0.35)]";
export const hSplashLogo =
  "absolute top-10 left-10 size-[100px] animate-splash-logo-in motion-reduce:animate-none";
export const hSplashFrame = "fill-[color-mix(in_srgb,var(--accent-color)_82%,#000)]";
export const hSplashStroke =
  "fill-none stroke-white [stroke-dasharray:1] [stroke-linecap:round] [stroke-linejoin:round] [stroke-width:9] animate-splash-logo-draw motion-reduce:animate-none";
/** 加载条：距两侧和底部 48 的 3px 轨道，在启动页停留的时间内逐渐填满（dc.html `msLoad`）。 */
export const hSplashTrack =
  "absolute right-12 bottom-12 left-12 h-[3px] overflow-hidden rounded-[2px] bg-white/8";
export const hSplashFill =
  "block h-full w-full origin-left bg-[color-mix(in_srgb,var(--accent-color)_70%,#FFFFFF)] animate-splash-load motion-reduce:animate-none";
