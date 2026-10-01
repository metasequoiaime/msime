import { useId } from "react";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import { groupTitle } from "../core/platform-controls-style";
import { keyboardThemeId, themeEntry } from "../theme/global-theme";
import { useCandidatePreviewTheme } from "../candidate/candidate-preview-theme";
import type { Preferences, TouchKeyboardScheme } from "../index";
import { touchKeyboardSchemeTitle } from "../settings/touch-keyboard-scheme-helpers";

// Every tappable surface on this page is the same card: full width, a hairline that strengthens on
// hover, and the shared press animation. Named here rather than repeated at each of the five call
// sites, because a card that drifts from the others is the failure this page is prone to.
const card =
  "press-spring w-full rounded-[14px] border border-edge bg-card text-left text-body shadow-card hover:border-edge-strong active:scale-[0.96] active:opacity-[0.88] motion-reduce:transition-none";
// The cards that are a single row: icon, a growing middle, and a trailing chevron.
const rowCard = `${card} flex items-center justify-between gap-3 px-3.5 py-[13px]`;
const cardTitle = "block text-[15px] font-semibold";
const cardNote = "mt-1";
// The middle column of a row card: it takes the leftover width and is allowed to shrink, which is
// what lets the note beside it ellipsise instead of pushing the chevron off the edge.
const rowBody = "min-w-0 flex-1";
const rowChevron = "shrink-0 grow-0 basis-auto text-lg text-muted";
// The pages that have no tab of their own are one grouped list, not a stack of separate cards:
// thirteen floating cards is thirteen shadows and a screenful of gaps, and the source draws this as
// a single inset list with a hairline between the rows.
const listGroup = "overflow-hidden rounded-[14px] border border-edge bg-card shadow-card";
const listRow =
  "press-spring flex w-full items-center gap-3 px-3.5 py-[11px] text-left text-[15px] text-body hover:bg-raised active:opacity-[0.88] motion-reduce:transition-none not-first:border-t not-first:border-edge";

const quickTile =
  "press-spring flex min-w-0 flex-col items-start gap-[5px] rounded-[11px] border border-edge bg-subtle p-3 text-left text-body hover:border-edge-strong hover:bg-raised active:scale-[0.96] active:opacity-[0.88] motion-reduce:transition-none max-tight:px-2 max-tight:py-2.5";
const quickIcon =
  "grid size-[34px] place-items-center rounded-[11px] text-[17px] font-[650] leading-none";
const quickTitle = "text-[13px] font-semibold";
const quickNote =
  "m-0 w-full overflow-hidden text-ellipsis whitespace-nowrap max-tight:text-[11px]";

/**
 * A settings page that has no tab of its own.
 *
 * The phone bar holds the source's four tabs and nothing more, so every other page has to be
 * reachable from inside one of them. They land here, at the foot of the 键盘 tab, the same way the
 * source keeps them inside its own 键盘 tab rather than growing the bar.
 */
// The icon block on a shortcut tile.
//
// The Apple app draws all six in the one brand green, and the Android port followed it — see the
// note in platforms/android/res/values/colors.xml, which calls the six pastels out by name. Desktop
// keeps them, because there the colour is how a tile is told apart at a glance in a wider grid.
// The glyph goes with them: a text symbol falls back to the host's emoji font, which is where the
// plastic gear on the 系统设置 tile came from.
function QuickIcon({
  touch,
  desktopClass,
  glyph,
  icon,
}: {
  touch: boolean;
  desktopClass: string;
  glyph: string;
  icon: string;
}) {
  if (touch) {
    // Masked rather than drawn: these files carry their own pale blue, and the source's tiles are
    // one brand green. A mask takes the shape and leaves the colour to us.
    const mask = `url("${icon}") center / contain no-repeat`;
    return (
      <span className={`${quickIcon} bg-accent-soft`} aria-hidden="true">
        <span className="block size-[19px] bg-accent" style={{ mask, WebkitMask: mask }} />
      </span>
    );
  }
  return (
    <span className={`${quickIcon} ${desktopClass}`} aria-hidden="true">
      {glyph}
    </span>
  );
}

