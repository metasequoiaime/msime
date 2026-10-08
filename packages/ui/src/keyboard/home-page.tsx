import { useId, useState, type KeyboardEvent } from "react";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import { groupTitle } from "../core/platform-controls-style";
import { keyboardThemeId, themeEntry } from "../theme/global-theme";
import { useCandidatePreviewTheme } from "../candidate/candidate-preview-theme";
import { ActionButton } from "../core/action-button";
import { FluentIcon } from "../core/fluent-icons";
import { NavGroup, NavRow } from "../core/platform-controls";
import type { ImeSetupClient } from "../core/host-contracts";
import type { Preferences } from "../index";
import { touchKeyboardSchemeTitle } from "../settings/touch-keyboard-scheme-helpers";
import { SetupStatusCard } from "./setup-status-card";
import { TryKeyboardSheet } from "./try-keyboard-sheet";
import {
  RootSettingsList,
  RootSettingsRowView,
  rootSettingsGroups,
  skinTitle as rootSkinTitle,
  type RootPage,
} from "./root-settings-list";

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
// 快捷方式格子上的图标块。桌面端保留六种浅色，因为在更宽的网格里，颜色是一眼区分格子的方式；触屏首页已经没有格子，它的页面是「设置」根页的行。
function QuickIcon({ desktopClass, glyph }: { desktopClass: string; glyph: string }) {
  return (
    <span className={`${quickIcon} ${desktopClass}`} aria-hidden="true">
      {glyph}
    </span>
  );
}

/** 桌面端行卡片打头的图标，尺寸与格子的图标块相同。 */
function RowIcon({ glyph, chip = false }: { glyph: string; chip?: boolean }) {
  const shell = chip
    ? "grid size-[34px] shrink-0 grow-0 basis-[34px] place-items-center rounded-[10px] bg-accent-soft"
    : "grid size-[34px] shrink-0 grow-0 basis-[34px] place-items-center";
  return (
    <span className={`${shell} text-[18px] text-accent`} aria-hidden="true">
      {glyph}
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
  /** 本输入法是否已启用、是否为当前输入法，供触屏首页的状态卡片使用；没有它的宿主照常显示卡片并提供两个步骤。 */
  setup?: ImeSetupClient;
}

export function HomePage({
  preferences,
  actions,
  onOpenPage,
  onOpenChat,
  touchLayout = false,
  ios = false,
  rootPages,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  onOpenChat?: () => void;
  touchLayout?: boolean;
  /** iOS opens the keyboard extension's settings, where 完全访问 lives; Android and HarmonyOS open the system input method settings. */
  ios?: boolean;
  /** 触屏首页以分组行列出的页面，标题与手机上显示的一致；桌面首页忽略它。 */
  rootPages?: readonly RootPage[];
}) {
  if (touchLayout)
    return (
      <TouchHomePage
        preferences={preferences}
        actions={actions}
        onOpenPage={onOpenPage}
        rootPages={rootPages}
      />
    );
  return (
    <DesktopHomePage
      preferences={preferences}
      actions={actions}
      onOpenPage={onOpenPage}
      onOpenChat={onOpenChat}
      ios={ios}
    />
  );
}

const searchPill =
  "flex h-10 shrink-0 cursor-text items-center gap-2 rounded-full bg-[rgba(255,255,255,0.1)] px-3.5 [color:var(--p-sub)] light-theme:bg-[rgba(0,0,0,0.05)]";
const searchField =
  "m-0 h-full min-w-0 flex-1 border-0 bg-transparent p-0 text-[16px] [color:var(--p-text)] [font-family:inherit] outline-none placeholder:[color:var(--p-sub)] [&::-webkit-search-cancel-button]:hidden";

/**
 * 触屏首页，即「设置」根页（大标题由外壳绘制）：一个搜索胶囊、设置状态卡片，以及分组行形式的各页面。有查询时隐藏卡片和分组，列出标题或当前值包含查询的行，与 Android 的「设置」页一样；Escape 清空查询。
 *
 * 「试用键盘」在宿主有对应窗口时打开宿主的键盘，否则打开页面内的试用面板；宿主打开窗口失败时也回退到这里。
 */
function TouchHomePage({
  preferences,
  actions,
  onOpenPage,
  rootPages,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  rootPages?: readonly RootPage[];
}) {
  const [query, setQuery] = useState("");
  const [trying, setTrying] = useState(false);
  const groups = rootPages ? rootSettingsGroups(rootPages, preferences) : [];
  const needle = query.trim().toLowerCase();
  const matches = needle
    ? groups
        .flat()
        .filter(
          (row) =>
            row.title.toLowerCase().includes(needle) ||
            Boolean(row.value?.toLowerCase().includes(needle)),
        )
    : [];

  const tryKeyboard = () => {
    const openKeyboard = actions?.openKeyboard;
    if (!openKeyboard) {
      setTrying(true);
      return;
    }
    void openKeyboard().catch(() => setTrying(true));
  };
  const searchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Escape" || !query) return;
    event.preventDefault();
    event.stopPropagation();
    setQuery("");
  };

  return (
    <section className="flex flex-col gap-5" aria-label="首页">
      <label className={searchPill}>
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          aria-hidden="true"
          className="shrink-0"
        >
          <circle cx="7" cy="7" r="5" />
          <path d="m11 11 3.5 3.5" />
        </svg>
        <input
          type="search"
          className={searchField}
          placeholder="搜索"
          aria-label="搜索设置"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={searchKeyDown}
        />
      </label>
      {needle ? (
        matches.length > 0 ? (
          <NavGroup>
            {matches.map((row) => (
              <RootSettingsRowView key={row.id} row={row} onOpenPage={onOpenPage} />
            ))}
          </NavGroup>
        ) : (
          <p className="m-0 px-1 text-[14px] [color:var(--p-sub)]" role="status">
            没有匹配的设置
          </p>
        )
      ) : (
        <>
          <SetupStatusCard actions={actions} preferences={preferences} onTry={tryKeyboard} />
          {rootPages ? (
            <RootSettingsList groups={groups} onOpenPage={onOpenPage} />
          ) : (
            // 外壳没有说明自己有哪些页面时，这些页面仍能通过「全部设置」列表到达。
            <NavGroup>
              <NavRow
                icon={<FluentIcon name="settings" size={20} />}
                title="全部设置"
                onClick={() => onOpenPage("more")}
              />
            </NavGroup>
          )}
        </>
      )}
      {trying && <TryKeyboardSheet actions={actions} onClose={() => setTrying(false)} />}
    </section>
  );
}

