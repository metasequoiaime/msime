import { useSyncExternalStore } from "react";

const attribute = "data-macos-liquid-glass";

function subscribe(notify: () => void) {
  const observer = new MutationObserver(notify);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: [attribute] });
  return () => observer.disconnect();
}

/** 只读取原生宿主确认的能力，辅助功能切换时同步恢复回退控件。 */
export function useMacOSLiquidGlass(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => document.documentElement.getAttribute(attribute) === "true",
    () => false,
  );
}