/** The same masked glyph as a tile carries, at the size a row uses. */
function RowIcon({
  touch,
  glyph,
  icon,
  chip = false,
}: {
  touch: boolean;
  glyph: string;
  icon: string;
  chip?: boolean;
}) {
  const shell = chip
    ? "grid size-[34px] shrink-0 grow-0 basis-[34px] place-items-center rounded-[10px] bg-accent-soft"
    : "grid size-[34px] shrink-0 grow-0 basis-[34px] place-items-center";
  if (!touch) {
    return (
      <span className={`${shell} text-[18px] text-accent`} aria-hidden="true">
        {glyph}
      </span>
    );
  }
  const mask = `url("${icon}") center / contain no-repeat`;
  return (
    <span className={shell} aria-hidden="true">
      <span className="block size-[19px] bg-accent" style={{ mask, WebkitMask: mask }} />
    </span>
  );
}

export interface MoreSettingsPage {
  id: string;
  title: string;
  icon: string;
}

/** 「全部设置」里的一组页面；`title` 是导航组名，为空时不显示组名。 */
export interface MoreSettingsGroup {
  title?: string;
  pages: readonly MoreSettingsPage[];
}

export interface HomePageActions {
  openKeyboard?: () => Promise<void>;
  openEmojiPanel?: () => Promise<void>;
  openClipboardPanel?: () => Promise<void>;
  openSystemKeyboardSettings?: () => Promise<void>;
  showInputMethodPicker?: () => Promise<void>;
}