function DesktopHomePage({
  preferences,
  actions,
  onOpenPage,
  onOpenChat,
  ios,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  onOpenChat?: () => void;
  ios: boolean;
}) {
  const theme = useCandidatePreviewTheme(preferences.theme, preferences.screen_keyboard_theme);
  const selected = themeEntry(preferences.global_theme).id;
  const skin = keyboardThemeId(selected, preferences.custom_theme);
  const customDesign = preferences.custom_theme?.keyboard ?? undefined;
  const skinTitle = rootSkinTitle(preferences);
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
        {/* 桌面端把品牌标志保留在句子旁，那里的标题栏是窗口装饰，而不是手机屏幕的顶部。 */}
        <img
          className="size-12 opacity-80"
          src={new URL("../assets/msime.svg", import.meta.url).href}
          alt=""
        />
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
          layout="desktop"
        />
        <span className="flex items-center justify-between text-[13px] font-semibold text-accent">
          ⌨ 试用键盘 <span aria-hidden="true">→</span>
        </span>
      </button>
      <div className="grid grid-cols-3 gap-2.5 max-tight:gap-[7px]">
        <button type="button" className={quickTile} onClick={() => onOpenPage("skin")}>
          <QuickIcon desktopClass="bg-[rgb(219_111_159/15%)] text-[#db6f9f]" glyph="◈" />
          <strong className={quickTitle}>主题</strong>
          <small className={quickNote}>{skinTitle}</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("input")}>
          <QuickIcon desktopClass="bg-[rgb(25_167_141/15%)] text-[#19a78d]" glyph="⌨" />
          <strong className={quickTitle}>输入方案</strong>
          <small className={quickNote}>{touchKeyboardSchemeTitle(preferences)}</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("screen-keyboard")}>
          <QuickIcon desktopClass="bg-[rgb(119_114_223/15%)] text-[#7772df]" glyph="⌗" />
          <strong className={quickTitle}>按键</strong>
          <small className={quickNote}>间距与语音</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("dictionary")}>
          <QuickIcon desktopClass="bg-[rgb(152_112_90/15%)] text-[#98705a]" glyph="▤" />
          <strong className={quickTitle}>词库</strong>
          <small className={quickNote}>个人词与同步</small>
        </button>
        <button type="button" className={quickTile} onClick={() => onOpenPage("ai")}>
          <QuickIcon desktopClass="bg-[rgb(229_155_67/15%)] text-[#e59b43]" glyph="✦" />
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
          <QuickIcon desktopClass="bg-[rgb(130_136_146/15%)] text-[#747b86]" glyph="⚙" />
          <strong className={quickTitle}>系统设置</strong>
          <small className={quickNote}>{ios ? "启用与完全访问" : "启用与设为默认"}</small>
        </button>
      </div>
      <button type="button" className={rowCard} onClick={() => onOpenPage("ai")}>
        <span
          className="grid size-[34px] shrink-0 grow-0 basis-[34px] place-items-center rounded-[10px] bg-accent-soft text-[18px] text-accent"
          aria-hidden="true"
        >
          ✦
        </span>
        <span className={rowBody}>
          <strong className={cardTitle}>高情商回复</strong>
          <small className={cardNote}>点键盘工具栏上的回复，试试更合适的表达</small>
        </span>
        <span className={rowChevron} aria-hidden="true">
          ↗
        </span>
      </button>
      {onOpenChat && (
        <button type="button" className={rowCard} onClick={onOpenChat}>
          <RowIcon glyph="◌" chip />
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
        <RowIcon glyph="⚙" />
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
          <ActionButton
            action={() => invokeAction(actions.openEmojiPanel)}
            className="secondary m-0"
            label="表情与符号"
          />
        )}
        {actions?.openClipboardPanel && (
          <ActionButton
            action={() => invokeAction(actions.openClipboardPanel)}
            className="secondary m-0"
            label="剪贴板历史"
          />
        )}
        {actions?.openSystemKeyboardSettings && (
          <ActionButton
            action={() => invokeAction(actions.openSystemKeyboardSettings)}
            className="secondary m-0"
            label={ios ? "系统键盘设置" : "系统输入法设置"}
          />
        )}
        {actions?.showInputMethodPicker && (
          <ActionButton
            action={() => invokeAction(actions.showInputMethodPicker)}
            className="secondary m-0"
            label="选择输入法"
          />
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
            {/* 页面图标画的是浅色，亮色主题下和桌面侧栏一样反转成深色。 */}
            <img
              src={item.icon}
              alt=""
              aria-hidden="true"
              className="size-[20px] shrink-0 light-theme:[filter:invert(1)_brightness(0.25)]"
            />
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
