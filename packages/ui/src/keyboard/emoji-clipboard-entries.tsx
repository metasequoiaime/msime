import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { clipboardTooltip } from "./emoji-panel-helpers";
import {
  clipboardContextMenu,
  clipboardContextMenuItem,
  clipboardDeleteButton,
  clipboardPreviewBubble,
  clipboardRow,
  clipboardRowFrame,
  clipboardSideButton,
} from "./emoji-panel-style";

export interface EmojiClipboardEntriesProps {
  items: readonly string[];
  busy: boolean;
  canCopy: boolean;
  onCopy: (text: string) => void;
  onItemKeyDown: (event: KeyboardEvent<HTMLButtonElement>, text: string) => void;
  onPaste?: (text: string) => void;
  /** 「发到云剪贴板」，经行的右键菜单（键盘上是菜单键或 Shift+F10）触发，与 macOS 原生剪贴板行的右键菜单一致。 */
  onSendToCloud?: (text: string) => void;
  onRemove?: (text: string) => void;
}

/** 悬停或键盘聚焦停留这么久才显示长文本预览，划过列表时不闪出一串气泡。 */
const PREVIEW_DELAY_MS = 350;

type Preview = { text: string; left: number; top: number; width: number; above: boolean };
type Menu = { text: string; left: number; top: number; keyboard: boolean };

/** 预览气泡的位置：优先放在行的上方，放不下才放到下方，与 macOS 原生剪贴板的悬停预览一致。 */
function previewFor(row: HTMLElement, text: string): Preview {
  const rect = row.getBoundingClientRect();
  const viewport = row.ownerDocument.defaultView?.innerHeight ?? 0;
  // 12 行 13px 文字加上下内边距约 230px；行上方的空间不够时翻到下方。
  const above = rect.top >= Math.min(230, viewport / 2);
  return {
    text: clipboardTooltip(text),
    left: rect.left,
    top: above ? rect.top - 8 : rect.bottom + 8,
    width: rect.width,
    above,
  };
}

