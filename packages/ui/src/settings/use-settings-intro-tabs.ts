import { useCallback, useState } from "react";

export interface SettingsIntroTabs {
  voice: "input" | "recording";
  "screen-keyboard": "keyboard" | "handwriting";
  tools: "clipboard" | "history";
}

/** 页首与内容共用标签选择；仅保留在当前设置窗口内，不写偏好。 */
export function useSettingsIntroTabs() {
  const [introTabs, setIntroTabs] = useState<SettingsIntroTabs>({
    voice: "input",
    "screen-keyboard": "keyboard",
    tools: "clipboard",
  });
  const selectIntroTab = useCallback(
    <Page extends keyof SettingsIntroTabs>(page: Page, tab: SettingsIntroTabs[Page]) => {
      setIntroTabs((current) => ({ ...current, [page]: tab }));
    },
    [],
  );
  return { introTabs, selectIntroTab };
}
