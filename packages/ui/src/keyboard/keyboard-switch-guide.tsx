import { useEffect, useRef, useSyncExternalStore } from "react";
import type { ImeSetupClient } from "../core/host-contracts";
import { useImeSetupState } from "./setup-status-card";
import { useInterval } from "../core/use-interval";
import { useTimeout } from "../core/use-timeout";
import { CheckGlyph } from "../core/check-glyph";

/**
 * 「设为默认输入法」在 HarmonyOS 手机上的引导：弹出键盘，告诉用户长按键盘左下角的 🌐、在菜单里选「水杉输入法」。
 *
 * 第三方应用不能自己切换输入法（`inputMethod.switchInputMethod` 要系统权限），能用的只有系统的选择对话框 `showOptionalInputMethods`，它在 HarmonyOS 上是 OpenHarmony 自带的那个粉底绿块、没有样式的列表。长按 🌐 弹出的是系统界面自己的输入法菜单，样子和系统一致，所以鸿蒙宿主在手机上把 `showInputMethodPicker` 换成 [`openKeyboardSwitchGuide`]，各个「设为默认」入口不用改。
 *
 * 引导层由 [`KeyboardSwitchGuideHost`] 画，它挂在设置外壳和欢迎流程里，所以继承页面的配色。
 */
let open = false;
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** 打开引导层。签名与宿主的 `showInputMethodPicker` 相同，可以直接替换它。 */
export function openKeyboardSwitchGuide(): Promise<void> {
  open = true;
  emit();
  return Promise.resolve();
}

function closeKeyboardSwitchGuide() {
  open = false;
  emit();
}

/** 设为默认之后多停这么久，让用户看到「已设为默认」再关。 */
const DONE_MS = 900;
/** 引导开着时多久请宿主重读一次设置状态：从 🌐 菜单切换输入法不会触发宿主监听的事件，只能自己去问。 */
const REFRESH_MS = 1000;

const backdrop = "fixed inset-0 z-50 bg-black/30 animate-ms-fade-in motion-reduce:animate-none";
const card =
  "fixed inset-x-4 top-[max(16px,env(safe-area-inset-top))] z-50 flex flex-col gap-3 rounded-[24px] bg-[var(--p-chrome,var(--p-bg))] p-5 text-[var(--p-text)] shadow-[0_12px_40px_rgba(0,0,0,0.25)] animate-ms-fade-in motion-reduce:animate-none";
const field =
  "m-0 h-11 w-full rounded-[14px] border-0 bg-[var(--p-group-bg)] px-3.5 text-[16px] text-[var(--p-text)] [box-shadow:inset_0_0_0_1px_var(--p-hair)] [font-family:inherit] outline-none placeholder:text-[var(--p-sub)]";
const closeButton =
  "m-0 h-10 cursor-pointer self-end rounded-full border-0 bg-transparent px-4 text-[15px] font-medium text-[var(--p-accent-text,var(--accent-color))] [font-family:inherit] active:opacity-70";

/** 引导层本身。没打开时什么也不画，所以挂在不用它的宿主里也没有影响。 */
export function KeyboardSwitchGuideHost({ setup }: { setup?: ImeSetupClient }) {
  const shown = useSyncExternalStore(subscribe, () => open);
  if (!shown) return null;
  return <KeyboardSwitchGuide setup={setup} />;
}

function KeyboardSwitchGuide({ setup }: { setup?: ImeSetupClient }) {
  const state = useImeSetupState(setup);
  const done = state?.current === true;
  const input = useRef<HTMLInputElement>(null);

  // 打开就把焦点交给输入框，系统随之弹出键盘，键盘下方就是要长按的 🌐。
  useEffect(() => {
    input.current?.focus();
  }, []);

  useInterval(() => void setup?.refresh?.(), done || !setup?.refresh ? null : REFRESH_MS);

  useEffect(() => {
    if (!done) return;
    input.current?.blur();
  }, [done]);
  useTimeout(closeKeyboardSwitchGuide, done ? DONE_MS : null);

  return (
    <>
      <div className={backdrop} aria-hidden="true" onClick={closeKeyboardSwitchGuide} />
      <section className={card} role="dialog" aria-modal="true" aria-label="把水杉设为默认输入法">
        <h2 className="m-0 text-[18px] font-semibold">把水杉设为默认输入法</h2>
        {done ? (
          <p
            className="m-0 flex items-center gap-2 text-[15px] text-[var(--p-accent-text,var(--accent-color))]"
            role="status"
          >
            <CheckGlyph size={18} strokeWidth={2.4} />
            已设为默认
          </p>
        ) : (
          <>
            <p className="m-0 text-[15px] leading-[1.6] text-[var(--p-sub)]">
              长按键盘左下角的
              <GlobeGlyph />
              ，在弹出的菜单里选「水杉输入法」。
            </p>
            <input
              ref={input}
              className={field}
              aria-label="弹出键盘"
              placeholder="点这里弹出键盘"
              autoComplete="off"
            />
          </>
        )}
        <button type="button" className={closeButton} onClick={closeKeyboardSwitchGuide}>
          {done ? "完成" : "关闭"}
        </button>
      </section>
    </>
  );
}

/** 系统画在键盘左下角的地球图标，画成同样的样子用户才对得上。 */
function GlobeGlyph() {
  return (
    <svg
      className="mx-1 inline-block size-[18px] align-[-3px] text-[var(--p-text)]"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      role="img"
      aria-label="地球图标"
    >
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9M12 3c-2.5 2.6-3.8 5.6-3.8 9s1.3 6.4 3.8 9" />
    </svg>
  );
}
