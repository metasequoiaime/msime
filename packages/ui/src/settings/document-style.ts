/**
 * The prose pages -- about, the guides, feedback -- and the AI skin generator's cards.
 *
 * These are the surfaces built out of running text rather than controls, so the shapes here are
 * mostly about measure and rhythm: a hero, subsections divided by a rule, and rows that read as a
 * list even though each one is a button.
 */

export const page = "leading-[1.7] [&>p]:mt-0 [&>p]:mb-3.5 [&>p]:text-secondary";
export const subsection =
  "mt-6 border-t border-[var(--divider-color)] pt-[18px] [&>p:last-child]:mb-0";
/** The page's lead: the mark or eyebrow, a title and a line of prose, as the first block of an untitled group. */
export const hero =
  "flex items-center gap-[18px] px-5 py-5 [&_p]:mt-[7px] [&_p]:mb-0 [&_p]:[font-size:var(--p-sub-fs)] [&_p]:[color:var(--p-sub)]";
export const eyebrow = "text-xs tracking-[0.04em] [color:var(--p-sub)]";
export const heroTitle = "mt-0.5 text-[22px] font-semibold [color:var(--p-text)]";
export const note = "flex flex-col gap-[5px] text-secondary [&>span]:text-xs [&>span]:text-muted";

/**
 * The guide cards: a term on the left, what it does on the right. The macOS reference window leads
 * each card with a single row, rules it off, and runs the rest together; the first row is the one
 * that carries the whole card, so it gets the air.
 */
export const guide = "flex flex-col gap-3";
export const guideRow =
  "grid grid-cols-[minmax(0,170px)_minmax(0,1fr)] items-baseline gap-5 max-narrow:grid-cols-1 max-narrow:gap-1";
export const guideLead = "border-b border-[var(--divider-color)] pb-3.5";
export const guideTerm = "text-body";
export const guideText = "m-0! text-secondary";

export const mark =
  "flex size-[62px] items-center justify-center rounded-2xl bg-raised p-2.5 [&>img]:size-12";
/** A row of a group that opens a page or a link; the group draws the dividers between rows. */
export const linkRow =
  "flex min-h-[var(--p-row-h)] w-full cursor-pointer items-center justify-between gap-4 rounded-[var(--p-row-r)] border-0 bg-[var(--p-row-bg)] text-left [padding:var(--p-row-pad)] [color:var(--p-sub)] hover:bg-[var(--p-hover)]";
export const linkTitle = "[font-size:var(--p-row-fs)] [color:var(--p-text)]";

export const version = "mt-1 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";
export const versionRow = "cursor-default items-start hover:bg-[var(--p-row-bg)]";
export const updateButton = "mt-0 shrink-0 grow-0 basis-auto";
export const updateStatus = "mt-[5px]! mb-0! text-xs";
export const updateResult =
  "[padding:var(--p-row-pad)] [color:var(--p-sub)] [&>p]:mt-0 [&>p]:mb-2 [&>p]:text-xs [&_code]:inline-block [&_code]:max-w-full [&_code]:break-anywhere";
export const updateWarning = "text-danger!";

// ---- HarmonyOS 的「关于」和「反馈」页 ----

/** 「关于」页首卡片的主体：标志圆、文字列、更新胶囊；在窄屏手机上胶囊换行到文字下方。 */
export const appHero = "flex flex-wrap items-center gap-4 p-[18px]";
/** 设计里的 `logoCirc`：强调色混入卡片底色，浅色模式 14%，深色模式 22%。 */
export const appHeroCircle =
  "flex size-20 shrink-0 items-center justify-center rounded-full bg-[color-mix(in_srgb,var(--accent-color)_22%,var(--p-group-bg))] light-theme:bg-[color-mix(in_srgb,var(--accent-color)_14%,var(--p-group-bg))]";
export const appHeroText = "flex min-w-[160px] flex-1 flex-col gap-[3px]";
export const appHeroTitle = "text-[18px] font-bold [color:var(--p-text)]";
export const appHeroVersion = "text-[13px] [color:var(--p-sub)]";
export const appHeroCopyright = "text-[12px] [color:var(--p-sub)]";
export const appHeroStatus = "m-0 text-[12px] [color:var(--p-sub)]";
/** 检查更新胶囊：强调色底配强调色上的前景文字；检查发现当前已是最新版本后改为无底色的强调色文字。检查期间保持完整颜色，此时禁用只是为了防止重复点击。 */
export const updatePill = (latest: boolean) =>
  `shrink-0 cursor-pointer rounded-[20px] border-0 px-4 py-[7px] text-[13px] whitespace-nowrap [font-family:inherit] disabled:cursor-default disabled:opacity-100 ${
    latest
      ? "bg-transparent [color:var(--p-accent-text)]"
      : "bg-[var(--accent-color)] [color:var(--p-on-accent)]"
  }`;
