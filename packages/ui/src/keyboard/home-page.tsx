import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import { globalThemeTitle, keyboardThemeId, themeEntry } from "../theme/global-theme";
import { useCandidatePreviewTheme } from "../candidate/candidate-preview-theme";
import { ActionButton } from "../core/action-button";
import { NavGroup } from "../core/platform-controls";
import { SearchGlyph } from "../core/search-glyph";
import { openHomeKeyboard, type HomePageActions } from "./home-model";
import { useHomeSearch } from "./use-home-search";
import type { Preferences } from "../core/preferences-model";
import type { SurfaceTheme } from "../settings/theme-preferences";
import { touchKeyboardSchemeTitle } from "../settings/touch-keyboard-scheme-helpers";
import { SetupStatusCard } from "./setup-status-card";
import { logo } from "../settings/app-resources";
import { RootSettingsList, RootSettingsRowView } from "./root-settings-list";
import type { RootPage } from "./root-settings-model";

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

const quickTile =
  "press-spring flex min-w-0 flex-col items-start gap-[5px] rounded-[11px] border border-edge bg-subtle p-3 text-left text-body hover:border-edge-strong hover:bg-raised active:scale-[0.96] active:opacity-[0.88] motion-reduce:transition-none max-tight:px-2 max-tight:py-2.5";
const quickIcon =
  "grid size-[34px] place-items-center rounded-[11px] text-[17px] font-[650] leading-none";
const quickTitle = "text-[13px] font-semibold";
const quickNote =
  "m-0 w-full overflow-hidden text-ellipsis whitespace-nowrap max-tight:text-[11px]";

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

export type { HomePageActions } from "./home-model";

export function HomePage({
  preferences,
  actions,
  onOpenPage,
  onOpenChat,
  touchLayout = false,
  ios = false,
  rootPages,
  keyboardSurfaceTheme,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  onOpenChat?: () => void;
  touchLayout?: boolean;
  /** 桌面首页键盘预览的单独明暗，取宿主真正会用的那个值：各界面跟随颜色模式的宿主不读存下的 `screen_keyboard_theme`，调用方就不传，预览照颜色模式画。触屏首页忽略它。 */
  keyboardSurfaceTheme?: SurfaceTheme;
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
        onOpenChat={onOpenChat}
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
      keyboardSurfaceTheme={keyboardSurfaceTheme}
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
 * 「试用键盘」在宿主有对应窗口时打开宿主的键盘，否则打开 AI 对话二级页并弹出键盘，与 Android 和 iOS 一样：试键盘就是在那里打字发给 AI。没有对话的宿主打开子页面「试用键盘」；宿主打开窗口失败时也按同样的顺序回退。
 */
function TouchHomePage({
  preferences,
  actions,
  onOpenPage,
  onOpenChat,
  rootPages,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  onOpenChat?: () => void;
  rootPages?: readonly RootPage[];
}) {
  const { query, setQuery, groups, needle, matches, searchKeyDown } = useHomeSearch(
    preferences,
    rootPages,
  );
  const tryKeyboard = () =>
    openHomeKeyboard(actions?.openKeyboard, onOpenChat ?? (() => onOpenPage("try-keyboard")));

  return (
    <section className="flex flex-col gap-5" aria-label="首页">
      <label className={searchPill}>
        <SearchGlyph size={16} className="shrink-0" />
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
          <RootSettingsList groups={groups} onOpenPage={onOpenPage} />
        </>
      )}
    </section>
  );
}

function DesktopHomePage({
  preferences,
  actions,
  onOpenPage,
  onOpenChat,
  ios,
  keyboardSurfaceTheme,
}: {
  preferences: Preferences;
  actions?: HomePageActions;
  onOpenPage: (page: string) => void;
  onOpenChat?: () => void;
  ios: boolean;
  keyboardSurfaceTheme?: SurfaceTheme;
}) {
  const theme = useCandidatePreviewTheme(preferences.theme, keyboardSurfaceTheme);
  const selected = themeEntry(preferences.global_theme).id;
  const skin = keyboardThemeId(selected, preferences.custom_theme);
  const customDesign = preferences.custom_theme?.keyboard ?? undefined;
  const skinTitle = globalThemeTitle(preferences.global_theme);
  const invokeAction = (action?: () => Promise<void>) => {
    if (action) void action();
  };
  const openKeyboard = () =>
    openHomeKeyboard(
      actions?.openKeyboard?.bind(actions),
      onOpenChat ?? (() => onOpenPage("screen-keyboard")),
    );

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
        <img className="size-12 opacity-80" src={logo} alt="" />
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
