import type { ReactNode } from "react";

import { ToastProvider } from "../core/toast";

/** 设计稿的 toast 只属于 HarmonyOS 外观；其他宿主不额外挂常驻的提示区域。 */
export function SettingsShellToasts({
  enabled,
  children,
}: {
  enabled: boolean;
  children: ReactNode;
}) {
  return enabled ? <ToastProvider>{children}</ToastProvider> : children;
}
