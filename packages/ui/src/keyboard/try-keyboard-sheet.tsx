import { useEffect, useId, useState, type KeyboardEvent } from "react";
import { errorMessage } from "../core/error-message";
import { useToast } from "../core/toast";
import { makeDefaultAction, useImeSetupState, type SetupActions } from "./setup-status-card";

const scrim =
  "fixed inset-0 z-50 animate-ms-fade-in bg-[rgba(0,0,0,0.35)] motion-reduce:animate-none";
const sheet =
  "fixed inset-x-0 bottom-0 z-50 flex max-h-[85%] flex-col gap-3 rounded-t-[20px] bg-[var(--p-bg)] px-4 pt-2 pb-[calc(16px+env(safe-area-inset-bottom))] [color:var(--p-text)] [font-family:var(--p-font,inherit)] animate-ms-sheet-up motion-reduce:animate-none";
const textButton =
  "m-0 cursor-pointer border-0 bg-transparent p-0 [font-family:inherit] [color:var(--p-accent-text,var(--accent-color))] active:opacity-70";

/**
 * 「设置」根页面的键盘试用，供自身没有键盘窗口可打开的宿主使用：一个底部弹窗，里面只有一个文本框，弹窗打开时它获得焦点，于是系统弹出当前的输入法，用户直接在里面打字。「完成」、遮罩和 Escape 都会关闭弹窗，焦点回到打开它的按钮上。
 *
 * 本输入法还不是当前输入法时，弹出的是别家的键盘，所以弹窗会说明这一点，并提供解决它的那一步操作。
 */
export function TryKeyboardSheet({
  actions,
  onClose,
}: {
  actions?: SetupActions;
  onClose: () => void;
}) {
  const state = useImeSetupState(actions?.setup);
  const toast = useToast();
  const titleId = useId();
  // 在渲染时取得，早于文本框的 autofocus 把焦点移进弹窗。
  const [opener] = useState(() =>
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  );

  useEffect(
    () => () => {
      if (opener?.isConnected) opener.focus();
    },
    [opener],
  );

  const notCurrent = state?.enabled === false || state?.current === false;
  const step = state?.enabled === false ? "去开启" : "设为默认";
  const stepAction = makeDefaultAction(actions, state);

  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    onClose();
  };

  return (
    <div className="contents">
      <div className={scrim} aria-hidden="true" onClick={onClose} />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={sheet}
        onKeyDown={keyDown}
      >
        <div className="relative flex h-11 shrink-0 items-center justify-center">
          <span id={titleId} className="text-[17px] font-semibold">
            试用键盘
          </span>
          <button
            type="button"
            className={`${textButton} absolute top-0 right-0 bottom-0 text-[16px] font-semibold`}
            onClick={onClose}
          >
            完成
          </button>
        </div>
        <textarea
          // 这个弹窗的意义在于键盘，而聚焦输入框正是让键盘弹出的方式。
          autoFocus
          aria-label="试用键盘输入框"
          placeholder="在这里打字试试"
          className="min-h-[120px] w-full resize-none rounded-[12px] border-0 bg-[var(--p-group-bg)] px-3.5 py-3 text-[16px] leading-[1.5] [color:var(--p-text)] [font-family:inherit] outline-none placeholder:[color:var(--p-sub)]"
        />
        {notCurrent && (
          <p className="m-0 text-[13px] [color:var(--p-sub)]">
            先完成上面的设置，键盘才会是水杉
            {stepAction && (
              <>
                {" "}
                <button
                  type="button"
                  className={`${textButton} text-[13px]`}
                  onClick={() =>
                    void stepAction().catch((error: unknown) => toast(errorMessage(error)))
                  }
                >
                  {step}
                </button>
              </>
            )}
          </p>
        )}
      </div>
    </div>
  );
}
