import { type ReactNode, useState } from "react";
import { HarmonyLaunchSplash, type HostChromeClient, type ResolvedAppTheme } from "@msime/ui";

/** 仅新建的手机设置文档播放一次；切换页面或从后台恢复沿用同一个组件状态。 */
export function HarmonyLaunch({
  mobile,
  appTheme,
  chrome,
  dark,
  children,
}: {
  mobile: boolean;
  appTheme: ResolvedAppTheme | null;
  chrome?: HostChromeClient;
  dark: boolean;
  children: ReactNode;
}) {
  const [finished, setFinished] = useState(false);
  return mobile && !finished ? (
    <HarmonyLaunchSplash
      appTheme={appTheme}
      chrome={chrome}
      dark={dark}
      onDone={() => setFinished(true)}
    />
  ) : (
    children
  );
}