export function HomePage({
  preferences,
  actions,
  onOpenPage,
  onSelectScheme,
  onOpenChat,
  touchLayout = false,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  onOpenChat?: () => void;
  onSelectScheme?: (scheme: TouchKeyboardScheme) => void;
  touchLayout?: boolean;
}) {
  const theme = useCandidatePreviewTheme(preferences.theme, preferences.screen_keyboard_theme);
  const selected = themeEntry(preferences.global_theme).id;
  const skin = keyboardThemeId(selected, preferences.custom_theme);
  const customDesign = preferences.custom_theme?.keyboard ?? undefined;
  const skinTitle = selected === "custom" ? "我的皮肤" : themeEntry(selected).title;
  const invokeAction = (action?: () => Promise<void>) => {
    if (action) void action();
  };
  const openKeyboard = () => {
    if (!actions?.openKeyboard) {
      // iOS has no separate desktop-style panel. Its Apple home card opens a
      // real text field so the system keyboard extension can be tried in place.
      if (onOpenChat) {
        onOpenChat();
        return;
      }
      onOpenPage("screen-keyboard");
      return;
    }
    void actions.openKeyboard().catch(() => {
      if (onOpenChat) onOpenChat();
      else onOpenPage("screen-keyboard");
    });
  };

  return (
    <section className="flex flex-col gap-3.5 pb-6" aria-label="首页">
      <header className="flex items-center justify-between gap-4 px-1 pt-2 pb-0.5">
        <div>
          <h2 className="m-0 text-2xl font-[650] tracking-[-0.02em] text-body max-tight:text-[21px]">
            让输入，更像你
          </h2>
          <p className="mt-1.5 mb-0 text-muted">从一次顺手的表达开始</p>
        </div>
        {/* The source's home page opens on the sentence and carries no brand mark — the app is
            already the thing you are looking at. Kept on desktop, where the header is a window
            chrome rather than the top of a phone screen. */}
        {!touchLayout && (
          <img
            className="size-12 opacity-80"
            src={new URL("../assets/msime.svg", import.meta.url).href}
            alt=""
          />
        )}
      </header>
      <button type="button" className={`${card} flex flex-col gap-3 p-4`} onClick={openKeyboard}>
        <div className="flex items-center justify-between gap-3">
          <span>
            <strong className={cardTitle}>我的键盘</strong>
            <small className={cardNote}>
              {skinTitle} · {touchKeyboardSchemeTitle(preferences)}
            </small>
          </span>
          <em className="shrink-0 grow-0 basis-auto rounded-full bg-accent-soft px-2 py-1 text-[11px] not-italic text-accent">
            当前外观
          </em>
        </div>
        <ScreenKeyboardPreview
          theme={theme}
          skin={skin}
          customDesign={customDesign}
          layout={touchLayout ? "touch" : "desktop"}
        />
        <span
          className={
            touchLayout
              ? "flex w-full items-center justify-center gap-2 rounded-[14px] bg-accent px-4 py-3 text-[15px] font-semibold text-white"
              : "flex items-center justify-between text-[13px] font-semibold text-accent"
          }
        >
          ⌨ 试用键盘 <span aria-hidden="true">→</span>
        </span>
      </button>
      <div className="grid grid-cols-3 gap-2.5 max-tight:gap-[7px]">
        <button type="button" className={quickTile} onClick={() => onOpenPage("skin")}>
          <QuickIcon
            touch={touchLayout}
            desktopClass="bg-[rgb(219_111_159/15%)] text-[#db6f9f]"
            glyph="◈"
            icon={new URL("../assets/skin.svg", import.meta.url).href}
          />
          <strong className={quickTitle}>主题</strong>
          <small className={quickNote}>{skinTitle}</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("input")}>
          <QuickIcon
            touch={touchLayout}
            desktopClass="bg-[rgb(25_167_141/15%)] text-[#19a78d]"
            glyph="⌨"
            icon={new URL("../assets/input.svg", import.meta.url).href}
          />
          <strong className={quickTitle}>输入方案</strong>
          <small className={quickNote}>{touchKeyboardSchemeTitle(preferences)}</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("screen-keyboard")}>
          <QuickIcon
            touch={touchLayout}
            desktopClass="bg-[rgb(119_114_223/15%)] text-[#7772df]"
            glyph="⌗"
            icon={new URL("../assets/screen-keyboard.svg", import.meta.url).href}
          />
          <strong className={quickTitle}>按键</strong>
          <small className={quickNote}>间距与语音</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("dictionary")}>
          <QuickIcon
            touch={touchLayout}
            desktopClass="bg-[rgb(152_112_90/15%)] text-[#98705a]"
            glyph="▤"
            icon={new URL("../assets/dictionary.svg", import.meta.url).href}
          />
          <strong className={quickTitle}>词库</strong>
          <small className={quickNote}>个人词与同步</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("ai")}>
          <QuickIcon
            touch={touchLayout}
            desktopClass="bg-[rgb(229_155_67/15%)] text-[#e59b43]"
            glyph="✦"
            icon={new URL("../assets/ai.svg", import.meta.url).href}
          />
          <strong className={quickTitle}>AI</strong>
          <small className={quickNote}>回复与润色</small>
        </button>
        <button
          type="button"
          className={quickTile}
          onClick={() =>
            actions?.openSystemKeyboardSettings
              ? invokeAction(actions.openSystemKeyboardSettings)
              : onOpenPage("screen-keyboard")
          }
        >
          <QuickIcon
            touch={touchLayout}
            desktopClass="bg-[rgb(130_136_146/15%)] text-[#747b86]"
            glyph="⚙"
            icon={new URL("../assets/utilities.svg", import.meta.url).href}
          />
          <strong className={quickTitle}>系统设置</strong>
          <small className={quickNote}>启用与完全访问</small>
        </button>
      </div>
      <button
        type="button"
        className={rowCard}
        onClick={() => {
          onSelectScheme?.("thoughtful_reply");
          onOpenPage("input");
        }}
      >
        <span
          className="grid size-[34px] shrink-0 grow-0 basis-[34px] place-items-center rounded-[10px] bg-accent-soft text-[18px] text-accent"
          aria-hidden="true"
        >
          ✦
        </span>
        <span className={rowBody}>
          <strong className={cardTitle}>高情商回复</strong>
          <small className={cardNote}>切换回复键盘，试试更合适的表达</small>
        </span>
        <span className={rowChevron} aria-hidden="true">
          ↗
        </span>
      </button>
      {onOpenChat && (
        <button type="button" className={rowCard} onClick={onOpenChat}>
          <RowIcon
            touch={touchLayout}
            glyph="◌"
            icon={new URL("../assets/help.svg", import.meta.url).href}
            chip
          />
          <span className={rowBody}>
            <strong className={cardTitle}>边聊天，边试键盘</strong>
            <small className={cardNote}>在共享账号中选择 EveryAPI 模型开始对话</small>
          </span>
          <span className={rowChevron} aria-hidden="true">
            →
          </span>
        </button>
      )}
      <button type="button" className={rowCard} onClick={() => onOpenPage("more")}>
        <RowIcon
          touch={touchLayout}
          glyph="⚙"
          icon={new URL("../assets/utilities.svg", import.meta.url).href}
        />
        <span className={rowBody}>
          <strong className={cardTitle}>全部设置</strong>
          <small className={cardNote}>打字、外观、语音与词库</small>
        </span>
        <span className={rowChevron} aria-hidden="true">
          ›
        </span>
      </button>
      <div className="flex flex-wrap gap-[9px]">
        {actions?.openEmojiPanel && (
          <button
            type="button"
            className="secondary m-0"
            onClick={() => invokeAction(actions.openEmojiPanel)}
          >
            表情与符号
          </button>
        )}
        {actions?.openClipboardPanel && (
          <button
            type="button"
            className="secondary m-0"
            onClick={() => invokeAction(actions.openClipboardPanel)}
          >
            剪贴板历史
          </button>
        )}
        {actions?.openSystemKeyboardSettings && (
          <button
            type="button"
            className="secondary m-0"
            onClick={() => invokeAction(actions.openSystemKeyboardSettings)}
          >
            系统键盘设置
          </button>
        )}
        {actions?.showInputMethodPicker && (
          <button
            type="button"
            className="secondary m-0"
            onClick={() => invokeAction(actions.showInputMethodPicker)}
          >
            选择输入法
          </button>
        )}
      </div>
    </section>
  );
}

