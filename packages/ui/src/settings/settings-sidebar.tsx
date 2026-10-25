import type { ReactNode } from "react";
import { NavItem } from "../core/platform-controls";
import { logo } from "./settings-options";
import { mobilePageTitle } from "./mobile-tab-helpers";
import * as settings from "./settings-style";
import type { SettingsPageGroup } from "./settings-page-projections";
import type { SettingsPageId } from "./settings-page-registry";
import type { settingsShellPlatformPresentation } from "./settings-shell-platform-presentation";
import type { useSettingsWindowInteractions } from "./use-settings-window-interactions";

export interface SettingsSidebarProps {
  presentation: Pick<
    ReturnType<typeof settingsShellPlatformPresentation>,
    "macShell" | "ipadShell" | "ipadSidebarShown" | "sidebarBrandShown" | "searchInSidebar"
  >;
  preview: boolean;
  shownSidebarGroups: readonly SettingsPageGroup[];
  navigationPage: SettingsPageId;
  navSearchField: ReactNode;
  navSearchResults?: ReactNode;
  windowDragHandlers: ReturnType<typeof useSettingsWindowInteractions>["windowDragHandlers"];
  selectPage: (page: SettingsPageId) => void;
}

/** 设置侧栏只绘制现有导航与平台投影，状态和窗口手势由外壳提供。 */
export function SettingsSidebar({
  presentation,
  preview,
  shownSidebarGroups,
  navigationPage,
  navSearchField,
  navSearchResults,
  windowDragHandlers,
  selectPage,
}: SettingsSidebarProps) {
  const { macShell, ipadShell, ipadSidebarShown, sidebarBrandShown, searchInSidebar } =
    presentation;
  return (
    <nav
      className={`${settings.sidebar} ${ipadSidebarShown ? "" : "ipad:hidden"}`}
      aria-label="设置分类"
    >
      {macShell && (
        <div className={settings.macosDragRow} data-window-drag="" {...windowDragHandlers} />
      )}
      {ipadShell && <h2 className={settings.sidebarTitle}>{mobilePageTitle("home", "")}</h2>}
      {/* 品牌（图标加名称）放在侧栏顶部、搜索框之上，只在没有自绘标题栏的桌面窗口出现：macOS 排在红绿灯那一行下面，并和那一行一样可以拖动窗口；HarmonyOS 2in1 与没有窗口命令的桌面窗口直接起头。Windows 和 Linux 的标题栏已经写着品牌，手机和 iPad 的界面由系统和标签栏承担，都不再画一份。图标是装饰，名称就是文字本身。 */}
      {sidebarBrandShown && (
        <div
          className={settings.sidebarHeader}
          data-sidebar-brand=""
          data-window-drag={macShell ? "" : undefined}
          {...(macShell ? windowDragHandlers : {})}
        >
          <img src={logo} alt="" draggable={false} />
          <span>水杉输入法</span>
        </div>
      )}
      {searchInSidebar && <label className={settings.sidebarSearch}>{navSearchField}</label>}
      <div className={settings.sidebarItems}>
        {navSearchResults ??
          shownSidebarGroups.flatMap((group) =>
            group.pages.map((item) => (
              <NavItem
                key={item.id}
                label={item.title}
                icon={<img className={settings.sidebarGlyph} src={item.icon} alt="" />}
                selected={navigationPage === item.id}
                controls="settings-content"
                onSelect={() => selectPage(item.id)}
              />
            )),
          )}
      </div>
      {!navSearchResults && shownSidebarGroups.length === 0 && (
        <p className={settings.sidebarEmpty} role="status">
          没有匹配的设置
        </p>
      )}
      {preview && <p className={settings.previewLabel}>客户端预览版</p>}
    </nav>
  );
}
