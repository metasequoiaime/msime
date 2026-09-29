import type { SettingsPageId } from "./mobile-navigation";
import { MobileSettingsTabs, type MobileSettingsTabsProps } from "./mobile-settings-tabs";
import { SettingsSidebar, type SettingsSidebarProps } from "./settings-sidebar";

export interface SettingsNavigationChromeProps {
  mobile: boolean;
  tabs: MobileSettingsTabsProps<{ id: SettingsPageId; title: string; icon: string }>["tabs"];
  activeTab: SettingsPageId;
  onSelectTab: (tab: SettingsPageId) => void;
  groups: SettingsSidebarProps["groups"];
  selectedPage: SettingsPageId;
  onSelectPage: SettingsSidebarProps["onSelectPage"];
}

/** Shared responsive navigation chrome for desktop and mobile settings. */
export function SettingsNavigationChrome({
  mobile,
  tabs,
  activeTab,
  onSelectTab,
  groups,
  selectedPage,
  onSelectPage,
}: SettingsNavigationChromeProps) {
  return (
    <>
      {mobile && <MobileSettingsTabs tabs={tabs} activeTab={activeTab} onSelect={onSelectTab} />}
      <SettingsSidebar groups={groups} selectedPage={selectedPage} onSelectPage={onSelectPage} />
    </>
  );
}