/**
 * 没有独立标签的设置页。
 *
 * 从「键盘」标签进入，而不是在标签栏里加第五格：来源应用的标签栏只有四格，其余页面都在第一个标签下一层。每组画成一张带分隔线的列表，而不是每页一张卡片。
 *
 * 每组列表上方显示导航的组名（与桌面侧栏同一份 `settingsNavGroups`），手机上只剩一项的组也能看出它属于哪一类。
 */
export function MoreSettingsPage({
  groups,
  onOpenPage,
}: {
  /** 导航分组，每组画成组名下的一张列表。 */
  groups: readonly MoreSettingsGroup[];
  onOpenPage: (page: string) => void;
}) {
  return (
    <section className="flex flex-col gap-3" aria-label="全部设置">
      {groups.map((group) => (
        <MoreSettingsGroupList key={group.pages[0]?.id} group={group} onOpenPage={onOpenPage} />
      ))}
    </section>
  );
}

function MoreSettingsGroupList({
  group,
  onOpenPage,
}: {
  group: MoreSettingsGroup;
  onOpenPage: (page: string) => void;
}) {
  const titleId = useId();
  return (
    <div
      className="flex flex-col gap-[var(--p-g-title-gap)]"
      role={group.title ? "group" : undefined}
      aria-labelledby={group.title ? titleId : undefined}
    >
      {group.title && (
        <h3 id={titleId} className={groupTitle}>
          {group.title}
        </h3>
      )}
      <div className={listGroup}>
        {group.pages.map((item) => (
          <button
            key={item.id}
            type="button"
            className={listRow}
            onClick={() => onOpenPage(item.id)}
          >
            <img src={item.icon} alt="" aria-hidden="true" className="size-[20px] shrink-0" />
            <span className={rowBody}>
              <strong className="block font-medium">{item.title}</strong>
            </span>
            <span className={rowChevron} aria-hidden="true">
              ›
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
