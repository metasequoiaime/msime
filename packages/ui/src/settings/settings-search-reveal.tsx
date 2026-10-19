import { useEffect, useRef, type ReactNode } from "react";

export const settingsSearchRevealEvent = "settings-search-reveal";

/** 下钻区把显隐交还自己的状态，搜索只请求展开，不直接修改 React 的 hidden。 */
export function SettingsSearchReveal({
  hidden,
  label,
  onReveal,
  children,
}: {
  hidden: boolean;
  label?: string;
  onReveal: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const node = ref.current;
    node?.addEventListener(settingsSearchRevealEvent, onReveal);
    return () => node?.removeEventListener(settingsSearchRevealEvent, onReveal);
  }, [onReveal]);
  return (
    <div
      ref={ref}
      hidden={hidden}
      role={label ? "region" : undefined}
      aria-label={label}
      data-settings-search-reveal=""
    >
      {children}
    </div>
  );
}
