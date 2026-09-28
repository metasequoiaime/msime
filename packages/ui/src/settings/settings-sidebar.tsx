import { logo } from "./app-resources";
import type { SettingsPageId } from "./mobile-navigation";
import * as settings from "./settings-style";

export interface SettingsSidebarItem {
  id: SettingsPageId;
  title: string;
  icon: string;
}

export interface SettingsSidebarProps {
  groups: readonly (readonly SettingsSidebarItem[])[];
  selectedPage: SettingsPageId;
  onSelectPage: (page: SettingsPageId) => void;
}

/** Desktop settings navigation with the grouped page list and product mark. */
export function SettingsSidebar({ groups, selectedPage, onSelectPage }: SettingsSidebarProps) {
  return (
    <nav className={settings.sidebar} aria-label="设置分类">
      <div className={settings.sidebarHeader}>
        <img src={logo} alt="" />
        <span>水杉 IME</span>
      </div>
      {groups.map((group, index) => (
        <div
          key={group[0].id}
          className={index > 0 ? `${settings.sidebarSection} mt-3.5` : settings.sidebarSection}
          data-sidebar-section=""
        >
          {group.map((item) => (
            <button
              key={item.id}
              type="button"
              className={settings.sidebarItem(selectedPage === item.id)}
              aria-current={selectedPage === item.id ? "page" : undefined}
              aria-controls="settings-content"
              onClick={() => onSelectPage(item.id)}
            >
              <span className={settings.sidebarIcon}>
                <img src={item.icon} alt="" />
              </span>
              {item.title}
            </button>
          ))}
        </div>
      ))}
      <p className={settings.previewLabel}>客户端预览版</p>
    </nav>
  );
}
