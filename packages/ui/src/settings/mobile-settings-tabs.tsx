import type { MobilePrimaryPageId } from "./mobile-navigation";
import { mobileTabIcon, mobileTabTitle } from "./mobile-tab-helpers";
import * as settings from "./settings-style";
import type { CSSProperties } from "react";
import { ActionButton } from "../core/action-button";

export interface MobileSettingsTab {
  id: MobilePrimaryPageId;
  title: string;
  icon: string;
}

export interface MobileSettingsTabsProps<T extends { id: string; title: string; icon: string }> {
  tabs: readonly T[];
  activeTab: T["id"];
  onSelect: (tab: T["id"]) => void;
}

/** Bottom navigation for the phone settings surface. */
export function MobileSettingsTabs<T extends { id: string; title: string; icon: string }>({
  tabs,
  activeTab,
  onSelect,
}: MobileSettingsTabsProps<T>) {
  return (
    <nav className={settings.mobileTabBar} aria-label="主要功能">
      {tabs.map((item) => (
        <ActionButton
          key={item.id}
          action={() => onSelect(item.id)}
          className={settings.mobileTab(activeTab === item.id)}
          ariaCurrent={activeTab === item.id ? "page" : undefined}
          label={
            <>
              <span className={settings.mobileTabPill(activeTab === item.id)}>
                <span
                  className={settings.mobileTabIcon}
                  style={
                    {
                      "--tab-icon": `url("${mobileTabIcon(item.id, item.icon)}")`,
                    } as CSSProperties
                  }
                  data-tab-icon={mobileTabIcon(item.id, item.icon)}
                  aria-hidden="true"
                />
              </span>
              {mobileTabTitle(item.id, item.title)}
            </>
          }
        />
      ))}
    </nav>
  );
}
