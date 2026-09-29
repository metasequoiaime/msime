import type { MobilePrimaryPageId } from "./mobile-navigation";
import { mobileTabIcon, mobileTabTitle } from "./mobile-tab-helpers";

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
    <nav
      className="hidden max-phone:order-2 max-phone:mx-3 max-phone:mb-[calc(0.5rem+env(safe-area-inset-bottom,0px))] max-phone:grid max-phone:grid-cols-4 max-phone:gap-1 max-phone:rounded-[26px] max-phone:border max-phone:border-edge max-phone:bg-card max-phone:p-1.5 max-phone:shadow-card"
      aria-label="主要功能"
    >
      {tabs.map((item) => (
        <button
          key={item.id}
          type="button"
          className={`flex min-h-[46px] min-w-0 cursor-pointer flex-col items-center justify-center gap-0.5 rounded-[20px] border-0 px-1 py-1 text-[11px] ${
            activeTab === item.id
              ? "bg-accent-soft font-semibold text-accent"
              : "bg-transparent text-muted"
          }`}
          aria-current={activeTab === item.id ? "page" : undefined}
          onClick={() => onSelect(item.id)}
        >
          <img
            src={mobileTabIcon(item.id, item.icon)}
            alt=""
            aria-hidden="true"
            className={`size-[22px] ${activeTab === item.id ? "opacity-100" : "opacity-60"}`}
          />
          {mobileTabTitle(item.id, item.title)}
        </button>
      ))}
    </nav>
  );
}
