import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { useDebouncedCallback } from "../core/use-debounced-callback";
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

/** 没有悬停的设备（触屏手机、平板）：× 要悬停才出现，而一直显示的 28px × 又小于触屏的点按尺寸、紧挨着复制整行，误触就删掉一条，所以这些设备上仍用行旁带文字的「删除」按钮。共享面板也经 Android 设置应用的表情与剪贴板页显示。 */
function noHoverDevice(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(hover: none)").matches
  );
}

/** 悬停或键盘聚焦停留这么久才显示长文本预览，划过列表时不闪出一串气泡。 */
const PREVIEW_DELAY_MS = 350;

type Preview = {
  text: string;
  left: number;
  width: number;
  rowTop: number;
  rowBottom: number;
  above: boolean;
};

/** 气泡与行之间的间距。 */
const PREVIEW_GAP = 8;
type Menu = { text: string; left: number; top: number; keyboard: boolean };

/** 预览气泡先按放在行上方渲染，量出实际高度后再定上下（见组件里的 layout effect）。 */
function previewFor(row: HTMLElement, text: string): Preview {
  const rect = row.getBoundingClientRect();
  return {
    text: clipboardTooltip(text),
    left: rect.left,
    width: rect.width,
    rowTop: rect.top,
    rowBottom: rect.bottom,
    above: true,
  };
}

/** 预览气泡的位置：行上方放得下就放在上方，放不下才放到下方，与 macOS 原生剪贴板的悬停预览一致。按气泡量出的高度判断，一两行的短预览在窗口顶部附近也放在上方，不去盖住下面的行。 */
function previewFitsAbove(rowTop: number, bubbleHeight: number) {
  return rowTop - PREVIEW_GAP >= bubbleHeight;
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
  const [sideDelete] = useState(noHoverDevice);
  const inlineDelete = onRemove && !sideDelete;
  const { schedule: schedulePreview, cancel: cancelPreview } = useDebouncedCallback(
    (row: HTMLElement, text: string) => {
      if (row.isConnected) setPreview(previewFor(row, text));
    },
    PREVIEW_DELAY_MS,
  );
  const pointerFocus = useRef(false);
  // 行上刚按下菜单键或 Shift+F10。浏览器随后派发的 contextmenu 带的是这一行的坐标（WebKit、Chromium 都取聚焦元素的左下角），不能凭坐标判断它来自键盘。
  const keyboardMenu = useRef(false);
  const menuItem = useRef<HTMLButtonElement>(null);
  const bubble = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    // 用鼠标打开时焦点还在行上，Esc 先在捕获阶段关掉菜单，不让它落到面板的 Esc（返回首页或关窗）上。
    const escape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      closeMenu(true);
    };
    const view = menuItem.current?.ownerDocument.defaultView;
    view?.addEventListener("pointerdown", close);
    view?.addEventListener("blur", close);
    view?.addEventListener("scroll", close, true);
    view?.addEventListener("keydown", escape, true);
    return () => {
      view?.removeEventListener("pointerdown", close);
      view?.removeEventListener("blur", close);
      view?.removeEventListener("scroll", close, true);
      view?.removeEventListener("keydown", escape, true);
    };
  }, [menu]);
  useLayoutEffect(() => {
    if (menu?.keyboard) menuItem.current?.focus({ preventScroll: true });
  }, [menu]);
  // 在绘制之前量出气泡的高度定上下，不会先在上方闪一下再跳到下方。
  useLayoutEffect(() => {
    const height = bubble.current?.getBoundingClientRect().height;
    if (!preview || height === undefined) return;
    const above = previewFitsAbove(preview.rowTop, height);
    if (above !== preview.above) setPreview({ ...preview, above });
  }, [preview]);
  // 列表内容变化（删除、刷新、搜索）后旧的预览和菜单指向的行可能已经不在了。按内容比较：面板每次轮询都会给出一个新的数组，内容没变时不能把正在显示的预览和菜单收掉。
  const itemsKey = items.join("\u0000");
  useEffect(() => {
    cancelPreview();
    setPreview(null);
    setMenu(null);
  }, [itemsKey, cancelPreview]);

  function hidePreview() {
    cancelPreview();
    setPreview(null);
  }
  function openMenu(event: MouseEvent<HTMLButtonElement>, text: string) {
    if (!onSendToCloud) return;
    event.preventDefault();
    hidePreview();
    const rect = event.currentTarget.getBoundingClientRect();
    // 键盘打开的菜单挂在行的左下角并把焦点移进去；鼠标打开的放在指针处，焦点留在行上。
    const keyboard = keyboardMenu.current;
    keyboardMenu.current = false;
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
              className={clipboardRow(Boolean(inlineDelete))}
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
                keyboardMenu.current =
                  event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
                onItemKeyDown(event, item);
              }}
              onPointerEnter={(event) => {
                if (event.pointerType === "mouse") schedulePreview(event.currentTarget, item);
              }}
              onPointerLeave={hidePreview}
              onPointerDown={() => {
                pointerFocus.current = true;
                keyboardMenu.current = false;
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
            {inlineDelete && (
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
          {onRemove && sideDelete && (
            <button
              type="button"
              className={clipboardSideButton}
              disabled={busy}
              aria-label="删除此条历史记录"
              onClick={() => void onRemove(item)}
            >
              删除
            </button>
          )}
        </div>
      ))}
      {preview && (
        <div
          ref={bubble}
          className={clipboardPreviewBubble}
          aria-hidden="true"
          data-clipboard-preview=""
          style={{
            left: preview.left,
            top: preview.above ? preview.rowTop - PREVIEW_GAP : preview.rowBottom + PREVIEW_GAP,
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
            // Esc 由上面捕获阶段的监听处理；Tab 离开菜单时同样关掉它并回到那一行。
            if (event.key !== "Tab") return;
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
