import { useSyncExternalStore } from "react";

function attributeStore(attribute: string) {
  return {
    subscribe(notify: () => void) {
      const observer = new MutationObserver(notify);
      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: [attribute],
      });
      return () => observer.disconnect();
    },
    snapshot: () => document.documentElement.getAttribute(attribute) === "true",
  };
}

const glass = attributeStore("data-macos-liquid-glass");
const controls = attributeStore("data-macos-system-controls");

/** 只读取原生宿主确认的能力，辅助功能切换时同步恢复回退控件。 */
export function useMacOSLiquidGlass(): boolean {
  return useSyncExternalStore(glass.subscribe, glass.snapshot, () => false);
}

/** 系统控件独立于玻璃启用；减少透明度时继续交给 WebKit 处理辅助功能。 */
export function useMacOSSystemControls(): boolean {
  return useSyncExternalStore(controls.subscribe, controls.snapshot, () => false);
}