/** 剪贴板记录的受控视图，导航标记和原始键盘事件交给面板状态处理。 */
export function EmojiClipboardEntries({
  items,
  busy,
  canCopy,
  onCopy,
  onItemKeyDown,
  onPaste,
  onSendToCloud,
  onRemove,
}: EmojiClipboardEntriesProps) {
  const [preview, setPreview] = useState<Preview | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const pointerFocus = useRef(false);
  const menuItem = useRef<HTMLButtonElement>(null);

  useEffect(() => () => clearTimeout(timer.current), []);
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const view = menuItem.current?.ownerDocument.defaultView;
    view?.addEventListener("pointerdown", close);
    view?.addEventListener("blur", close);
    view?.addEventListener("scroll", close, true);
    return () => {
      view?.removeEventListener("pointerdown", close);
      view?.removeEventListener("blur", close);
      view?.removeEventListener("scroll", close, true);
    };
  }, [menu]);
  useLayoutEffect(() => {
    if (menu?.keyboard) menuItem.current?.focus({ preventScroll: true });
  }, [menu]);
  // 列表变化（删除、刷新、搜索）后旧的预览和菜单指向的行可能已经不在了。
  useEffect(() => {
    clearTimeout(timer.current);
    setPreview(null);
    setMenu(null);
  }, [items]);

  function schedulePreview(row: HTMLElement, text: string) {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      if (row.isConnected) setPreview(previewFor(row, text));
    }, PREVIEW_DELAY_MS);
  }
  function hidePreview() {
    clearTimeout(timer.current);
    setPreview(null);
  }
  function openMenu(event: MouseEvent<HTMLButtonElement>, text: string) {
    if (!onSendToCloud) return;
    event.preventDefault();
    hidePreview();
    const rect = event.currentTarget.getBoundingClientRect();
    // 键盘（菜单键、Shift+F10）触发的 contextmenu 没有指针坐标，菜单挂在行的左下角。
    const keyboard = event.clientX === 0 && event.clientY === 0;
    setMenu({
      text,
      left: keyboard ? rect.left + 12 : event.clientX,
      top: keyboard ? rect.bottom : event.clientY,
      keyboard,
    });
  }
  function closeMenu(restoreFocus: boolean) {
    const text = menu?.text;
    setMenu(null);
    if (!restoreFocus || text === undefined) return;
    const rows =
      menuItem.current?.ownerDocument.querySelectorAll<HTMLButtonElement>("[data-clipboard-item]");
    Array.from(rows ?? [])
      .find((row) => row.dataset.clipboardText === text)
      ?.focus({ preventScroll: true });
  }

  return (
    <div className="flex flex-col gap-[7px]">
      {items.map((item) => (
        <div className="flex items-stretch gap-1.5" data-clipboard-row="" key={item}>
          <div className={clipboardRowFrame}>
            <button
              type="button"
              className={clipboardRow(Boolean(onRemove))}
              data-clipboard-item=""
              data-clipboard-text={item}
              data-emoji-navigation-item
              aria-keyshortcuts={
                [canCopy ? "Control+c" : "", onRemove ? "Delete" : ""].filter(Boolean).join(" ") ||
                undefined
              }
              aria-haspopup={onSendToCloud ? "menu" : undefined}
              onKeyDown={(event) => {
                hidePreview();
                onItemKeyDown(event, item);
              }}
              onPointerEnter={(event) => {
                if (event.pointerType === "mouse") schedulePreview(event.currentTarget, item);
              }}
              onPointerLeave={hidePreview}
              onPointerDown={() => {
                pointerFocus.current = true;
                hidePreview();
              }}
              onFocus={(event) => {
                // 只有键盘移来的焦点才显示预览；点击和触屏聚焦不弹气泡。
                if (!pointerFocus.current) schedulePreview(event.currentTarget, item);
                pointerFocus.current = false;
              }}
              onBlur={hidePreview}
              onContextMenu={(event) => openMenu(event, item)}
              disabled={busy}
              onClick={() => void onCopy(item)}
            >
              {item.replace(/[\r\n\t]/g, " ")}
            </button>
            {onRemove && (
              <button
                type="button"
                className={clipboardDeleteButton}
                disabled={busy}
                aria-label="删除此条历史记录"
                title="删除此条历史记录，不会清空系统剪贴板"
                onClick={() => void onRemove(item)}
              >
                <span aria-hidden="true">×</span>
              </button>
            )}
          </div>
          {onPaste && (
            <button
              type="button"
              className={clipboardSideButton}
              disabled={busy}
              aria-label="粘贴此条记录到原应用"
              onClick={() => void onPaste(item)}
            >
              粘贴
            </button>
          )}
        </div>
      ))}
      {preview && (
        <div
          className={clipboardPreviewBubble}
          aria-hidden="true"
          data-clipboard-preview=""
          style={{
            left: preview.left,
            top: preview.top,
            maxWidth: preview.width,
            transform: preview.above ? "translateY(-100%)" : undefined,
          }}
        >
          {preview.text}
        </div>
      )}
      {menu && onSendToCloud && (
        <div
          className={clipboardContextMenu}
          role="menu"
          aria-label="剪贴板记录"
          style={{ left: menu.left, top: menu.top }}
          onPointerDown={(event) => event.stopPropagation()}
          onKeyDown={(event) => {
            if (event.key !== "Escape" && event.key !== "Tab") return;
            event.preventDefault();
            event.stopPropagation();
            closeMenu(true);
          }}
        >
          <button
            ref={menuItem}
            type="button"
            role="menuitem"
            className={clipboardContextMenuItem}
            disabled={busy}
            onClick={() => {
              const text = menu.text;
              closeMenu(true);
              void onSendToCloud(text);
            }}
          >
            发到云剪贴板
          </button>
        </div>
      )}
    </div>
  );
}