/** 行尾的文字按钮（查看），与其他改版后的行一样使用平台的按钮 token 绘制。 */
export const rowButton =
  "shrink-0 cursor-pointer rounded-[var(--p-r-ctl)] px-3.5 py-[5px] text-[13px] whitespace-nowrap [background:var(--p-btn-bg)] [border:var(--p-btn-border)] [color:var(--p-btn-fg)] [font-family:inherit] active:opacity-60 disabled:cursor-default disabled:opacity-50";

/** 「系统信息」行尾的值：次要文字色，右对齐，长的发行版名或设备型号在行内折行，可以选中复制。 */
export const systemInfoValue =
  "max-w-[60%] text-right break-anywhere select-text [font-size:var(--p-sub-fs)] [color:var(--p-sub)]";

/** 「描述」卡片：用分组的底色和圆角包住一个无边框的 textarea。 */
export const feedbackTextCard = "overflow-hidden rounded-[20px] bg-[var(--p-group-bg)]";
export const feedbackTextarea =
  "block min-h-[150px] w-full resize-none border-0 bg-transparent px-4 py-3.5 text-[17px] leading-normal outline-none [color:var(--p-text)] [font-family:inherit] placeholder:[color:var(--p-sub)]";
export const feedbackCounter = "px-4 text-[12px] [color:var(--p-sub)]";
/** 通栏提交按钮：有内容可发送时为强调色，否则为浅底灰字。灰色状态就是禁用状态，所以保持完全不透明，不套用全局的禁用淡化。 */
export const feedbackSubmit = (ready: boolean) =>
  `mt-3.5 flex h-[50px] w-full items-center justify-center rounded-[14px] border-0 text-[17px] font-semibold [font-family:inherit] disabled:cursor-default disabled:opacity-100 ${
    ready
      ? "cursor-pointer bg-[var(--accent-color)] [color:var(--p-on-accent)] active:opacity-80"
      : "bg-[rgba(255,255,255,0.08)] [color:var(--p-sub)] light-theme:bg-[rgba(0,0,0,0.06)]"
  }`;
export const feedbackError = "m-0 px-4 pt-2 text-[13px] text-danger";

/** Icon, body, action. The icon column narrows on a phone, where 42px of gutter is a lot to give up. */
export const feedbackCard =
  "grid min-h-[var(--p-row-h)] grid-cols-[42px_minmax(0,1fr)_auto] items-center gap-3.5 [padding:var(--p-row-pad)] max-phone:grid-cols-[36px_minmax(0,1fr)] [&>.secondary]:mt-0 [&>.secondary]:whitespace-nowrap [&>.secondary]:max-phone:col-start-2 [&>.secondary]:max-phone:justify-self-start";
export const feedbackIcon =
  "flex size-[38px] items-center justify-center rounded-[10px] border [border-color:var(--p-hair)]";
export const feedbackBody =
  "min-w-0 [&>p]:mt-[3px] [&>p]:mb-[5px] [&>p]:[font-size:var(--p-sub-fs)] [&>p]:[color:var(--p-sub)] [&_code]:block [&_code]:break-anywhere [&_code]:text-xs [&_code]:[color:var(--p-sub)]";
export const feedbackTitle = "[font-size:var(--p-row-fs)] font-semibold [color:var(--p-text)]";

// ---- the AI skin generator ----

export const generation = "w-[min(760px,100%)]";
export const generationNote = "m-0 text-xs leading-normal text-muted";
/** Three face-down cards, fanned. The rotation is what makes them read as a draw rather than a grid. */
export const mysteryCards = "grid grid-cols-3 gap-2.5 py-3";
export const mysteryCard = (index: number) =>
  `flex min-h-[130px] flex-col items-center justify-between rounded-[14px] p-3.5 text-[11px] tracking-[0.08em] text-white [&>span]:text-[34px] [&>span]:font-light [&>span]:tracking-normal ${
    index === 1
      ? "-translate-y-1.5 bg-[linear-gradient(145deg,#5b528d,#29234c)]"
      : index === 2
        ? "rotate-4 bg-[linear-gradient(145deg,#9b6b45,#4c2e26)]"
        : "-rotate-4 bg-[linear-gradient(145deg,#4d806b,#1a493c)]"
  }`;
export const cardList = "grid grid-cols-3 gap-3 max-narrow:grid-cols-1";
export const card =
  "flex min-w-0 flex-col gap-2 rounded-xl border border-edge bg-subtle p-3 [&>h3]:m-0 [&>h3]:text-sm [&>p]:m-0 [&>p]:min-h-[52px] [&>p]:text-[11px] [&>p]:leading-normal [&>p]:text-muted max-narrow:[&>p]:min-h-0 [&_.screen-keyboard-artwork]:min-h-40 [&_.screen-keyboard-artwork]:w-full";
export const cardActions = "flex flex-wrap gap-1.5 [&>button]:m-0 [&>button]:flex-[1_1_100%]";
export const publishForm =
  "mt-1 flex flex-col gap-2.5 rounded-[10px] border border-edge-strong bg-raised p-3 [&>h3]:m-0 [&>p]:m-0";
