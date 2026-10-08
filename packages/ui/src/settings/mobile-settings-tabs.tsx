import type { MobilePrimaryPageId } from "./mobile-navigation";
import { harmonyTabIcon, mobileTabIcon, mobileTabTitle } from "./mobile-tab-helpers";
import * as settings from "./settings-style";
import type { CSSProperties } from "react";
import { ActionButton } from "../core/action-button";
import { FluentIcon } from "../core/fluent-icons";
import type { SettingsPlatform } from "../theme/platform-tokens";

export interface MobileSettingsTab {
  id: MobilePrimaryPageId;
  title: string;
  icon: string;
}

export interface MobileSettingsTabsProps<T extends { id: string; title: string; icon: string }> {
  tabs: readonly T[];
  activeTab: T["id"];
  onSelect: (tab: T["id"]) => void;
  /** 设置外观；HarmonyOS 手机绘制设计稿中的 Fluent 字形，而不是遮罩形式的页面图标。 */
  platform?: SettingsPlatform;
}

/** Bottom navigation for the phone settings surface. */
export function MobileSettingsTabs<T extends { id: string; title: string; icon: string }>({
  tabs,
  activeTab,
  onSelect,
  platform,
}: MobileSettingsTabsProps<T>) {
  return (
    <nav className={settings.mobileTabBar} aria-label="主要功能">
      {tabs.map((item) => {
        const fluent = platform === "harmony" ? harmonyTabIcon(item.id) : undefined;
        return (
          <ActionButton
            key={item.id}
            action={() => onSelect(item.id)}
            className={settings.mobileTab(activeTab === item.id)}
            ariaCurrent={activeTab === item.id ? "page" : undefined}
            label={
              <>
                <span className={settings.mobileTabPill(activeTab === item.id)}>
                  {fluent ? (
                    // 字形以 `currentColor` 填充，所以标签被选中时它与文字一起取强调色。
                    <span className="block size-6" data-tab-icon={fluent} aria-hidden="true">
                      <FluentIcon name={fluent} size={24} className="block" />
                    </span>
                  ) : (
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
                  )}
                </span>
                {mobileTabTitle(item.id, item.title)}
              </>
            }
          />
        );
      })}
    </nav>
  );
}
