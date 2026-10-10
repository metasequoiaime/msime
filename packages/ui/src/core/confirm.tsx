import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import * as style from "../community/community-style";

/**
 * An in-page replacement for `window.confirm`.
 *
 * `window.confirm` is not a dialog this application can rely on. On the Apple hosts it does not
 * appear at all: they render through wry's WKWebView, whose `WKUIDelegate` implements no
 * JavaScript dialog methods, and WKWebView has no built-in ones - so `confirm()` returns `false`
 * immediately and the guarded action silently never runs. A button that does nothing when pressed
 * is worse than a missing button, because it claims the feature exists. Where it does appear it is
 * a host modal: it does not follow the page theme, its buttons carry the host's own labels (wry's
 * Android client hardcodes English `OK`/`Cancel`), and while it is up the page's own keyboard and
 * focus handling is suspended.
 *
 * This renders in the React tree instead, so it behaves the same on every host.
 */
export type ConfirmRequest = {
  message: string;
  title?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Styles the confirming button as destructive. Deleting something should look like it. */
  danger?: boolean;
};

type Pending = ConfirmRequest & { resolve: (confirmed: boolean) => void };

/**
 * Returns an `await`-able `confirm` and the dialog to render.
 *
 * The caller renders `{confirmation}` somewhere in its tree; no provider or portal is involved, so
 * a page opts in by using the hook rather than by being wrapped in something.
 */
export function useConfirm(): {
  confirm: (request: string | ConfirmRequest) => Promise<boolean>;
  confirmation: ReactNode;
} {
  const [pending, setPending] = useState<Pending>();
  // The dialog steals focus, so remember where it came from and give it back; otherwise every
  // confirmation drops the user at the top of the page.
  const restoreFocus = useRef<HTMLElement | null>(null);
  const confirmButton = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const messageId = useId();
  // Resolving twice would be harmless for a promise but hides a double-close bug, and an unmount
  // while open must resolve rather than leave the caller waiting forever.
  const open = useRef<Pending>(undefined);

  const settle = useCallback((confirmed: boolean) => {
    const current = open.current;
    open.current = undefined;
    setPending(undefined);
    if (!current) return;
    current.resolve(confirmed);
    restoreFocus.current?.focus();
    restoreFocus.current = null;
  }, []);

  useEffect(
    () => () => {
      // Unmounting is not an answer, but leaving the caller awaiting forever is worse: it would
      // hold whatever state the action was about in limbo with nothing left to release it.
      open.current?.resolve(false);
      open.current = undefined;
    },
    [],
  );

  const confirm = useCallback(
    (request: string | ConfirmRequest) =>
      new Promise<boolean>((resolve) => {
        // A second request while one is open answers the new one negatively rather than replacing
        // the dialog underneath the user, who is mid-decision about the first.
        if (open.current) {
          resolve(false);
          return;
        }
        const active = document.activeElement;
        restoreFocus.current = active instanceof HTMLElement ? active : null;
        const next: Pending = {
          ...(typeof request === "string" ? { message: request } : request),
          resolve,
        };
        open.current = next;
        setPending(next);
      }),
    [],
  );

  useEffect(() => {
    if (!pending) return;
    // A destructive request starts on the harmless answer, so a stray Enter or Space (a repeating
    // key, or the one that opened the dialog) cannot delete anything.
    (pending.danger ? cancelButton : confirmButton).current?.focus();
  }, [pending]);

  const confirmation = pending ? (
    <div
      className={style.backdrop}
      // A click on the backdrop is a cancel, the same as Escape. `currentTarget` so a click that
      // started inside the dialog and ended on the backdrop does not count.
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) settle(false);
      }}
    >
      <div
        className={`${style.dialog} w-[min(420px,100%)]`}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={messageId}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.stopPropagation();
            settle(false);
          }
        }}
      >
        <h2 id={titleId} className={style.dialogTitle}>
          {pending.title ?? "确认操作"}
        </h2>
        <p id={messageId} className="m-0 leading-relaxed text-secondary">
          {pending.message}
        </p>
        <div className={style.dialogActions}>
          <button
            type="button"
            ref={cancelButton}
            className="secondary"
            onClick={() => settle(false)}
          >
            {pending.cancelLabel ?? "取消"}
          </button>
          <button
            type="button"
            ref={confirmButton}
            // The answer the dialog asks for is its main action: without a class it fell back to the web view's own grey button on the phone hosts, beside a styled 取消.
            className={pending.danger ? style.destructive : "primary"}
            onClick={() => settle(true)}
          >
            {pending.confirmLabel ?? "确定"}
          </button>
        </div>
      </div>
    </div>
  ) : null;

  return { confirm, confirmation };
}
