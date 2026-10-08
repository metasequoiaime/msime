import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

/** toast 停留的时长，即设计稿的 1.6 秒（Android `Ui.TOAST_MILLIS`）。 */
export const toastMillis = 1600;

type ShowToast = (text: string) => void;

const ToastContext = createContext<ShowToast | null>(null);

function ignore() {}

/**
 * 设计稿的 toast：底部标签栏上方的一个深色胶囊（浅色模式 `#1C1C1E` 配白字，深色模式 `#F2F2F2` 配 `#111`），淡入后 1.6 秒消失。新 toast 会替换正在显示的那个并重新计时，和 Android 的 `MsToast` 一样。屏幕阅读器通过一个始终挂载的 polite live region 读出它，所以即使消息到达时区域是空的也会被播报。
 */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<{ text: string; key: number } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const counter = useRef(0);

  const show = useCallback<ShowToast>((text) => {
    clearTimeout(timer.current);
    counter.current += 1;
    // 换一个新 key 会重新挂载胶囊，所以替换上来的 toast 会重新淡入，而不是原地换掉文字。
    setToast({ text, key: counter.current });
    timer.current = setTimeout(() => setToast(null), toastMillis);
  }, []);

  useEffect(() => () => clearTimeout(timer.current), []);

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div
        role="status"
        aria-live="polite"
        className="pointer-events-none fixed inset-x-0 bottom-[calc(110px+env(safe-area-inset-bottom))] z-[60] flex justify-center"
      >
        {toast && (
          <span
            key={toast.key}
            className="max-w-[80%] animate-ms-fade-in rounded-full bg-[#F2F2F2] px-[18px] py-2.5 text-center text-[14px] text-[#111] shadow-[0_6px_20px_rgba(0,0,0,0.18)] [font-family:var(--p-font,inherit)] [overflow-wrap:anywhere] motion-reduce:animate-none light-theme:bg-[#1C1C1E] light-theme:text-white"
          >
            {toast.text}
          </span>
        )}
      </div>
    </ToastContext.Provider>
  );
}

/** 显示一条 toast；在 `ToastProvider` 之外什么也不做，所以页面可以无条件调用，没有 provider 的宿主只是不显示。 */
export function useToast(): ShowToast {
  return useContext(ToastContext) ?? ignore;
}
